//! S3 client for arcade-ops.
//!
//! Same pattern as the relay's S3 client but panics on initialization failure
//! (since arcade-ops can't do anything useful without S3 for most commands).

use std::time::Duration;

use aws_sdk_s3::Client;
use tokio::runtime::Runtime;

pub struct S3Client {
    client: Client,
    runtime: Runtime,
    bucket: String,
}

impl S3Client {
    pub fn new(bucket: &str) -> Self {
        let runtime = Runtime::new().expect("failed to create tokio runtime");
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
        Self {
            client,
            runtime,
            bucket: bucket.to_string(),
        }
    }

    pub fn get_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let result = self
            .runtime
            .block_on(self.client.get_object().bucket(&self.bucket).key(key).send());
        match result {
            Ok(output) => {
                let bytes = self
                    .runtime
                    .block_on(output.body.collect())
                    .map_err(|e| eprintln!("s3: failed to read body for {key}: {e}"))
                    .ok()?
                    .into_bytes();
                serde_json::from_slice(&bytes)
                    .map_err(|e| eprintln!("s3: failed to parse {key}: {e}"))
                    .ok()
            }
            Err(e) => {
                // NoSuchKey is expected for missing keys — don't log it.
                let is_not_found = e
                    .as_service_error()
                    .map_or(false, |se| se.is_no_such_key());
                if !is_not_found {
                    eprintln!("s3: failed to get {key}: {e}");
                }
                None
            }
        }
    }

    pub fn put_json<T: serde::Serialize>(&self, key: &str, value: &T) -> bool {
        let json = match serde_json::to_vec(value) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("s3: failed to serialize {key}: {e}");
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
            eprintln!("s3: failed to put {key}: {e}");
            return false;
        }
        true
    }

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
                eprintln!("s3: failed to list {prefix}: {e}");
                Vec::new()
            }
        }
    }

    pub fn delete(&self, key: &str) -> bool {
        let result = self.runtime.block_on(
            self.client
                .delete_object()
                .bucket(&self.bucket)
                .key(key)
                .send(),
        );
        if let Err(e) = result {
            eprintln!("s3: failed to delete {key}: {e}");
            return false;
        }
        true
    }

    /// Get raw bytes and content length for a key. Returns None on failure.
    pub fn get_size(&self, key: &str) -> Option<u64> {
        let result = self.runtime.block_on(
            self.client
                .head_object()
                .bucket(&self.bucket)
                .key(key)
                .send(),
        );
        match result {
            Ok(output) => output.content_length().map(|l| l as u64),
            Err(_) => None,
        }
    }

    /// List keys and return total size of all objects under a prefix.
    pub fn prefix_size(&self, prefix: &str) -> u64 {
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
                .filter_map(|obj| obj.size())
                .map(|s| s as u64)
                .sum(),
            Err(_) => 0,
        }
    }
}
