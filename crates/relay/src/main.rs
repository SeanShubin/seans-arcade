//! UDP relay server for chat.
//!
//! Coordinates message exchange between clients. The relay never interprets
//! application-level payloads — it broadcasts Input messages to all peers
//! with the same commit hash.
//!
//! Usage: `RELAY_SECRET=test cargo run -p relay [-- --data-dir local/relay --bind 0.0.0.0:7700]`

mod identity;
mod logging;

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::Instant;

use identity::{IdentityRegistry, ValidationResult};
use logging::LogWriter;
use protocol::{ClientMessage, ChatPayload, RelayMessage, deserialize, serialize};

const RECV_BUF_SIZE: usize = 4096;
const TIMEOUT_SECS: u64 = 30;

struct ClientInfo {
    identity_name: String,
    commit_hash: String,
    last_seen: Instant,
}

struct RelayState {
    clients: HashMap<SocketAddr, ClientInfo>,
    identity_registry: IdentityRegistry,
    relay_secret: String,
    log_writer: LogWriter,
    registry_path: std::path::PathBuf,
}

impl RelayState {
    fn peer_count_for_hash(&self, commit_hash: &str) -> u32 {
        self.clients
            .values()
            .filter(|c| c.commit_hash == commit_hash)
            .count() as u32
    }

    fn broadcast_to_peers(
        &self,
        socket: &UdpSocket,
        msg: &RelayMessage,
        commit_hash: &str,
        exclude: Option<&SocketAddr>,
    ) {
        let data = serialize(msg);
        for (addr, client) in &self.clients {
            if client.commit_hash == commit_hash && exclude.map_or(true, |ex| addr != ex) {
                let _ = socket.send_to(&data, addr);
            }
        }
    }
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

    let mut state = RelayState {
        clients: HashMap::new(),
        identity_registry: IdentityRegistry::load(&registry_path),
        relay_secret: relay_secret_from_env(),
        log_writer: LogWriter::new(&log_dir),
        registry_path,
    };

    let mut buf = [0u8; RECV_BUF_SIZE];

    loop {
        // Timeout sweep: remove clients not seen in TIMEOUT_SECS
        let now = Instant::now();
        let timed_out: Vec<SocketAddr> = state
            .clients
            .iter()
            .filter(|(_, info)| now.duration_since(info.last_seen).as_secs() > TIMEOUT_SECS)
            .map(|(addr, _)| *addr)
            .collect();

        for addr in timed_out {
            if let Some(info) = state.clients.remove(&addr) {
                println!("relay: {} timed out from {}", info.identity_name, addr);
                let msg = RelayMessage::PeerLeft {
                    name: info.identity_name.clone(),
                };
                state.broadcast_to_peers(&socket, &msg, &info.commit_hash, None);
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

        match msg {
            ClientMessage::Hello {
                commit_hash,
                relay_secret,
                identity_name,
                identity_secret,
                new_identity_secret,
            } => {
                // Validate relay secret
                if relay_secret != state.relay_secret {
                    let reject = serialize(&RelayMessage::RejectSecret);
                    let _ = socket.send_to(&reject, src);
                    continue;
                }

                // Validate identity
                let result = state.identity_registry.validate(
                    &identity_name,
                    &identity_secret,
                    new_identity_secret.as_deref(),
                );

                match result {
                    ValidationResult::NameClaimed => {
                        let reject = serialize(&RelayMessage::NameClaimed);
                        let _ = socket.send_to(&reject, src);
                        continue;
                    }
                    ValidationResult::NewRegistration | ValidationResult::Accepted => {
                        state.identity_registry.save(&state.registry_path);
                    }
                }

                // Remove any existing connection for this address
                if let Some(old_info) = state.clients.remove(&src) {
                    let left = RelayMessage::PeerLeft {
                        name: old_info.identity_name.clone(),
                    };
                    state.broadcast_to_peers(&socket, &left, &old_info.commit_hash, Some(&src));
                }

                let peer_count = state.peer_count_for_hash(&commit_hash);
                println!(
                    "relay: {} connected from {} (hash: {}, peers: {})",
                    identity_name,
                    src,
                    &commit_hash[..8.min(commit_hash.len())],
                    peer_count + 1
                );

                // Send Welcome
                let welcome = serialize(&RelayMessage::Welcome {
                    peer_count: peer_count,
                });
                let _ = socket.send_to(&welcome, src);

                // Broadcast PeerJoined to existing peers
                let joined = RelayMessage::PeerJoined {
                    name: identity_name.clone(),
                };
                state.broadcast_to_peers(&socket, &joined, &commit_hash, Some(&src));

                // Send existing peer names to the new client
                for client in state.clients.values() {
                    if client.commit_hash == commit_hash {
                        let peer_joined = serialize(&RelayMessage::PeerJoined {
                            name: client.identity_name.clone(),
                        });
                        let _ = socket.send_to(&peer_joined, src);
                    }
                }

                state.clients.insert(
                    src,
                    ClientInfo {
                        identity_name,
                        commit_hash,
                        last_seen: Instant::now(),
                    },
                );
            }
            ClientMessage::Input { payload } => {
                let Some(client) = state.clients.get_mut(&src) else {
                    continue;
                };
                client.last_seen = Instant::now();

                let from = client.identity_name.clone();
                let commit_hash = client.commit_hash.clone();

                // Try to decode payload for logging
                if let Some(chat) = deserialize::<ChatPayload>(&payload) {
                    match &chat {
                        ChatPayload::Text(text) => {
                            state.log_writer.log_message(&from, text);
                        }
                    }
                }

                // Broadcast to all peers with same commit hash (including sender for echo)
                let broadcast = RelayMessage::Broadcast {
                    from,
                    payload,
                };
                state.broadcast_to_peers(&socket, &broadcast, &commit_hash, None);
            }
            ClientMessage::Disconnect => {
                if let Some(info) = state.clients.remove(&src) {
                    println!("relay: {} disconnected from {}", info.identity_name, src);
                    let left = RelayMessage::PeerLeft {
                        name: info.identity_name.clone(),
                    };
                    state.broadcast_to_peers(&socket, &left, &info.commit_hash, None);
                }
            }
        }
    }
}
