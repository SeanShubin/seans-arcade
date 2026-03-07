//! Shared protocol types for the chat relay.
//!
//! All three binaries (arcade, relay, arcade-cli) depend on this crate.
//! Messages are serialized with `postcard` (compact, serde-based, no framing
//! needed since UDP is message-oriented).

use serde::{Deserialize, Serialize};

// ---- Client -> Relay --------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    Hello {
        commit_hash: String,
        relay_secret: String,
        identity_name: String,
        identity_secret: String,
        new_identity_secret: Option<String>,
    },
    Input {
        payload: Vec<u8>,
    },
    Disconnect,
}

// ---- Relay -> Client --------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelayMessage {
    Welcome { peer_count: u32 },
    NameClaimed,
    RejectSecret,
    RejectVersion { expected: String },
    PeerJoined { name: String },
    PeerLeft { name: String },
    Broadcast { from: String, payload: Vec<u8> },
}

// ---- Application-level payload (clients only, opaque to relay) --------------

#[derive(Debug, Serialize, Deserialize)]
pub enum ChatPayload {
    Text(String),
}

// ---- Serialization helpers --------------------------------------------------

pub fn serialize<T: Serialize>(value: &T) -> Vec<u8> {
    postcard::to_allocvec(value).expect("serialization should not fail")
}

pub fn deserialize<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Option<T> {
    postcard::from_bytes(bytes).ok()
}
