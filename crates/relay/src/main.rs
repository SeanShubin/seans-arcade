//! UDP relay server for deterministic lockstep multiplayer.
//!
//! Coordinates input exchange between two clients. The relay never interprets
//! game-specific payload bytes — it only waits for both players to submit input
//! for a tick, then broadcasts the combined inputs to both.
//!
//! Usage: `cargo run -p relay [bind_address]`
//! Default bind address: `0.0.0.0:7700`

use std::net::{SocketAddr, UdpSocket};

use relay::{
    ClientMessage, PlayerSlot, RelayMessage, Tick, deserialize, serialize,
};

const MAX_PLAYERS: usize = 2;
const RECV_BUF_SIZE: usize = 1024;

struct RelayState {
    players: [Option<SocketAddr>; MAX_PLAYERS],
    game_started: bool,
    current_tick: Tick,
    tick_inputs: [Option<Vec<u8>>; MAX_PLAYERS],
}

impl RelayState {
    fn new() -> Self {
        Self {
            players: [None; MAX_PLAYERS],
            game_started: false,
            current_tick: 0,
            tick_inputs: [None, None],
        }
    }

    fn find_player(&self, addr: &SocketAddr) -> Option<usize> {
        self.players.iter().position(|slot| slot.as_ref() == Some(addr))
    }

    fn next_empty_slot(&self) -> Option<usize> {
        self.players.iter().position(|slot| slot.is_none())
    }

    fn all_slots_filled(&self) -> bool {
        self.players.iter().all(|slot| slot.is_some())
    }

    fn all_inputs_received(&self) -> bool {
        self.tick_inputs.iter().all(|input| input.is_some())
    }
}

fn main() {
    let bind_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:7700".into());

    let socket = UdpSocket::bind(&bind_addr)
        .unwrap_or_else(|e| panic!("failed to bind to {bind_addr}: {e}"));

    println!("relay: listening on {bind_addr}");

    let mut state = RelayState::new();
    let mut buf = [0u8; RECV_BUF_SIZE];

    loop {
        let (len, src) = match socket.recv_from(&mut buf) {
            Ok(result) => result,
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
            ClientMessage::Hello => {
                // Already connected? Re-send welcome.
                if let Some(slot) = state.find_player(&src) {
                    let welcome = serialize(&RelayMessage::Welcome {
                        player_slot: slot as PlayerSlot,
                    });
                    let _ = socket.send_to(&welcome, src);
                    if state.game_started {
                        let start = serialize(&RelayMessage::GameStart);
                        let _ = socket.send_to(&start, src);
                    }
                    continue;
                }

                let Some(slot) = state.next_empty_slot() else {
                    eprintln!("relay: rejected {src}, game is full");
                    continue;
                };

                state.players[slot] = Some(src);
                println!("relay: player {slot} connected from {src}");

                let welcome = serialize(&RelayMessage::Welcome {
                    player_slot: slot as PlayerSlot,
                });
                let _ = socket.send_to(&welcome, src);

                if state.all_slots_filled() && !state.game_started {
                    state.game_started = true;
                    println!("relay: all players connected, starting game");
                    let start = serialize(&RelayMessage::GameStart);
                    for addr in state.players.iter().flatten() {
                        let _ = socket.send_to(&start, addr);
                    }
                }
            }
            ClientMessage::Input { tick, payload } => {
                let Some(slot) = state.find_player(&src) else {
                    eprintln!("relay: input from unknown client {src}");
                    continue;
                };

                if tick != state.current_tick {
                    // Ignore inputs for wrong tick (stale or future).
                    continue;
                }

                state.tick_inputs[slot] = Some(payload);

                if state.all_inputs_received() {
                    let inputs: Vec<Vec<u8>> = state
                        .tick_inputs
                        .iter()
                        .map(|input| input.clone().unwrap())
                        .collect();

                    let msg = serialize(&RelayMessage::TickInputs {
                        tick: state.current_tick,
                        inputs,
                    });

                    for addr in state.players.iter().flatten() {
                        let _ = socket.send_to(&msg, addr);
                    }

                    // Advance to next tick.
                    state.current_tick += 1;
                    state.tick_inputs = [None, None];
                }
            }
        }
    }
}
