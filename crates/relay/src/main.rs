//! UDP relay server for chat.
//!
//! Coordinates message exchange between clients. The relay never interprets
//! application-level payloads — it broadcasts Input messages to all peers
//! with the same commit hash.
//!
//! Usage: `RELAY_SECRET=test cargo run -p relay [-- --data-dir local/relay --bind 0.0.0.0:7700]`

mod identity;
mod logging;
mod s3;

use std::collections::{HashMap, VecDeque};
use std::net::{SocketAddr, UdpSocket};
use std::time::Instant;

use identity::{IdentityRegistry, ValidationResult};
use logging::LogWriter;
use protocol::{ClientMessage, ChatPayload, HistoryEntry, RelayMessage, deserialize, serialize};

const RECV_BUF_SIZE: usize = 4096;
const TIMEOUT_SECS: u64 = 30;
const CHAT_HISTORY_SIZE: usize = 1000;

struct ClientInfo {
    identity_name: String,
    commit_hash: String,
    last_seen: Instant,
}

/// IO-free action returned by handler functions. Executed by the main loop.
#[derive(Debug, Clone, PartialEq)]
enum RelayAction {
    SendTo(SocketAddr, RelayMessage),
    SaveRegistry,
    LogChat { from: String, text: String },
    SyncChatHistory,
}

struct RelayState {
    clients: HashMap<SocketAddr, ClientInfo>,
    identity_registry: IdentityRegistry,
    relay_secret: String,
    log_writer: LogWriter,
    registry_path: std::path::PathBuf,
    /// Per-version chat history buffer. Key is commit_hash.
    chat_history: HashMap<String, VecDeque<HistoryEntry>>,
}

impl RelayState {
    fn peer_count_for_hash(&self, commit_hash: &str) -> u32 {
        self.clients
            .values()
            .filter(|c| c.commit_hash == commit_hash)
            .count() as u32
    }

    fn broadcast_actions(
        &self,
        msg: &RelayMessage,
        commit_hash: &str,
        exclude: Option<&SocketAddr>,
    ) -> Vec<RelayAction> {
        self.clients
            .iter()
            .filter(|(addr, client)| {
                client.commit_hash == commit_hash && exclude.map_or(true, |ex| *addr != ex)
            })
            .map(|(addr, _)| RelayAction::SendTo(*addr, msg.clone()))
            .collect()
    }

    /// Pure logic: process a client message and return actions to execute.
    fn handle_message(
        &mut self,
        src: SocketAddr,
        msg: ClientMessage,
        now: Instant,
    ) -> Vec<RelayAction> {
        let mut actions = Vec::new();

        match msg {
            ClientMessage::Hello {
                commit_hash,
                relay_secret,
                identity_name,
                identity_secret,
                new_identity_secret,
            } => {
                if relay_secret != self.relay_secret {
                    actions.push(RelayAction::SendTo(src, RelayMessage::RejectSecret));
                    return actions;
                }

                let result = self.identity_registry.validate(
                    &identity_name,
                    &identity_secret,
                    new_identity_secret.as_deref(),
                );

                match result {
                    ValidationResult::NameClaimed => {
                        actions.push(RelayAction::SendTo(src, RelayMessage::NameClaimed));
                        return actions;
                    }
                    ValidationResult::NewRegistration | ValidationResult::Accepted => {
                        actions.push(RelayAction::SaveRegistry);
                    }
                }

                // Remove any existing connection for this address
                if let Some(old_info) = self.clients.remove(&src) {
                    let left = RelayMessage::PeerLeft {
                        name: old_info.identity_name.clone(),
                    };
                    actions.extend(self.broadcast_actions(&left, &old_info.commit_hash, Some(&src)));
                }

                let peer_count = self.peer_count_for_hash(&commit_hash);

                // Welcome the new client
                actions.push(RelayAction::SendTo(
                    src,
                    RelayMessage::Welcome { peer_count },
                ));

                // Flush chat history to S3 so the new client can download it.
                actions.push(RelayAction::SyncChatHistory);

                // Broadcast PeerJoined to existing peers
                let joined = RelayMessage::PeerJoined {
                    name: identity_name.clone(),
                };
                actions.extend(self.broadcast_actions(&joined, &commit_hash, Some(&src)));

                // Send existing peer names to the new client
                for client in self.clients.values() {
                    if client.commit_hash == commit_hash {
                        actions.push(RelayAction::SendTo(
                            src,
                            RelayMessage::PeerJoined {
                                name: client.identity_name.clone(),
                            },
                        ));
                    }
                }

                self.clients.insert(
                    src,
                    ClientInfo {
                        identity_name,
                        commit_hash,
                        last_seen: now,
                    },
                );
            }
            ClientMessage::Input { payload } => {
                let Some(client) = self.clients.get_mut(&src) else {
                    return actions;
                };
                client.last_seen = now;

                let from = client.identity_name.clone();
                let commit_hash = client.commit_hash.clone();

                if let Some(ChatPayload::Text(text)) = deserialize::<ChatPayload>(&payload) {
                    actions.push(RelayAction::LogChat {
                        from: from.clone(),
                        text,
                    });
                }

                // Buffer non-empty payloads (skip keepalives)
                if !payload.is_empty() {
                    let buffer = self.chat_history.entry(commit_hash.clone()).or_default();
                    buffer.push_back(HistoryEntry {
                        from: from.clone(),
                        payload: payload.clone(),
                    });
                    if buffer.len() > CHAT_HISTORY_SIZE {
                        buffer.pop_front();
                    }
                }

                let broadcast = RelayMessage::Broadcast { from, payload };
                actions.extend(self.broadcast_actions(&broadcast, &commit_hash, None));
            }
            ClientMessage::Disconnect => {
                if let Some(info) = self.clients.remove(&src) {
                    let left = RelayMessage::PeerLeft {
                        name: info.identity_name.clone(),
                    };
                    actions.extend(self.broadcast_actions(&left, &info.commit_hash, None));
                }
            }
        }

        actions
    }

    /// Pure logic: find timed-out clients and return actions.
    fn sweep_timeouts(&mut self, now: Instant) -> Vec<RelayAction> {
        let mut actions = Vec::new();

        let timed_out: Vec<SocketAddr> = self
            .clients
            .iter()
            .filter(|(_, info)| now.duration_since(info.last_seen).as_secs() > TIMEOUT_SECS)
            .map(|(addr, _)| *addr)
            .collect();

        for addr in timed_out {
            if let Some(info) = self.clients.remove(&addr) {
                println!("relay: {} timed out from {}", info.identity_name, addr);
                let left = RelayMessage::PeerLeft {
                    name: info.identity_name.clone(),
                };
                actions.extend(self.broadcast_actions(&left, &info.commit_hash, None));
            }
        }

        actions
    }
}

fn execute_actions(
    socket: &UdpSocket,
    state: &mut RelayState,
    actions: &[RelayAction],
    s3_client: &Option<s3::S3Client>,
) {
    for action in actions {
        match action {
            RelayAction::SendTo(addr, msg) => {
                let data = serialize(msg);
                let _ = socket.send_to(&data, *addr);
            }
            RelayAction::SaveRegistry => {
                state.identity_registry.save(&state.registry_path);
            }
            RelayAction::LogChat { from, text } => {
                state.log_writer.log_message(from, text);
            }
            RelayAction::SyncChatHistory => {
                if let Some(s3) = s3_client {
                    let persisted =
                        protocol::PersistedChatHistory::from_memory(&state.chat_history);
                    s3.put_json("admin/chat-history.json", &persisted);
                }
            }
        }
    }
}

const S3_SYNC_INTERVAL_SECS: u64 = 15;

/// Write all admin state to S3. Best-effort: logs errors but never crashes.
/// If any file is deleted from S3, it gets recreated on the next sync cycle.
fn sync_to_s3(state: &RelayState, s3: &s3::S3Client, start_time: Instant) {
    let now = chrono::Utc::now().to_rfc3339();

    // Heartbeat
    s3.put_json(
        "admin/heartbeat.json",
        &s3::Heartbeat {
            timestamp: now.clone(),
            uptime_secs: start_time.elapsed().as_secs(),
            client_count: state.clients.len(),
            commit_hash: env!("GIT_COMMIT_HASH").to_string(),
        },
    );

    // Connected users
    let current_time = Instant::now();
    let users: Vec<s3::ConnectedUser> = state
        .clients
        .values()
        .map(|c| s3::ConnectedUser {
            name: c.identity_name.clone(),
            commit_hash: c.commit_hash.clone(),
            idle_secs: current_time.duration_since(c.last_seen).as_secs(),
        })
        .collect();
    s3.put_json(
        "admin/connected.json",
        &s3::ConnectedUsers {
            timestamp: now.clone(),
            users,
        },
    );

    // Per-version chat history
    for (hash, entries) in &state.chat_history {
        let persisted = protocol::persist_entries(entries);
        s3.put_json(&format!("admin/versions/{hash}/chat-history.json"), &persisted);
    }

    // Registered identities (names only, no secrets)
    s3.put_json(
        "admin/identities.json",
        &s3::RegisteredIdentities {
            timestamp: now,
            names: state.identity_registry.names(),
        },
    );
}

const ADMIN_COMMANDS_PREFIX: &str = "admin/commands/";

/// Poll S3 for admin command files, execute them, and delete the files.
/// Returns actions to execute (e.g., disconnect messages to send).
fn poll_admin_commands(state: &mut RelayState, s3: &s3::S3Client) -> Vec<RelayAction> {
    let mut actions = Vec::new();

    let keys = s3.list_keys(ADMIN_COMMANDS_PREFIX);
    for key in keys {
        // Skip the prefix itself (S3 may return the "directory" key)
        if key == ADMIN_COMMANDS_PREFIX {
            continue;
        }

        let Some(cmd) = s3.get_json::<s3::AdminCommand>(&key) else {
            eprintln!("relay: s3: ignoring unparseable command file: {key}");
            s3.delete(&key);
            continue;
        };

        println!("relay: s3: executing command from {key}: {cmd:?}");

        match cmd {
            s3::AdminCommand::DeleteUser { name } => {
                // Remove from identity registry
                if state.identity_registry.remove(&name) {
                    state.identity_registry.save(&state.registry_path);
                    println!("relay: deleted identity: {name}");
                } else {
                    println!("relay: identity not found: {name}");
                }

                // Disconnect any active connections for this user
                let to_disconnect: Vec<SocketAddr> = state
                    .clients
                    .iter()
                    .filter(|(_, c)| c.identity_name == name)
                    .map(|(addr, _)| *addr)
                    .collect();
                for addr in to_disconnect {
                    if let Some(info) = state.clients.remove(&addr) {
                        let left = RelayMessage::PeerLeft {
                            name: info.identity_name.clone(),
                        };
                        actions.extend(
                            state.broadcast_actions(&left, &info.commit_hash, None),
                        );
                    }
                }
            }
            s3::AdminCommand::ResetIdentity { name } => {
                if state.identity_registry.remove(&name) {
                    state.identity_registry.save(&state.registry_path);
                    println!("relay: reset identity: {name}");
                } else {
                    println!("relay: identity not found: {name}");
                }
            }
            s3::AdminCommand::Broadcast { message } => {
                let payload = serialize(&ChatPayload::Text(message.clone()));
                // Add to each version group's chat history
                for entries in state.chat_history.values_mut() {
                    entries.push_back(HistoryEntry {
                        from: String::new(),
                        payload: payload.clone(),
                    });
                    if entries.len() > CHAT_HISTORY_SIZE {
                        entries.pop_front();
                    }
                }
                // Send to all connected clients
                let msg = RelayMessage::Broadcast {
                    from: String::new(),
                    payload,
                };
                for addr in state.clients.keys() {
                    actions.push(RelayAction::SendTo(*addr, msg.clone()));
                }
                println!("relay: broadcast: {message}");
            }
            s3::AdminCommand::Drain => {
                let count = state.clients.len();
                state.clients.clear();
                println!("relay: drained {count} clients");
            }
        }

        // Delete the command file after execution
        s3.delete(&key);
    }

    actions
}

fn data_dir_from_args() -> std::path::PathBuf {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() - 1 {
        if args[i] == "--data-dir" {
            return std::path::PathBuf::from(&args[i + 1]);
        }
    }
    std::path::PathBuf::from(".")
}

fn bind_address_from_args() -> String {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() - 1 {
        if args[i] == "--bind" {
            return args[i + 1].clone();
        }
    }
    "0.0.0.0:7700".into()
}

fn relay_secret_from_env() -> String {
    std::env::var("RELAY_SECRET").unwrap_or_else(|_| {
        eprintln!("relay: WARNING: RELAY_SECRET not set, using empty string");
        String::new()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct RelayTester {
        state: RelayState,
        _temp_dir: std::path::PathBuf,
    }

    impl RelayTester {
        fn new() -> Self {
            let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let temp_dir = std::env::temp_dir().join(format!(
                "relay_test_{}_{}",
                std::process::id(),
                id
            ));
            let log_dir = LogWriter::log_dir_from_data_dir(&temp_dir);
            let registry_path = IdentityRegistry::path_from_data_dir(&temp_dir);
            let state = RelayState {
                clients: HashMap::new(),
                identity_registry: IdentityRegistry::default(),
                relay_secret: "test_secret".to_string(),
                log_writer: LogWriter::new(&log_dir),
                registry_path,
                chat_history: HashMap::new(),
            };
            Self {
                state,
                _temp_dir: temp_dir,
            }
        }

        fn add_client(&mut self, addr: SocketAddr, name: &str, hash: &str, last_seen: Instant) {
            self.state.clients.insert(
                addr,
                ClientInfo {
                    identity_name: name.to_string(),
                    commit_hash: hash.to_string(),
                    last_seen,
                },
            );
        }

        fn handle(&mut self, src: SocketAddr, msg: ClientMessage) -> Vec<RelayAction> {
            self.state.handle_message(src, msg, Instant::now())
        }

        fn sweep(&mut self, now: Instant) -> Vec<RelayAction> {
            self.state.sweep_timeouts(now)
        }

        fn client_count(&self) -> usize {
            self.state.clients.len()
        }

        fn client_names(&self) -> Vec<String> {
            let mut names: Vec<String> = self
                .state
                .clients
                .values()
                .map(|c| c.identity_name.clone())
                .collect();
            names.sort();
            names
        }

        fn has_client(&self, name: &str) -> bool {
            self.state
                .clients
                .values()
                .any(|c| c.identity_name == name)
        }
    }

    impl Drop for RelayTester {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self._temp_dir);
        }
    }

    fn addr(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    fn hello_msg(name: &str, secret: &str, relay_secret: &str) -> ClientMessage {
        ClientMessage::Hello {
            commit_hash: "abc123".to_string(),
            relay_secret: relay_secret.to_string(),
            identity_name: name.to_string(),
            identity_secret: secret.to_string(),
            new_identity_secret: None,
        }
    }

    // ========================================================================
    // Hello tests
    // ========================================================================

    #[test]
    fn hello_with_wrong_relay_secret_returns_reject_secret() {
        // given a relay with a known secret
        let mut tester = RelayTester::new();

        // when a client sends Hello with a wrong relay secret
        let actions = tester.handle(addr("1.0.0.1:1001"), hello_msg("alice", "id_sec", "wrong"));

        // then the relay responds with RejectSecret
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            RelayAction::SendTo(addr("1.0.0.1:1001"), RelayMessage::RejectSecret)
        );

        // and no client is added
        assert_eq!(tester.client_count(), 0);
    }

    #[test]
    fn hello_with_new_name_returns_welcome_and_save_registry() {
        // given an empty relay
        let mut tester = RelayTester::new();

        // when a client sends Hello with a new name
        let actions = tester.handle(
            addr("1.0.0.1:1001"),
            hello_msg("alice", "id_sec", "test_secret"),
        );

        // then the relay responds with SaveRegistry and Welcome
        assert!(actions.contains(&RelayAction::SaveRegistry));
        assert!(actions.contains(&RelayAction::SendTo(
            addr("1.0.0.1:1001"),
            RelayMessage::Welcome { peer_count: 0 },
        )));

        // and the client is added
        assert_eq!(tester.client_count(), 1);
        assert!(tester.has_client("alice"));
    }

    #[test]
    fn hello_with_claimed_name_returns_name_claimed() {
        // given a relay with alice already registered (via identity registry)
        let mut tester = RelayTester::new();
        tester.handle(
            addr("1.0.0.1:1001"),
            hello_msg("alice", "id_sec", "test_secret"),
        );

        // when a different client sends Hello with the same name but wrong identity secret
        let actions = tester.handle(
            addr("1.0.0.2:1002"),
            hello_msg("alice", "wrong_id_sec", "test_secret"),
        );

        // then the relay responds with NameClaimed
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            RelayAction::SendTo(addr("1.0.0.2:1002"), RelayMessage::NameClaimed)
        );

        // and no new client is added (still just the original)
        assert_eq!(tester.client_count(), 1);
    }

    #[test]
    fn hello_broadcasts_peer_joined_to_existing_peer() {
        // given a relay with alice connected
        let mut tester = RelayTester::new();
        tester.handle(
            addr("1.0.0.1:1001"),
            hello_msg("alice", "sec_a", "test_secret"),
        );

        // when bob joins with the same commit hash
        let actions = tester.handle(
            addr("1.0.0.2:1002"),
            hello_msg("bob", "sec_b", "test_secret"),
        );

        // then alice receives PeerJoined for bob
        assert!(actions.contains(&RelayAction::SendTo(
            addr("1.0.0.1:1001"),
            RelayMessage::PeerJoined {
                name: "bob".to_string()
            },
        )));

        // and both clients are present
        assert_eq!(tester.client_count(), 2);
        assert!(tester.has_client("alice"));
        assert!(tester.has_client("bob"));
    }

    #[test]
    fn hello_sends_existing_peer_names_to_new_client() {
        // given a relay with alice connected
        let mut tester = RelayTester::new();
        tester.handle(
            addr("1.0.0.1:1001"),
            hello_msg("alice", "sec_a", "test_secret"),
        );

        // when bob joins
        let actions = tester.handle(
            addr("1.0.0.2:1002"),
            hello_msg("bob", "sec_b", "test_secret"),
        );

        // then bob receives PeerJoined for alice (existing peer notification)
        assert!(actions.contains(&RelayAction::SendTo(
            addr("1.0.0.2:1002"),
            RelayMessage::PeerJoined {
                name: "alice".to_string()
            },
        )));
    }

    // ========================================================================
    // Input tests
    // ========================================================================

    #[test]
    fn input_from_known_client_broadcasts_and_logs() {
        // given a relay with alice and bob connected (same commit hash)
        let mut tester = RelayTester::new();
        tester.handle(
            addr("1.0.0.1:1001"),
            hello_msg("alice", "sec_a", "test_secret"),
        );
        tester.handle(
            addr("1.0.0.2:1002"),
            hello_msg("bob", "sec_b", "test_secret"),
        );

        // when alice sends a chat text input
        let chat_payload = protocol::serialize(&protocol::ChatPayload::Text("hello".to_string()));
        let actions = tester.handle(
            addr("1.0.0.1:1001"),
            ClientMessage::Input {
                payload: chat_payload.clone(),
            },
        );

        // then both alice and bob receive the Broadcast
        let expected_broadcast = RelayMessage::Broadcast {
            from: "alice".to_string(),
            payload: chat_payload,
        };
        assert!(actions.contains(&RelayAction::SendTo(
            addr("1.0.0.1:1001"),
            expected_broadcast.clone(),
        )));
        assert!(actions.contains(&RelayAction::SendTo(
            addr("1.0.0.2:1002"),
            expected_broadcast,
        )));

        // and a LogChat action is emitted
        assert!(actions.contains(&RelayAction::LogChat {
            from: "alice".to_string(),
            text: "hello".to_string(),
        }));
    }

    #[test]
    fn input_from_unknown_client_produces_no_actions() {
        // given an empty relay
        let mut tester = RelayTester::new();

        // when an unknown address sends an Input message
        let actions = tester.handle(
            addr("1.0.0.1:1001"),
            ClientMessage::Input {
                payload: vec![1, 2, 3],
            },
        );

        // then no actions are produced
        assert!(actions.is_empty());
    }

    // ========================================================================
    // Disconnect tests
    // ========================================================================

    #[test]
    fn disconnect_removes_client_and_broadcasts_peer_left() {
        // given a relay with alice and bob connected
        let mut tester = RelayTester::new();
        tester.handle(
            addr("1.0.0.1:1001"),
            hello_msg("alice", "sec_a", "test_secret"),
        );
        tester.handle(
            addr("1.0.0.2:1002"),
            hello_msg("bob", "sec_b", "test_secret"),
        );

        // when alice disconnects
        let actions = tester.handle(addr("1.0.0.1:1001"), ClientMessage::Disconnect);

        // then bob receives PeerLeft for alice
        assert!(actions.contains(&RelayAction::SendTo(
            addr("1.0.0.2:1002"),
            RelayMessage::PeerLeft {
                name: "alice".to_string()
            },
        )));

        // and alice is removed from clients
        assert_eq!(tester.client_count(), 1);
        assert!(!tester.has_client("alice"));
        assert!(tester.has_client("bob"));
    }

    #[test]
    fn disconnect_from_unknown_client_produces_no_actions() {
        // given an empty relay
        let mut tester = RelayTester::new();

        // when an unknown address sends Disconnect
        let actions = tester.handle(addr("1.0.0.1:1001"), ClientMessage::Disconnect);

        // then no actions are produced
        assert!(actions.is_empty());
    }

    // ========================================================================
    // Timeout sweep tests
    // ========================================================================

    #[test]
    fn sweep_removes_stale_clients_and_broadcasts_peer_left() {
        // given a relay with alice (stale) and bob (recent)
        let mut tester = RelayTester::new();
        let now = Instant::now();
        let stale_time = now - std::time::Duration::from_secs(TIMEOUT_SECS + 10);
        tester.add_client(addr("1.0.0.1:1001"), "alice", "abc123", stale_time);
        tester.add_client(addr("1.0.0.2:1002"), "bob", "abc123", now);

        // when we sweep at the current time
        let actions = tester.sweep(now);

        // then bob receives PeerLeft for alice
        assert!(actions.contains(&RelayAction::SendTo(
            addr("1.0.0.2:1002"),
            RelayMessage::PeerLeft {
                name: "alice".to_string()
            },
        )));

        // and alice is removed, bob remains
        assert_eq!(tester.client_count(), 1);
        assert!(!tester.has_client("alice"));
        assert!(tester.has_client("bob"));
    }

    #[test]
    fn sweep_ignores_recent_clients() {
        // given a relay with only recent clients
        let mut tester = RelayTester::new();
        let now = Instant::now();
        tester.add_client(addr("1.0.0.1:1001"), "alice", "abc123", now);
        tester.add_client(addr("1.0.0.2:1002"), "bob", "abc123", now);

        // when we sweep at the current time
        let actions = tester.sweep(now);

        // then no actions are produced
        assert!(actions.is_empty());

        // and both clients remain
        assert_eq!(tester.client_count(), 2);
        assert_eq!(tester.client_names(), vec!["alice", "bob"]);
    }
}

fn main() {
    println!("relay {}", env!("GIT_COMMIT_HASH"));

    let bind_addr = bind_address_from_args();
    let socket = UdpSocket::bind(&bind_addr)
        .unwrap_or_else(|e| panic!("failed to bind to {bind_addr}: {e}"));
    // Set a timeout so we can periodically sweep for disconnected clients
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .expect("failed to set read timeout");

    println!("relay: listening on {bind_addr}");

    let data_dir = data_dir_from_args();
    let registry_path = IdentityRegistry::path_from_data_dir(&data_dir);
    let log_dir = LogWriter::log_dir_from_data_dir(&data_dir);

    // S3 is optional: enabled when S3_BUCKET is set.
    // Without it, the relay works fine but chat history doesn't survive restarts.
    let s3_client = match std::env::var("S3_BUCKET") {
        Ok(bucket) => {
            println!("relay: s3: connecting to bucket {bucket}");
            match s3::S3Client::new(bucket) {
                Some(client) => {
                    println!("relay: s3: enabled");
                    Some(client)
                }
                None => {
                    eprintln!("relay: s3: failed to create client, continuing without S3");
                    None
                }
            }
        }
        Err(_) => {
            println!("relay: s3: S3_BUCKET not set, running without S3");
            None
        }
    };

    // Restore per-version chat history from S3.
    let restored_history = s3_client
        .as_ref()
        .map(|s3| {
            println!("relay: s3: restoring chat history...");
            let keys = s3.list_keys("admin/versions/");
            let hashes: std::collections::HashSet<String> = keys
                .iter()
                .filter_map(|k| {
                    k.strip_prefix("admin/versions/")
                        .and_then(|rest| rest.split('/').next())
                        .filter(|h| !h.is_empty())
                        .map(|h| h.to_string())
                })
                .collect();
            let mut history: HashMap<String, VecDeque<HistoryEntry>> = HashMap::new();
            for hash in &hashes {
                let key = format!("admin/versions/{hash}/chat-history.json");
                if let Some(entries) = s3.get_json::<Vec<protocol::PersistedHistoryEntry>>(&key) {
                    let restored = protocol::restore_entries(entries);
                    if !restored.is_empty() {
                        history.insert(hash.clone(), restored);
                    }
                }
            }
            let total: usize = history.values().map(|v| v.len()).sum();
            println!("relay: s3: restored {total} messages across {} version groups", history.len());
            history
        })
        .unwrap_or_default();

    let mut state = RelayState {
        clients: HashMap::new(),
        identity_registry: IdentityRegistry::load(&registry_path),
        relay_secret: relay_secret_from_env(),
        log_writer: LogWriter::new(&log_dir),
        registry_path,
        chat_history: restored_history,
    };

    let mut buf = [0u8; RECV_BUF_SIZE];
    let start_time = Instant::now();
    let mut s3_sync_timer = Instant::now();

    loop {
        let timeout_actions = state.sweep_timeouts(Instant::now());
        execute_actions(&socket, &mut state, &timeout_actions, &s3_client);

        // Periodic S3 sync: write all admin state every S3_SYNC_INTERVAL_SECS.
        // Runs on the same cadence as the recv timeout, so worst case is
        // interval + 5 seconds between syncs.
        if let Some(ref s3) = s3_client {
            if s3_sync_timer.elapsed().as_secs() >= S3_SYNC_INTERVAL_SECS {
                let cmd_actions = poll_admin_commands(&mut state, s3);
                execute_actions(&socket, &mut state, &cmd_actions, &s3_client);
                sync_to_s3(&state, s3, start_time);
                s3_sync_timer = Instant::now();
            }
        }

        let (len, src) = match socket.recv_from(&mut buf) {
            Ok(result) => result,
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(e) => {
                eprintln!("relay: recv error: {e}");
                continue;
            }
        };

        let Some(msg) = deserialize::<ClientMessage>(&buf[..len]) else {
            eprintln!("relay: bad message from {src}");
            continue;
        };

        let actions = state.handle_message(src, msg, Instant::now());
        execute_actions(&socket, &mut state, &actions, &s3_client);
    }
}
