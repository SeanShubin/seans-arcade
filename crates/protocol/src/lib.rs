//! Shared protocol types for the chat relay.
//!
//! All three binaries (arcade, relay, arcade-ops) depend on this crate.
//! Messages are serialized with `postcard` (compact, serde-based, no framing
//! needed since UDP is message-oriented).

use serde::{Deserialize, Serialize};

// Re-export the derive macro so downstream crates use `protocol::HasSchema`.
pub use protocol_derive::HasSchema;

/// Trait for types that can describe their own schema structure.
pub trait HasSchema {
    fn schema() -> SchemaType;
}

// ---- Client -> Relay --------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, HasSchema)]
pub enum ClientMessage {
    Hello {
        commit_hash: String,
        relay_secret: String,
        identity_name: String,
        identity_secret: String,
        new_identity_secret: Option<String>,
    },
    Input {
        context: String,
        payload: Vec<u8>,
    },
    Disconnect,
}

/// Well-known context identifiers for routing.
/// The relay uses these to determine broadcast scope.
pub mod context {
    /// Chat messages — broadcast to all connected clients regardless of version.
    pub const CHAT: &str = "chat";
}

// ---- Relay -> Client --------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, HasSchema)]
pub enum RelayMessage {
    Welcome { peer_count: u32 },
    NameClaimed,
    RejectSecret,
    RejectVersion { expected: String },
    PeerJoined { name: String },
    PeerLeft { name: String },
    Broadcast { from: String, payload: Vec<u8> },
    ChatHistory { messages: Vec<HistoryEntry> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub from: String,
    pub payload: Vec<u8>,
}

// ---- Application-level payload (clients only, opaque to relay) --------------

#[derive(Debug, Serialize, Deserialize, HasSchema)]
pub enum ChatPayload {
    Text(String),
}

// ---- S3 persistence types ---------------------------------------------------
//
// Shared between relay (writes) and client (reads). JSON-serializable chat
// history with base64-encoded payloads.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use std::collections::VecDeque;

#[derive(Serialize, Deserialize)]
pub struct PersistedHistoryEntry {
    pub from: String,
    pub payload: String,
}

// ---- Chat persistence helpers -----------------------------------------------
//
// Used for reading/writing chat history to S3.
// Chat history lives at admin/chat-history.json (shared across all versions).

/// Convert a single version group to persisted format.
pub fn persist_entries(entries: &VecDeque<HistoryEntry>) -> Vec<PersistedHistoryEntry> {
    entries
        .iter()
        .map(|e| PersistedHistoryEntry {
            from: e.from.clone(),
            payload: BASE64.encode(&e.payload),
        })
        .collect()
}

/// Restore a single version group from persisted format.
/// Entries with invalid base64 are silently skipped.
pub fn restore_entries(persisted: Vec<PersistedHistoryEntry>) -> VecDeque<HistoryEntry> {
    persisted
        .into_iter()
        .filter_map(|e| {
            let payload = BASE64.decode(&e.payload).ok()?;
            Some(HistoryEntry {
                from: e.from,
                payload,
            })
        })
        .collect()
}

// ---- Admin types (shared between relay and arcade-ops) ----------------------
//
// These types define the S3-based admin interface. The relay writes state files
// and polls for command files. arcade-ops reads state and writes commands.

/// Relay heartbeat: written to `admin/heartbeat.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub timestamp: String,
    pub uptime_secs: u64,
    pub client_count: usize,
    pub commit_hash: String,
    pub start_time: String,
    pub total_messages: u64,
    pub messages_since_sync: u64,
}

/// Connected client info for `admin/connected.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedUsers {
    pub timestamp: String,
    pub users: Vec<ConnectedUser>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedUser {
    pub name: String,
    pub commit_hash: String,
    pub idle_secs: u64,
}

/// Registered identities for `admin/identities.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredIdentities {
    pub timestamp: String,
    pub names: Vec<String>,
}

/// Admin command written to `admin/commands/` and polled by the relay.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command")]
pub enum AdminCommand {
    #[serde(rename = "delete-user")]
    DeleteUser { name: String },
    #[serde(rename = "reset-identity")]
    ResetIdentity { name: String },
    #[serde(rename = "broadcast")]
    Broadcast { message: String },
    #[serde(rename = "drain")]
    Drain,
}

// ---- Schema types -----------------------------------------------------------
//
// Informational schema metadata written alongside version data. Not used for
// decode decisions, but helps operators understand format differences across
// versions without needing the source code.

/// Schema for the postcard-serialized payloads. Written to
/// `admin/versions/<hash>/schema.json` on relay startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadSchema {
    pub schema_version: u32,
    pub commit_hash: String,
    pub types: Vec<SchemaType>,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaType {
    pub name: String,
    pub kind: String,
    pub variants: Vec<SchemaVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaVariant {
    pub name: String,
    pub fields: Vec<SchemaField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaField {
    pub name: String,
    pub ty: String,
}

/// Build the current protocol schema from the derived `HasSchema` impls.
pub fn current_payload_schema(commit_hash: &str) -> PayloadSchema {
    let types = vec![
        ClientMessage::schema(),
        RelayMessage::schema(),
        ChatPayload::schema(),
    ];
    let fingerprint = schema_fingerprint(&types);
    PayloadSchema {
        schema_version: 1,
        commit_hash: commit_hash.to_string(),
        types,
        fingerprint,
    }
}

/// FNV-1a hash of the canonical JSON representation of the schema types.
/// Deterministic and stable across platforms without external dependencies.
fn schema_fingerprint(types: &[SchemaType]) -> String {
    let json = serde_json::to_string(types).expect("schema serialization should not fail");
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in json.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

// ---- Serialization helpers --------------------------------------------------

pub fn serialize<T: Serialize>(value: &T) -> Vec<u8> {
    postcard::to_allocvec(value).expect("serialization should not fail")
}

pub fn deserialize<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Option<T> {
    postcard::from_bytes(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // 1. ClientMessage serialization roundtrips
    // ========================================================================

    #[test]
    fn client_hello_all_fields_roundtrip() {
        // given a Hello with every field populated
        let msg = ClientMessage::Hello {
            commit_hash: "abc123".into(),
            relay_secret: "secret".into(),
            identity_name: "alice".into(),
            identity_secret: "id_secret".into(),
            new_identity_secret: Some("new_secret".into()),
        };

        // when we serialize then deserialize
        let bytes = serialize(&msg);
        let result: ClientMessage = deserialize(&bytes).expect("should deserialize");

        // then the fields match
        match result {
            ClientMessage::Hello {
                commit_hash,
                relay_secret,
                identity_name,
                identity_secret,
                new_identity_secret,
            } => {
                assert_eq!(commit_hash, "abc123");
                assert_eq!(relay_secret, "secret");
                assert_eq!(identity_name, "alice");
                assert_eq!(identity_secret, "id_secret");
                assert_eq!(new_identity_secret, Some("new_secret".into()));
            }
            other => panic!("expected Hello, got {:?}", other),
        }
    }

    #[test]
    fn client_hello_no_new_identity_secret_roundtrip() {
        // given a Hello with new_identity_secret = None
        let msg = ClientMessage::Hello {
            commit_hash: "def456".into(),
            relay_secret: "s".into(),
            identity_name: "bob".into(),
            identity_secret: "is".into(),
            new_identity_secret: None,
        };

        // when we serialize then deserialize
        let bytes = serialize(&msg);
        let result: ClientMessage = deserialize(&bytes).expect("should deserialize");

        // then new_identity_secret is None
        match result {
            ClientMessage::Hello {
                new_identity_secret,
                ..
            } => {
                assert_eq!(new_identity_secret, None);
            }
            other => panic!("expected Hello, got {:?}", other),
        }
    }

    #[test]
    fn client_input_with_payload_roundtrip() {
        // given an Input with a non-empty payload
        let msg = ClientMessage::Input {
            context: "chat".into(),
            payload: vec![1, 2, 3, 4, 5],
        };

        // when we serialize then deserialize
        let bytes = serialize(&msg);
        let result: ClientMessage = deserialize(&bytes).expect("should deserialize");

        // then the context and payload match
        match result {
            ClientMessage::Input { context, payload } => {
                assert_eq!(context, "chat");
                assert_eq!(payload, vec![1, 2, 3, 4, 5]);
            }
            other => panic!("expected Input, got {:?}", other),
        }
    }

    #[test]
    fn client_input_empty_payload_roundtrip() {
        // given an Input with an empty payload (keepalive)
        let msg = ClientMessage::Input { context: String::new(), payload: vec![] };

        // when we serialize then deserialize
        let bytes = serialize(&msg);
        let result: ClientMessage = deserialize(&bytes).expect("should deserialize");

        // then the payload is empty
        match result {
            ClientMessage::Input { payload, .. } => {
                assert!(payload.is_empty());
            }
            other => panic!("expected Input, got {:?}", other),
        }
    }

    #[test]
    fn client_disconnect_roundtrip() {
        // given a Disconnect message
        let msg = ClientMessage::Disconnect;

        // when we serialize then deserialize
        let bytes = serialize(&msg);
        let result: ClientMessage = deserialize(&bytes).expect("should deserialize");

        // then it deserializes to Disconnect
        assert!(matches!(result, ClientMessage::Disconnect));
    }

    // ========================================================================
    // 2. RelayMessage serialization roundtrips
    // ========================================================================

    #[test]
    fn relay_welcome_roundtrip() {
        // given a Welcome message
        let msg = RelayMessage::Welcome { peer_count: 42 };

        // when we serialize then deserialize
        let bytes = serialize(&msg);
        let result: RelayMessage = deserialize(&bytes).expect("should deserialize");

        // then peer_count matches
        match result {
            RelayMessage::Welcome { peer_count } => assert_eq!(peer_count, 42),
            other => panic!("expected Welcome, got {:?}", other),
        }
    }

    #[test]
    fn relay_name_claimed_roundtrip() {
        // given a NameClaimed message
        let msg = RelayMessage::NameClaimed;

        // when we serialize then deserialize
        let bytes = serialize(&msg);
        let result: RelayMessage = deserialize(&bytes).expect("should deserialize");

        // then it deserializes to NameClaimed
        assert!(matches!(result, RelayMessage::NameClaimed));
    }

    #[test]
    fn relay_reject_secret_roundtrip() {
        // given a RejectSecret message
        let msg = RelayMessage::RejectSecret;

        // when we serialize then deserialize
        let bytes = serialize(&msg);
        let result: RelayMessage = deserialize(&bytes).expect("should deserialize");

        // then it deserializes to RejectSecret
        assert!(matches!(result, RelayMessage::RejectSecret));
    }

    #[test]
    fn relay_reject_version_roundtrip() {
        // given a RejectVersion message
        let msg = RelayMessage::RejectVersion {
            expected: "v2.0.0".into(),
        };

        // when we serialize then deserialize
        let bytes = serialize(&msg);
        let result: RelayMessage = deserialize(&bytes).expect("should deserialize");

        // then the expected version matches
        match result {
            RelayMessage::RejectVersion { expected } => assert_eq!(expected, "v2.0.0"),
            other => panic!("expected RejectVersion, got {:?}", other),
        }
    }

    #[test]
    fn relay_peer_joined_roundtrip() {
        // given a PeerJoined message
        let msg = RelayMessage::PeerJoined {
            name: "charlie".into(),
        };

        // when we serialize then deserialize
        let bytes = serialize(&msg);
        let result: RelayMessage = deserialize(&bytes).expect("should deserialize");

        // then the name matches
        match result {
            RelayMessage::PeerJoined { name } => assert_eq!(name, "charlie"),
            other => panic!("expected PeerJoined, got {:?}", other),
        }
    }

    #[test]
    fn relay_peer_left_roundtrip() {
        // given a PeerLeft message
        let msg = RelayMessage::PeerLeft {
            name: "dave".into(),
        };

        // when we serialize then deserialize
        let bytes = serialize(&msg);
        let result: RelayMessage = deserialize(&bytes).expect("should deserialize");

        // then the name matches
        match result {
            RelayMessage::PeerLeft { name } => assert_eq!(name, "dave"),
            other => panic!("expected PeerLeft, got {:?}", other),
        }
    }

    #[test]
    fn relay_broadcast_roundtrip() {
        // given a Broadcast message with payload
        let msg = RelayMessage::Broadcast {
            from: "eve".into(),
            payload: vec![10, 20, 30],
        };

        // when we serialize then deserialize
        let bytes = serialize(&msg);
        let result: RelayMessage = deserialize(&bytes).expect("should deserialize");

        // then from and payload match
        match result {
            RelayMessage::Broadcast { from, payload } => {
                assert_eq!(from, "eve");
                assert_eq!(payload, vec![10, 20, 30]);
            }
            other => panic!("expected Broadcast, got {:?}", other),
        }
    }

    // ========================================================================
    // 3. ChatPayload roundtrips
    // ========================================================================

    #[test]
    fn chat_payload_text_with_content_roundtrip() {
        // given a Text payload with content
        let msg = ChatPayload::Text("hello world".into());

        // when we serialize then deserialize
        let bytes = serialize(&msg);
        let result: ChatPayload = deserialize(&bytes).expect("should deserialize");

        // then the text matches
        match result {
            ChatPayload::Text(s) => assert_eq!(s, "hello world"),
        }
    }

    #[test]
    fn chat_payload_text_empty_string_roundtrip() {
        // given a Text payload with an empty string
        let msg = ChatPayload::Text(String::new());

        // when we serialize then deserialize
        let bytes = serialize(&msg);
        let result: ChatPayload = deserialize(&bytes).expect("should deserialize");

        // then the text is empty
        match result {
            ChatPayload::Text(s) => assert!(s.is_empty()),
        }
    }

    // ========================================================================
    // 4. Garbage data returns None
    // ========================================================================

    #[test]
    fn garbage_data_returns_none_for_client_message() {
        // given random garbage bytes
        let garbage = vec![0xFF, 0xFE, 0xFD, 0xFC, 0xFB, 0xFA];

        // when we try to deserialize as ClientMessage
        let result: Option<ClientMessage> = deserialize(&garbage);

        // then we get None
        assert!(result.is_none());
    }

    #[test]
    fn garbage_data_returns_none_for_relay_message() {
        // given random garbage bytes
        let garbage = vec![0xFF, 0xFE, 0xFD, 0xFC, 0xFB, 0xFA];

        // when we try to deserialize as RelayMessage
        let result: Option<RelayMessage> = deserialize(&garbage);

        // then we get None
        assert!(result.is_none());
    }

    #[test]
    fn garbage_data_returns_none_for_chat_payload() {
        // given random garbage bytes
        let garbage = vec![0xFF, 0xFE, 0xFD, 0xFC, 0xFB, 0xFA];

        // when we try to deserialize as ChatPayload
        let result: Option<ChatPayload> = deserialize(&garbage);

        // then we get None
        assert!(result.is_none());
    }

    // ========================================================================
    // 5. Each variant deserializes to the correct discriminant
    // ========================================================================

    #[test]
    fn client_message_variants_not_confused() {
        // given one of each ClientMessage variant
        let hello = ClientMessage::Hello {
            commit_hash: "h".into(),
            relay_secret: "r".into(),
            identity_name: "n".into(),
            identity_secret: "s".into(),
            new_identity_secret: None,
        };
        let input = ClientMessage::Input {
            context: "chat".into(),
            payload: vec![1],
        };
        let disconnect = ClientMessage::Disconnect;

        // when we serialize each
        let hello_bytes = serialize(&hello);
        let input_bytes = serialize(&input);
        let disconnect_bytes = serialize(&disconnect);

        // then each deserializes to its own variant, not another
        let h: ClientMessage = deserialize(&hello_bytes).unwrap();
        assert!(matches!(h, ClientMessage::Hello { .. }));

        let i: ClientMessage = deserialize(&input_bytes).unwrap();
        assert!(matches!(i, ClientMessage::Input { .. }));

        let d: ClientMessage = deserialize(&disconnect_bytes).unwrap();
        assert!(matches!(d, ClientMessage::Disconnect));

        // and Hello bytes do not deserialize to Input or Disconnect discriminants
        assert!(!matches!(
            deserialize::<ClientMessage>(&hello_bytes).unwrap(),
            ClientMessage::Input { .. }
        ));
        assert!(!matches!(
            deserialize::<ClientMessage>(&hello_bytes).unwrap(),
            ClientMessage::Disconnect
        ));
    }

    #[test]
    fn relay_message_variants_not_confused() {
        // given one of each RelayMessage variant
        let variants: Vec<RelayMessage> = vec![
            RelayMessage::Welcome { peer_count: 1 },
            RelayMessage::NameClaimed,
            RelayMessage::RejectSecret,
            RelayMessage::RejectVersion { expected: "v1".into() },
            RelayMessage::PeerJoined { name: "a".into() },
            RelayMessage::PeerLeft { name: "b".into() },
            RelayMessage::Broadcast { from: "c".into(), payload: vec![0] },
            RelayMessage::ChatHistory { messages: vec![HistoryEntry { from: "d".into(), payload: vec![1] }] },
        ];

        // when we serialize then deserialize each
        let roundtripped: Vec<RelayMessage> = variants
            .iter()
            .map(|v| {
                let bytes = serialize(v);
                deserialize::<RelayMessage>(&bytes).unwrap()
            })
            .collect();

        // then each matches its own discriminant
        assert!(matches!(roundtripped[0], RelayMessage::Welcome { .. }));
        assert!(matches!(roundtripped[1], RelayMessage::NameClaimed));
        assert!(matches!(roundtripped[2], RelayMessage::RejectSecret));
        assert!(matches!(roundtripped[3], RelayMessage::RejectVersion { .. }));
        assert!(matches!(roundtripped[4], RelayMessage::PeerJoined { .. }));
        assert!(matches!(roundtripped[5], RelayMessage::PeerLeft { .. }));
        assert!(matches!(roundtripped[6], RelayMessage::Broadcast { .. }));
        assert!(matches!(roundtripped[7], RelayMessage::ChatHistory { .. }));
    }

    // ========================================================================
    // 6. Derived schema correctness
    // ========================================================================

    #[test]
    fn derived_schema_has_correct_type_names() {
        // given the derived schemas
        let client = ClientMessage::schema();
        let relay = RelayMessage::schema();
        let chat = ChatPayload::schema();

        // then type names and kinds are correct
        assert_eq!(client.name, "ClientMessage");
        assert_eq!(client.kind, "enum");
        assert_eq!(relay.name, "RelayMessage");
        assert_eq!(relay.kind, "enum");
        assert_eq!(chat.name, "ChatPayload");
        assert_eq!(chat.kind, "enum");
    }

    #[test]
    fn derived_schema_has_correct_variant_counts() {
        // given the derived schemas
        let client = ClientMessage::schema();
        let relay = RelayMessage::schema();
        let chat = ChatPayload::schema();

        // then variant counts match the actual enums
        assert_eq!(client.variants.len(), 3, "ClientMessage: Hello, Input, Disconnect");
        assert_eq!(relay.variants.len(), 8, "RelayMessage: Welcome..ChatHistory");
        assert_eq!(chat.variants.len(), 1, "ChatPayload: Text");
    }

    #[test]
    fn derived_schema_has_correct_field_names() {
        // given the ClientMessage schema
        let schema = ClientMessage::schema();

        // then Hello variant has the expected fields
        let hello = &schema.variants[0];
        assert_eq!(hello.name, "Hello");
        let field_names: Vec<&str> = hello.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            field_names,
            vec!["commit_hash", "relay_secret", "identity_name", "identity_secret", "new_identity_secret"]
        );

        // and Disconnect has no fields
        let disconnect = &schema.variants[2];
        assert_eq!(disconnect.name, "Disconnect");
        assert!(disconnect.fields.is_empty());
    }

    #[test]
    fn derived_schema_fingerprint_is_stable() {
        // given two calls to current_payload_schema
        let schema1 = current_payload_schema("test");
        let schema2 = current_payload_schema("test");

        // then the fingerprint is the same
        assert_eq!(schema1.fingerprint, schema2.fingerprint);

        // and it's a 16-character hex string
        assert_eq!(schema1.fingerprint.len(), 16);
        assert!(schema1.fingerprint.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
