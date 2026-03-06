//! Shared protocol types for the deterministic lockstep relay.
//!
//! Both the relay server and game clients depend on this crate.
//! Messages are serialized with `postcard` (compact, serde-based, no framing
//! needed since UDP is message-oriented).

use serde::{Deserialize, Serialize};

pub type Tick = u32;
pub type PlayerSlot = u8;

// ---- Client -> Relay --------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    Hello,
    Input { tick: Tick, payload: Vec<u8> },
}

// ---- Relay -> Client --------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub enum RelayMessage {
    Welcome { player_slot: PlayerSlot },
    GameStart,
    TickInputs { tick: Tick, inputs: Vec<Vec<u8>> },
}

// ---- Serialization helpers --------------------------------------------------

pub fn serialize<T: Serialize>(value: &T) -> Vec<u8> {
    postcard::to_allocvec(value).expect("serialization should not fail")
}

pub fn deserialize<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Option<T> {
    postcard::from_bytes(bytes).ok()
}
