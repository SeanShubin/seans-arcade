//! Optional S3 client for state persistence.
//!
//! All operations are best-effort: failures are logged but never crash the relay.
//! S3 is the canonical store for persisted state. The relay's in-memory buffer
//! is a write cache that gets flushed to S3 periodically. If S3 is unavailable,
//! the relay keeps running and recreates admin files on the next successful sync.

use std::time::Duration;

use aws_sdk_s3::Client;
use tokio::runtime::Runtime;

// Re-export admin types from protocol (shared with arcade-ops).
pub use protocol::{
    AdminCommand, ConnectedUser, ConnectedUsers, Heartbeat, RegisteredIdentities,
};

/// Best-effort S3 client. Every operation logs errors and returns None/false
/// on failure — never panics or propagates errors.
pub struct S3Client {
    client: Client,
    runtime: Runtime,
    bucket: String,
}

impl S3Client {
    /// Create a new S3 client. Returns None if the runtime fails to start.
    /// Credentials and region come from standard AWS environment variables
    /// (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_DEFAULT_REGION).
    pub fn new(bucket: String) -> Option<Self> {
        let runtime = Runtime::new()
            .map_err(|e| eprintln!("relay: s3: failed to create runtime: {e}"))
            .ok()?;
        let config = runtime.block_on(
            aws_config::defaults(aws_config::BehaviorVersion::latest())
                .timeout_config(
                    aws_sdk_s3::config::timeout::TimeoutConfig::builder()
                        .operation_timeout(Duration::from_secs(10))
                        .build(),
                )
                .load(),
        );
        let client = Client::new(&config);
        Some(Self {
            client,
            runtime,
            bucket,
        })
    }

    /// Read and deserialize a JSON object from S3. Returns None on any failure.
    pub fn get_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let result = self
            .runtime
            .block_on(self.client.get_object().bucket(&self.bucket).key(key).send());
        match result {
            Ok(output) => {
                let bytes = self
                    .runtime
                    .block_on(output.body.collect())
                    .map_err(|e| eprintln!("relay: s3: failed to read body for {key}: {e}"))
                    .ok()?
                    .into_bytes();
                serde_json::from_slice(&bytes)
                    .map_err(|e| eprintln!("relay: s3: failed to parse {key}: {e}"))
                    .ok()
            }
            Err(e) => {
                let is_not_found = e
                    .as_service_error()
                    .map_or(false, |se| se.is_no_such_key());
                if !is_not_found {
                    eprintln!("relay: s3: failed to get {key}: {e}");
                }
                None
            }
        }
    }

    /// Serialize and write a JSON object to S3. Returns true on success.
    pub fn put_json<T: serde::Serialize>(&self, key: &str, value: &T) -> bool {
        let json = match serde_json::to_vec(value) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("relay: s3: failed to serialize {key}: {e}");
                return false;
            }
        };
        let result = self.runtime.block_on(
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(key)
                .body(json.into())
                .content_type("application/json")
                .cache_control("no-cache")
                .send(),
        );
        if let Err(e) = result {
            eprintln!("relay: s3: failed to put {key}: {e}");
            return false;
        }
        true
    }

    /// List all object keys under a prefix. Returns empty vec on failure.
    pub fn list_keys(&self, prefix: &str) -> Vec<String> {
        let result = self.runtime.block_on(
            self.client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(prefix)
                .send(),
        );
        match result {
            Ok(output) => output
                .contents()
                .iter()
                .filter_map(|obj| obj.key().map(|k| k.to_string()))
                .collect(),
            Err(e) => {
                eprintln!("relay: s3: failed to list {prefix}: {e}");
                Vec::new()
            }
        }
    }

    /// Delete an object from S3. Returns true on success.
    pub fn delete(&self, key: &str) -> bool {
        let result = self.runtime.block_on(
            self.client
                .delete_object()
                .bucket(&self.bucket)
                .key(key)
                .send(),
        );
        if let Err(e) = result {
            eprintln!("relay: s3: failed to delete {key}: {e}");
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use protocol::{HistoryEntry, PersistedHistoryEntry};
    use std::collections::VecDeque;

    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as BASE64;

    #[test]
    fn persisted_history_roundtrip() {
        // given an in-memory chat history
        let mut entries = VecDeque::new();
        entries.push_back(HistoryEntry {
            from: "alice".into(),
            payload: vec![1, 2, 3],
        });
        entries.push_back(HistoryEntry {
            from: "bob".into(),
            payload: vec![4, 5, 6],
        });

        // when we convert to persisted format and back
        let persisted = protocol::persist_entries(&entries);
        let json = serde_json::to_string(&persisted).unwrap();
        let restored: Vec<PersistedHistoryEntry> = serde_json::from_str(&json).unwrap();
        let result = protocol::restore_entries(restored);

        // then the history matches
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].from, "alice");
        assert_eq!(result[0].payload, vec![1, 2, 3]);
        assert_eq!(result[1].from, "bob");
        assert_eq!(result[1].payload, vec![4, 5, 6]);
    }

    #[test]
    fn persisted_history_empty_roundtrip() {
        // given an empty chat history
        let entries: VecDeque<HistoryEntry> = VecDeque::new();

        // when we convert to persisted format and back
        let persisted = protocol::persist_entries(&entries);
        let result = protocol::restore_entries(persisted);

        // then the result is empty
        assert!(result.is_empty());
    }

    #[test]
    fn invalid_base64_entries_are_skipped() {
        // given persisted entries with an invalid base64 entry
        let persisted = vec![
            PersistedHistoryEntry {
                from: "alice".into(),
                payload: BASE64.encode(&[1, 2, 3]),
            },
            PersistedHistoryEntry {
                from: "bob".into(),
                payload: "not valid base64!!!".into(),
            },
            PersistedHistoryEntry {
                from: "charlie".into(),
                payload: BASE64.encode(&[7, 8, 9]),
            },
        ];

        // when we restore to memory
        let result = protocol::restore_entries(persisted);

        // then the valid entries are kept and the invalid one is skipped
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].from, "alice");
        assert_eq!(result[1].from, "charlie");
    }
}
