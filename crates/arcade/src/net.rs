//! Networking plugin: UDP connection to relay server.
//!
//! Owns: ConnectionState, PeerList, ClientConfig, NetSocket.
//! Cross-domain communication uses RelayEvent (Bevy event).

use std::net::UdpSocket;

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

    let relay_addr: std::net::SocketAddr = config
        .relay_address
        .parse()
        .expect("invalid relay address in config");

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

        // Net-domain concerns: ConnectionState, PeerList, Config
        match &msg {
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
                println!("arcade: name claimed by another identity");
                *state = ConnectionState::NameClaimed;
            }
            RelayMessage::RejectSecret => {
                bevy::log::info!("received RejectSecret from relay");
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

        // Forward to cross-domain consumers (chat UI, future systems)
        events.write(RelayEvent(msg));
    }
}

fn handle_connect_request(
    mut events: MessageReader<ConnectRequest>,
    mut commands: Commands,
    config: Res<ClientConfig>,
    mut state: ResMut<ConnectionState>,
) {
    for _ in events.read() {
        let relay_addr: std::net::SocketAddr = config
            .config
            .relay_address
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:7700".parse().unwrap());

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

