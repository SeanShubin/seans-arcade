//! Networking plugin: UDP connection to relay server.
//!
//! Owns: ConnectionState, PeerList, ClientConfig, NetSocket.
//! Cross-domain communication uses RelayEvent (Bevy event).

use std::net::{ToSocketAddrs, UdpSocket};

use bevy::prelude::*;
use protocol::{ClientMessage, RelayMessage, deserialize, serialize};

use crate::config::{data_dir_from_args, load_config, save_config};

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ConnectionState::Loading)
            .init_resource::<PeerList>()
            .insert_resource(HelloTimer(Timer::from_seconds(0.5, TimerMode::Repeating)))
            .insert_resource(KeepaliveTimer(Timer::from_seconds(10.0, TimerMode::Repeating)))
            .add_message::<RelayEvent>()
            .add_message::<ConnectRequest>()
            .add_systems(Startup, setup_network)
            .add_systems(
                Update,
                (
                    handle_connect_request,
                    send_hello.run_if(is_connecting),
                    receive_messages.run_if(has_socket).in_set(ReceiveSet),
                    send_keepalive.run_if(is_connected),
                ),
            );
    }
}

/// Relay message forwarded as a Bevy message for cross-domain consumers.
#[derive(Message)]
pub struct RelayEvent(pub RelayMessage);

/// Fired by chat to request a new connection (after relay secret entry).
#[derive(Message)]
pub struct ConnectRequest;

/// System set for ordering: chat systems that read RelayEvent run after this.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReceiveSet;

#[derive(Resource)]
pub struct NetSocket {
    pub socket: UdpSocket,
    pub relay_addr: std::net::SocketAddr,
}

#[derive(Resource, Debug, PartialEq, Eq, Clone)]
pub enum ConnectionState {
    Loading,
    FirstLaunch,
    NeedsRelaySecret,
    Connecting,
    Connected,
    NameClaimed,
    #[allow(dead_code)]
    Disconnected,
}

#[derive(Resource, Default)]
pub struct PeerList(pub Vec<String>);

#[derive(Resource)]
pub struct ClientConfig {
    pub config: crate::config::Config,
    pub data_dir: std::path::PathBuf,
}

#[derive(Resource)]
struct HelloTimer(Timer);

#[derive(Resource)]
struct KeepaliveTimer(Timer);

fn is_connecting(state: Res<ConnectionState>) -> bool {
    *state == ConnectionState::Connecting
}

fn is_connected(state: Res<ConnectionState>) -> bool {
    *state == ConnectionState::Connected
}

fn has_socket(socket: Option<Res<NetSocket>>) -> bool {
    socket.is_some()
}

fn setup_network(mut commands: Commands, mut state: ResMut<ConnectionState>) {
    let data_dir = data_dir_from_args();
    let config = load_config(&data_dir);

    if config.identity_name.is_empty() {
        bevy::log::info!("first launch: no identity name");
        *state = ConnectionState::FirstLaunch;
        commands.insert_resource(ClientConfig { config, data_dir });
        return;
    }

    if config.relay_secret.is_none() {
        bevy::log::info!("needs relay secret: name={}", config.identity_name);
        *state = ConnectionState::NeedsRelaySecret;
        commands.insert_resource(ClientConfig { config, data_dir });
        return;
    }

    let relay_addr = config
        .relay_address
        .to_socket_addrs()
        .expect("failed to resolve relay address")
        .next()
        .expect("relay address resolved to no addresses");

    let socket = UdpSocket::bind("0.0.0.0:0").expect("failed to bind local UDP socket");
    socket
        .set_nonblocking(true)
        .expect("failed to set non-blocking");

    commands.insert_resource(NetSocket {
        socket,
        relay_addr,
    });
    commands.insert_resource(ClientConfig { config, data_dir });
    *state = ConnectionState::Connecting;
}

fn send_hello(net: Option<Res<NetSocket>>, config: Res<ClientConfig>, mut timer: ResMut<HelloTimer>, time: Res<Time>) {
    let Some(net) = net else { return };
    timer.0.tick(time.delta());
    if timer.0.just_finished() {
        let msg = serialize(&ClientMessage::Hello {
            commit_hash: env!("GIT_COMMIT_HASH").to_string(),
            relay_secret: config.config.relay_secret.clone().unwrap_or_default(),
            identity_name: config.config.identity_name.clone(),
            identity_secret: config.config.identity_secret.clone(),
            new_identity_secret: config.config.new_identity_secret.clone(),
        });
        let _ = net.socket.send_to(&msg, net.relay_addr);
    }
}

fn receive_messages(
    net: Res<NetSocket>,
    mut state: ResMut<ConnectionState>,
    mut events: MessageWriter<RelayEvent>,
    mut peers: ResMut<PeerList>,
    mut config: ResMut<ClientConfig>,
) {
    let mut buf = [0u8; 4096];
    loop {
        let len = match net.socket.recv(&mut buf) {
            Ok(len) => len,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => {
                eprintln!("arcade: recv error: {e}");
                break;
            }
        };

        let Some(msg) = deserialize::<RelayMessage>(&buf[..len]) else {
            continue;
        };

        apply_relay_message(&msg, &mut state, &mut peers, &mut config);

        // Forward to cross-domain consumers (chat UI, future systems)
        events.write(RelayEvent(msg));
    }
}

/// Pure logic for handling relay messages — no IO, testable directly.
fn apply_relay_message(
    msg: &RelayMessage,
    state: &mut ConnectionState,
    peers: &mut PeerList,
    config: &mut ClientConfig,
) {
    match msg {
        RelayMessage::Welcome { peer_count } => {
            if *state == ConnectionState::Connecting {
                println!("arcade: connected ({peer_count} peers)");
                if config.config.new_identity_secret.is_some() {
                    config.config.identity_secret =
                        config.config.new_identity_secret.take().unwrap();
                }
                save_config(&config.data_dir, &config.config);
                *state = ConnectionState::Connected;
            }
        }
        RelayMessage::NameClaimed => {
            *state = ConnectionState::NameClaimed;
        }
        RelayMessage::RejectSecret => {
            config.config.relay_secret = None;
            *state = ConnectionState::NeedsRelaySecret;
        }
        RelayMessage::PeerJoined { name } => {
            if !peers.0.contains(name) {
                peers.0.push(name.clone());
            }
        }
        RelayMessage::PeerLeft { name } => {
            peers.0.retain(|n| n != name);
        }
        _ => {}
    }
}

fn handle_connect_request(
    mut events: MessageReader<ConnectRequest>,
    mut commands: Commands,
    config: Res<ClientConfig>,
    mut state: ResMut<ConnectionState>,
) {
    for _ in events.read() {
        let relay_addr = config
            .config
            .relay_address
            .to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.next())
            .unwrap_or_else(|| "127.0.0.1:7700".parse().unwrap());

        let socket = UdpSocket::bind("0.0.0.0:0").expect("failed to bind local UDP socket");
        socket
            .set_nonblocking(true)
            .expect("failed to set non-blocking");

        commands.insert_resource(NetSocket {
            socket,
            relay_addr,
        });
        *state = ConnectionState::Connecting;
    }
}

fn send_keepalive(
    net: Option<Res<NetSocket>>,
    mut timer: ResMut<KeepaliveTimer>,
    time: Res<Time>,
) {
    let Some(net) = net else { return };
    timer.0.tick(time.delta());
    if timer.0.just_finished() {
        let msg = serialize(&ClientMessage::Input {
            payload: Vec::new(),
        });
        let _ = net.socket.send_to(&msg, net.relay_addr);
    }
}

pub fn send_chat_message(net: &NetSocket, text: &str) {
    use protocol::ChatPayload;
    let payload = serialize(&ChatPayload::Text(text.to_string()));
    let msg = serialize(&ClientMessage::Input { payload });
    let _ = net.socket.send_to(&msg, net.relay_addr);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use protocol::RelayMessage;

    fn test_config() -> ClientConfig {
        let dir = std::env::temp_dir().join(format!("arcade_net_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        ClientConfig {
            config: Config {
                identity_name: "Alice".into(),
                identity_secret: "old-secret".into(),
                relay_address: "127.0.0.1:7700".into(),
                relay_secret: Some("test".into()),
                new_identity_secret: None,
            },
            data_dir: dir,
        }
    }

    // -- Welcome --

    #[test]
    fn welcome_when_connecting_transitions_to_connected() {
        // given
        let mut state = ConnectionState::Connecting;
        let mut peers = PeerList::default();
        let mut config = test_config();

        // when
        apply_relay_message(
            &RelayMessage::Welcome { peer_count: 3 },
            &mut state,
            &mut peers,
            &mut config,
        );

        // then
        assert_eq!(state, ConnectionState::Connected);
    }

    #[test]
    fn welcome_when_already_connected_is_ignored() {
        // given
        let mut state = ConnectionState::Connected;
        let mut peers = PeerList::default();
        let mut config = test_config();

        // when
        apply_relay_message(
            &RelayMessage::Welcome { peer_count: 1 },
            &mut state,
            &mut peers,
            &mut config,
        );

        // then
        assert_eq!(state, ConnectionState::Connected);
    }

    #[test]
    fn welcome_applies_new_identity_secret() {
        // given
        let mut state = ConnectionState::Connecting;
        let mut peers = PeerList::default();
        let mut config = test_config();
        config.config.new_identity_secret = Some("new-secret".into());

        // when
        apply_relay_message(
            &RelayMessage::Welcome { peer_count: 0 },
            &mut state,
            &mut peers,
            &mut config,
        );

        // then
        assert_eq!(config.config.identity_secret, "new-secret");
        assert!(config.config.new_identity_secret.is_none());
    }

    // -- RejectSecret --

    #[test]
    fn reject_secret_clears_secret_and_transitions() {
        // given
        let mut state = ConnectionState::Connecting;
        let mut peers = PeerList::default();
        let mut config = test_config();

        // when
        apply_relay_message(
            &RelayMessage::RejectSecret,
            &mut state,
            &mut peers,
            &mut config,
        );

        // then
        assert_eq!(state, ConnectionState::NeedsRelaySecret);
        assert!(config.config.relay_secret.is_none());
    }

    // -- NameClaimed --

    #[test]
    fn name_claimed_transitions_state() {
        // given
        let mut state = ConnectionState::Connecting;
        let mut peers = PeerList::default();
        let mut config = test_config();

        // when
        apply_relay_message(
            &RelayMessage::NameClaimed,
            &mut state,
            &mut peers,
            &mut config,
        );

        // then
        assert_eq!(state, ConnectionState::NameClaimed);
    }

    // -- PeerJoined --

    #[test]
    fn peer_joined_adds_to_list() {
        // given
        let mut state = ConnectionState::Connected;
        let mut peers = PeerList::default();
        let mut config = test_config();

        // when
        apply_relay_message(
            &RelayMessage::PeerJoined { name: "Bob".into() },
            &mut state,
            &mut peers,
            &mut config,
        );

        // then
        assert_eq!(peers.0, vec!["Bob"]);
    }

    #[test]
    fn peer_joined_does_not_duplicate() {
        // given
        let mut state = ConnectionState::Connected;
        let mut peers = PeerList(vec!["Bob".into()]);
        let mut config = test_config();

        // when
        apply_relay_message(
            &RelayMessage::PeerJoined { name: "Bob".into() },
            &mut state,
            &mut peers,
            &mut config,
        );

        // then
        assert_eq!(peers.0.len(), 1);
    }

    // -- PeerLeft --

    #[test]
    fn peer_left_removes_from_list() {
        // given
        let mut state = ConnectionState::Connected;
        let mut peers = PeerList(vec!["Alice".into(), "Bob".into()]);
        let mut config = test_config();

        // when
        apply_relay_message(
            &RelayMessage::PeerLeft { name: "Bob".into() },
            &mut state,
            &mut peers,
            &mut config,
        );

        // then
        assert_eq!(peers.0, vec!["Alice"]);
    }

    #[test]
    fn peer_left_unknown_name_is_noop() {
        // given
        let mut state = ConnectionState::Connected;
        let mut peers = PeerList(vec!["Alice".into()]);
        let mut config = test_config();

        // when
        apply_relay_message(
            &RelayMessage::PeerLeft { name: "Unknown".into() },
            &mut state,
            &mut peers,
            &mut config,
        );

        // then
        assert_eq!(peers.0, vec!["Alice"]);
    }

    // -- Other messages are no-ops for net domain --

    #[test]
    fn broadcast_does_not_change_net_state() {
        // given
        let mut state = ConnectionState::Connected;
        let mut peers = PeerList(vec!["Bob".into()]);
        let mut config = test_config();

        // when
        apply_relay_message(
            &RelayMessage::Broadcast {
                from: "Bob".into(),
                payload: vec![1, 2, 3],
            },
            &mut state,
            &mut peers,
            &mut config,
        );

        // then
        assert_eq!(state, ConnectionState::Connected);
        assert_eq!(peers.0.len(), 1);
    }
}

