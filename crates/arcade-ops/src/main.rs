//! Operator CLI for Sean's Arcade.
//!
//! Single interface for monitoring, management, analytics, and infrastructure
//! control. Reads state from S3, writes commands to S3, and shells out to
//! AWS/SSH/Terraform for infrastructure operations.

mod s3;

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::process::Command;

use protocol::{
    AdminCommand, ChatPayload, ConnectedUsers, Heartbeat, PayloadSchema, PersistedHistoryEntry,
    RegisteredIdentities,
};

const DEFAULT_BUCKET: &str = "arcade.seanshubin.com";
const DEFAULT_RELAY_HOST: &str = "relay.seanshubin.com";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let positional = collect_positional(&args);

    if positional.is_empty() {
        print_usage();
        return;
    }

    let bucket = std::env::var("ARCADE_OPS_BUCKET").unwrap_or_else(|_| DEFAULT_BUCKET.into());

    // Warn if arcade-ops version doesn't match the deployed relay
    let skip_check = matches!(positional[0].as_str(), "update" | "version");
    if !skip_check {
        check_version_mismatch(&bucket);
    }

    match positional[0].as_str() {
        // Observe
        "status" => cmd_status(&bucket, &args),
        "users" => cmd_users(&bucket),
        "identities" => cmd_identities(&bucket),
        "history" => cmd_history(&bucket, &positional[1..]),
        "logs" => cmd_logs(&bucket, &positional[1..]),
        // Control
        "kick" => cmd_kick(&bucket, &positional[1..]),
        "reset-identity" => cmd_reset_identity(&bucket, &positional[1..]),
        "broadcast" => cmd_broadcast(&bucket, &positional[1..]),
        "drain" => cmd_drain(&bucket),
        // Infrastructure
        "relay" => cmd_relay(&positional[1..]),
        "infra" => cmd_infra(&positional[1..]),
        // Analytics
        "stats" => cmd_stats(&bucket, &positional[1..]),
        "uptime" => cmd_uptime(&bucket),
        "versions" => cmd_versions(&bucket),
        "health" => cmd_health(&bucket),
        // Data management
        "data" => cmd_data(&bucket, &positional[1..]),
        // Meta
        "update" => cmd_update(),
        "version" => println!("arcade-ops {}", env!("GIT_COMMIT_HASH")),
        other => {
            eprintln!("Unknown command: {other}");
            print_usage();
        }
    }
}

fn collect_positional(args: &[String]) -> Vec<String> {
    let mut positional = Vec::new();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--data-dir" || args[i] == "--version" {
            i += 2;
        } else if args[i].starts_with("--") {
            // Skip flags like --watch (handled by individual commands)
            i += 1;
        } else {
            positional.push(args[i].clone());
            i += 1;
        }
    }
    positional
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn print_usage() {
    eprintln!("arcade-ops {}", env!("GIT_COMMIT_HASH"));
    eprintln!();
    eprintln!("Observe:");
    eprintln!("  status [--watch]               Relay health (uptime, clients, version)");
    eprintln!("  users                          Connected users with idle times");
    eprintln!("  identities                     Registered identity names");
    eprintln!("  history [--version HASH|--all]  Chat history (default: this version)");
    eprintln!("  logs [FILE|--latest] [--remote] Chat logs (local or S3)");
    eprintln!();
    eprintln!("Control:");
    eprintln!("  kick <user>                    Disconnect and delete user identity");
    eprintln!("  reset-identity <user>          Wipe user's identity secret");
    eprintln!("  broadcast <message>            System message to all clients");
    eprintln!("  drain                          Disconnect all clients");
    eprintln!();
    eprintln!("Infrastructure:");
    eprintln!("  relay restart|redeploy|destroy|ssh");
    eprintln!("  infra plan|apply|destroy");
    eprintln!();
    eprintln!("Analytics:");
    eprintln!("  stats [--version HASH|--all]    Message volume (default: this version)");
    eprintln!("  uptime                         Relay uptime from heartbeat");
    eprintln!("  versions                       Client version distribution");
    eprintln!("  health                         Composite system health check");
    eprintln!();
    eprintln!("Data management:");
    eprintln!("  data versions                  List versions with stored data");
    eprintln!("  data inspect <hash>            View messages for a version");
    eprintln!("  data delete <hash>             Delete all data for a version");
    eprintln!("  data prune                     Delete data for inactive versions");
    eprintln!();
    eprintln!("  update                         Download latest arcade-ops binary");
    eprintln!("  version                        Print arcade-ops version");
    eprintln!();
    eprintln!("Environment:");
    eprintln!("  ARCADE_OPS_BUCKET              S3 bucket (default: {DEFAULT_BUCKET})");
    eprintln!("  RELAY_SSH_KEY                  Path to SSH key for relay commands");
}

// =============================================================================
// Observe commands
// =============================================================================

fn cmd_status(bucket: &str, args: &[String]) {
    let watch = has_flag(args, "--watch");
    loop {
        let s3 = s3::S3Client::new(bucket);
        match s3.get_json::<Heartbeat>("admin/heartbeat.json") {
            Some(hb) => {
                let age = heartbeat_age(&hb.timestamp);
                let status = if age <= 30 { "UP" } else { "DOWN (stale)" };
                println!("Relay:      {status}");
                println!("Uptime:     {}", format_duration(hb.uptime_secs));
                println!("Clients:    {}", hb.client_count);
                println!("Version:    {}", hb.commit_hash);
                println!("Started:    {}", hb.start_time);
                println!("Messages:   {} total, {} since last sync", hb.total_messages, hb.messages_since_sync);
                println!("Last sync:  {age}s ago");
            }
            None => {
                println!("Relay:    DOWN (no heartbeat)");
            }
        }
        if !watch {
            break;
        }
        println!("---");
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}

fn cmd_users(bucket: &str) {
    let s3 = s3::S3Client::new(bucket);
    match s3.get_json::<ConnectedUsers>("admin/connected.json") {
        Some(cu) => {
            if cu.users.is_empty() {
                println!("No connected users.");
                return;
            }
            println!(
                "{:<20} {:<12} {:>8}",
                "NAME", "VERSION", "IDLE"
            );
            for u in &cu.users {
                let hash_short = short_hash(&u.commit_hash);
                println!(
                    "{:<20} {:<12} {:>7}s",
                    u.name, hash_short, u.idle_secs
                );
            }
            println!("\n{} users (as of {})", cu.users.len(), cu.timestamp);
        }
        None => eprintln!("Failed to fetch connected users."),
    }
}

fn cmd_identities(bucket: &str) {
    let s3 = s3::S3Client::new(bucket);
    match s3.get_json::<RegisteredIdentities>("admin/identities.json") {
        Some(ri) => {
            if ri.names.is_empty() {
                println!("No registered identities.");
                return;
            }
            for name in &ri.names {
                println!("{name}");
            }
            println!("\n{} identities (as of {})", ri.names.len(), ri.timestamp);
        }
        None => eprintln!("Failed to fetch identities."),
    }
}

fn cmd_history(bucket: &str, args: &[String]) {
    let s3 = s3::S3Client::new(bucket);

    // --all: show all versions. --version HASH: specific version. Default: arcade-ops version.
    let show_all = args.iter().any(|a| a == "--all");
    let version_filter = args
        .iter()
        .position(|a| a == "--version")
        .and_then(|i| args.get(i + 1).map(|s| s.to_string()));
    let target = if show_all {
        None
    } else {
        Some(version_filter.unwrap_or_else(|| env!("GIT_COMMIT_HASH").to_string()))
    };

    let hashes = list_version_hashes(&s3);
    if hashes.is_empty() {
        println!("No chat history found.");
        return;
    }

    for hash in &hashes {
        if let Some(ref filter) = target {
            if !hash.starts_with(filter.as_str()) {
                continue;
            }
        }

        let key = format!("admin/versions/{hash}/chat-history.json");
        let Some(entries) = s3.get_json::<Vec<PersistedHistoryEntry>>(&key) else {
            continue;
        };

        let hash_short = short_hash(hash);
        println!("--- version {hash_short} ({} messages) ---", entries.len());

        for entry in protocol::restore_entries(entries) {
            if entry.payload.is_empty() {
                continue;
            }
            match protocol::deserialize::<ChatPayload>(&entry.payload) {
                Some(ChatPayload::Text(text)) => {
                    let from = if entry.from.is_empty() {
                        "[system]"
                    } else {
                        &entry.from
                    };
                    println!("  {from}: {text}");
                }
                None => {
                    println!(
                        "  {}: [undecoded: {} bytes]",
                        if entry.from.is_empty() { "[system]" } else { &entry.from },
                        entry.payload.len()
                    );
                }
            }
        }
    }
}

fn cmd_logs(bucket: &str, args: &[String]) {
    if args.iter().any(|a| a == "--remote") {
        cmd_logs_remote(bucket, args);
    } else {
        cmd_logs_local(args);
    }
}

fn cmd_logs_remote(bucket: &str, args: &[String]) {
    let s3 = s3::S3Client::new(bucket);
    let keys = s3.list_keys("admin/logs/");
    let log_keys: Vec<&String> = keys.iter().filter(|k| k.ends_with(".log")).collect();

    if log_keys.is_empty() {
        println!("No remote logs found in S3.");
        return;
    }

    // If a specific filename is given, show that one
    let filename_arg: Option<&String> =
        args.iter().find(|a| *a != "--remote" && *a != "--latest");

    if let Some(filename) = filename_arg {
        let matching: Vec<&&String> = log_keys
            .iter()
            .filter(|k| k.ends_with(filename.as_str()))
            .collect();
        if let Some(key) = matching.first() {
            if let Some(contents) = s3.get_json::<String>(key) {
                print_log_contents(&contents);
            } else {
                eprintln!("Failed to fetch {key} from S3.");
            }
        } else {
            eprintln!("No remote log matching '{filename}'");
        }
        return;
    }

    // --latest: show the last log file
    if args.iter().any(|a| a == "--latest") {
        if let Some(key) = log_keys.last() {
            println!("--- {key} ---");
            if let Some(contents) = s3.get_json::<String>(key) {
                print_log_contents(&contents);
            }
        }
        return;
    }

    // List remote log files
    for key in &log_keys {
        let name = key.strip_prefix("admin/logs/").unwrap_or(key);
        println!("{name}");
    }
}

fn cmd_logs_local(args: &[String]) {
    let log_dir = log_dir_from_args();

    if args.is_empty() {
        list_logs(&log_dir);
        return;
    }

    if args[0] == "--latest" {
        print_latest_log(&log_dir);
        return;
    }

    let path = log_dir.join(&args[0]);
    match std::fs::read_to_string(&path) {
        Ok(contents) => print_log_contents(&contents),
        Err(e) => eprintln!("Error reading {}: {e}", path.display()),
    }
}

// =============================================================================
// Control commands
// =============================================================================

fn cmd_kick(bucket: &str, args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: arcade-ops kick <user>");
        return;
    }
    let name = &args[0];
    let s3 = s3::S3Client::new(bucket);
    let cmd = AdminCommand::DeleteUser {
        name: name.clone(),
    };
    let key = format!("admin/commands/delete-user-{name}.json");
    if s3.put_json(&key, &cmd) {
        println!("Sent kick command for {name}. Relay will execute within ~15s.");
    } else {
        eprintln!("Failed to send kick command.");
    }
}

fn cmd_reset_identity(bucket: &str, args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: arcade-ops reset-identity <user>");
        return;
    }
    let name = &args[0];
    let s3 = s3::S3Client::new(bucket);
    let cmd = AdminCommand::ResetIdentity {
        name: name.clone(),
    };
    let key = format!("admin/commands/reset-identity-{name}.json");
    if s3.put_json(&key, &cmd) {
        println!("Sent reset-identity command for {name}. Relay will execute within ~15s.");
    } else {
        eprintln!("Failed to send reset-identity command.");
    }
}

fn cmd_broadcast(bucket: &str, args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: arcade-ops broadcast <message>");
        return;
    }
    let message = args.join(" ");
    let s3 = s3::S3Client::new(bucket);
    let cmd = AdminCommand::Broadcast {
        message: message.clone(),
    };
    let timestamp = chrono::Utc::now().timestamp();
    let key = format!("admin/commands/broadcast-{timestamp}.json");
    if s3.put_json(&key, &cmd) {
        println!("Sent broadcast: {message}");
    } else {
        eprintln!("Failed to send broadcast command.");
    }
}

fn cmd_drain(bucket: &str) {
    eprint!("This will disconnect ALL clients. Continue? [y/N] ");
    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() || !input.trim().eq_ignore_ascii_case("y") {
        println!("Cancelled.");
        return;
    }
    let s3 = s3::S3Client::new(bucket);
    let cmd = AdminCommand::Drain;
    let key = "admin/commands/drain.json";
    if s3.put_json(key, &cmd) {
        println!("Sent drain command. Relay will disconnect all clients within ~15s.");
    } else {
        eprintln!("Failed to send drain command.");
    }
}

// =============================================================================
// Infrastructure commands
// =============================================================================

fn cmd_relay(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: arcade-ops relay restart|redeploy|destroy|ssh");
        return;
    }

    let relay_host = std::env::var("RELAY_HOST").unwrap_or_else(|_| DEFAULT_RELAY_HOST.into());
    let ssh_key = std::env::var("RELAY_SSH_KEY").unwrap_or_else(|_| {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".into());
        format!("{home}/.ssh/relay-key")
    });
    let ssh_user = std::env::var("RELAY_SSH_USER").unwrap_or_else(|_| "ec2-user".into());
    let ssh_target = format!("{ssh_user}@{relay_host}");
    let ssh_opts = [
        "-o",
        "StrictHostKeyChecking=no",
        "-i",
        &ssh_key,
    ];

    match args[0].as_str() {
        "restart" => {
            println!("Restarting relay container on {relay_host}...");
            run_ssh(&ssh_opts, &ssh_target, "sudo docker restart arcade-relay");
        }
        "redeploy" => {
            println!("Redeploying relay on {relay_host}...");
            run_ssh(
                &ssh_opts,
                &ssh_target,
                "sudo docker stop arcade-relay 2>/dev/null; \
                 sudo docker rm arcade-relay 2>/dev/null; \
                 sudo docker build -t arcade-relay:latest -f /opt/arcade-relay/Dockerfile /opt/arcade-relay && \
                 sudo docker run -d --name arcade-relay --restart unless-stopped --network host \
                   -e RELAY_SECRET=\"$RELAY_SECRET\" \
                   -e S3_BUCKET=\"$S3_BUCKET\" \
                   -e AWS_ACCESS_KEY_ID=\"$AWS_ACCESS_KEY_ID\" \
                   -e AWS_SECRET_ACCESS_KEY=\"$AWS_SECRET_ACCESS_KEY\" \
                   -e AWS_DEFAULT_REGION=\"us-east-1\" \
                   -v /opt/arcade-relay/data:/data \
                   arcade-relay:latest relay --data-dir /data",
            );
        }
        "destroy" => {
            eprint!("This will destroy the relay infrastructure. Continue? [y/N] ");
            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_err()
                || !input.trim().eq_ignore_ascii_case("y")
            {
                println!("Cancelled.");
                return;
            }
            run_terraform(&["destroy", "-target=module.relay", "-auto-approve"]);
        }
        "ssh" => {
            let status = Command::new("ssh")
                .args(&ssh_opts)
                .arg(&ssh_target)
                .status();
            match status {
                Ok(s) if !s.success() => eprintln!("SSH exited with {s}"),
                Err(e) => eprintln!("Failed to launch SSH: {e}"),
                _ => {}
            }
        }
        other => eprintln!("Unknown relay command: {other}"),
    }
}

fn cmd_infra(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: arcade-ops infra plan|apply|destroy");
        return;
    }

    match args[0].as_str() {
        "plan" => run_terraform(&["plan"]),
        "apply" => {
            eprint!("Apply infrastructure changes? [y/N] ");
            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_err()
                || !input.trim().eq_ignore_ascii_case("y")
            {
                println!("Cancelled.");
                return;
            }
            run_terraform(&["apply", "-auto-approve"]);
        }
        "destroy" => {
            eprint!("DESTROY all infrastructure? This is irreversible. Type 'destroy' to confirm: ");
            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_err()
                || input.trim() != "destroy"
            {
                println!("Cancelled.");
                return;
            }
            run_terraform(&["destroy", "-auto-approve"]);
        }
        other => eprintln!("Unknown infra command: {other}"),
    }
}

// =============================================================================
// Analytics commands
// =============================================================================

fn cmd_stats(bucket: &str, args: &[String]) {
    let s3 = s3::S3Client::new(bucket);
    let all_hashes = list_version_hashes(&s3);

    if all_hashes.is_empty() {
        println!("No data found.");
        return;
    }

    // --all: all versions. --version HASH: specific. Default: arcade-ops version.
    let show_all = args.iter().any(|a| a == "--all");
    let version_filter = args
        .iter()
        .position(|a| a == "--version")
        .and_then(|i| args.get(i + 1).map(|s| s.to_string()));
    let target = if show_all {
        None
    } else {
        Some(version_filter.unwrap_or_else(|| env!("GIT_COMMIT_HASH").to_string()))
    };

    let hashes: Vec<&String> = all_hashes
        .iter()
        .filter(|h| target.as_ref().map_or(true, |t| h.starts_with(t.as_str())))
        .collect();

    if hashes.is_empty() {
        println!("No data found for specified version.");
        return;
    }

    let mut total_messages = 0usize;
    let mut total_users: HashSet<String> = HashSet::new();

    for hash in &hashes {
        let key = format!("admin/versions/{hash}/chat-history.json");
        if let Some(entries) = s3.get_json::<Vec<PersistedHistoryEntry>>(&key) {
            total_messages += entries.len();
            for entry in &entries {
                if !entry.from.is_empty() {
                    total_users.insert(entry.from.clone());
                }
            }
        }
    }

    println!("Total messages:  {total_messages}");
    println!("Unique senders:  {}", total_users.len());
    println!("Version groups:  {}", hashes.len());

    if hashes.len() > 1 {
        println!("\nPer version:");
        println!("{:<12} {:>8}", "VERSION", "MESSAGES");
        for hash in &hashes {
            let key = format!("admin/versions/{hash}/chat-history.json");
            if let Some(entries) = s3.get_json::<Vec<PersistedHistoryEntry>>(&key) {
                let hash_short = short_hash(hash);
                println!("{:<12} {:>8}", hash_short, entries.len());
            }
        }
    }
}

fn cmd_uptime(bucket: &str) {
    let s3 = s3::S3Client::new(bucket);
    match s3.get_json::<Heartbeat>("admin/heartbeat.json") {
        Some(hb) => {
            let age = heartbeat_age(&hb.timestamp);
            let status = if age <= 30 { "UP" } else { "DOWN" };
            println!("Status:   {status}");
            println!("Uptime:   {}", format_duration(hb.uptime_secs));
            println!("Started:  {}", hb.start_time);
            println!("Version:  {}", hb.commit_hash);
            println!("Messages: {} total", hb.total_messages);
            if age > 30 {
                println!("Warning: heartbeat is {age}s old (threshold: 30s)");
            }
        }
        None => println!("No heartbeat found. Relay may be down or S3 unavailable."),
    }
}

fn cmd_versions(bucket: &str) {
    let s3 = s3::S3Client::new(bucket);
    match s3.get_json::<ConnectedUsers>("admin/connected.json") {
        Some(cu) => {
            let mut by_version: HashMap<String, Vec<String>> = HashMap::new();
            for u in &cu.users {
                by_version
                    .entry(u.commit_hash.clone())
                    .or_default()
                    .push(u.name.clone());
            }

            if by_version.is_empty() {
                println!("No connected clients.");
                return;
            }

            for (hash, users) in &by_version {
                let hash_short = short_hash(hash);
                println!("{hash_short}: {} clients", users.len());
                for name in users {
                    println!("  {name}");
                }
            }
        }
        None => eprintln!("Failed to fetch connected users."),
    }
}

fn cmd_health(bucket: &str) {
    let mut all_ok = true;

    // Check relay heartbeat
    let s3 = s3::S3Client::new(bucket);
    match s3.get_json::<Heartbeat>("admin/heartbeat.json") {
        Some(hb) => {
            let age = heartbeat_age(&hb.timestamp);
            if age <= 30 {
                println!("[OK]   Relay heartbeat ({age}s ago)");
            } else {
                println!("[FAIL] Relay heartbeat is {age}s old (threshold: 30s)");
                all_ok = false;
            }
        }
        None => {
            println!("[FAIL] No relay heartbeat found");
            all_ok = false;
        }
    }

    // Check S3 admin directory exists
    let keys = s3.list_keys("admin/");
    if keys.is_empty() {
        println!("[FAIL] No admin data in S3");
        all_ok = false;
    } else {
        println!("[OK]   S3 admin data ({} keys)", keys.len());
    }

    // Check DNS resolution
    match std::net::ToSocketAddrs::to_socket_addrs(&(DEFAULT_RELAY_HOST, 7700)) {
        Ok(_) => println!("[OK]   DNS resolves {DEFAULT_RELAY_HOST}"),
        Err(e) => {
            println!("[FAIL] DNS resolution failed for {DEFAULT_RELAY_HOST}: {e}");
            all_ok = false;
        }
    }

    println!();
    if all_ok {
        println!("All checks passed.");
    } else {
        println!("Some checks failed.");
        std::process::exit(1);
    }
}

// =============================================================================
// Data management commands
// =============================================================================

fn cmd_data(bucket: &str, args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: arcade-ops data versions|inspect|delete|prune");
        return;
    }

    match args[0].as_str() {
        "versions" => data_versions(bucket),
        "inspect" => {
            if args.len() < 2 {
                eprintln!("Usage: arcade-ops data inspect <hash>");
                return;
            }
            data_inspect(bucket, &args[1]);
        }
        "delete" => {
            if args.len() < 2 {
                eprintln!("Usage: arcade-ops data delete <hash>");
                return;
            }
            data_delete(bucket, &args[1]);
        }
        "prune" => data_prune(bucket),
        other => eprintln!("Unknown data command: {other}"),
    }
}

fn data_versions(bucket: &str) {
    let s3 = s3::S3Client::new(bucket);
    let hashes = list_version_hashes(&s3);

    if hashes.is_empty() {
        println!("No version data found.");
        return;
    }

    // Get current connected versions for comparison
    let connected_hashes: HashSet<String> = s3
        .get_json::<ConnectedUsers>("admin/connected.json")
        .map(|cu| cu.users.iter().map(|u| u.commit_hash.clone()).collect())
        .unwrap_or_default();

    let current_hash = env!("GIT_COMMIT_HASH");

    // Compute our schema fingerprint for comparison
    let our_schema = protocol::current_payload_schema(current_hash);

    println!(
        "{:<3} {:<12} {:>8} {:>10}  {:<18}",
        "", "HASH", "MESSAGES", "SIZE", "SCHEMA"
    );

    for hash in &hashes {
        let key = format!("admin/versions/{hash}/chat-history.json");
        let msg_count = s3
            .get_json::<Vec<PersistedHistoryEntry>>(&key)
            .map(|e| e.len())
            .unwrap_or(0);

        let size = s3.prefix_size(&format!("admin/versions/{hash}/"));
        let hash_short = short_hash(hash);

        let schema_key = format!("admin/versions/{hash}/schema.json");
        let schema_label = match s3.get_json::<PayloadSchema>(&schema_key) {
            Some(schema) if schema.fingerprint == our_schema.fingerprint => "compatible".into(),
            Some(schema) => format!("differs ({})", short_hash(&schema.fingerprint)),
            None => "no schema".into(),
        };

        let marker = if hash == current_hash {
            "*"
        } else if connected_hashes.contains(hash) {
            "+"
        } else {
            ""
        };

        println!(
            "{:<3} {:<12} {:>8} {:>10}  {:<18}",
            marker,
            hash_short,
            msg_count,
            format_bytes(size),
            schema_label
        );
    }

    println!();
    println!("* = current arcade-ops version, + = has connected clients");
}

fn data_inspect(bucket: &str, hash_prefix: &str) {
    let s3 = s3::S3Client::new(bucket);
    let hashes = list_version_hashes(&s3);

    // Find matching hash (prefix match)
    let matching: Vec<&String> = hashes
        .iter()
        .filter(|h| h.starts_with(hash_prefix))
        .collect();

    let hash = match matching.len() {
        0 => {
            eprintln!("No version found matching '{hash_prefix}'");
            return;
        }
        1 => matching[0],
        _ => {
            eprintln!("Ambiguous prefix '{hash_prefix}', matches:");
            for h in &matching {
                let short = short_hash(h);
                eprintln!("  {short}");
            }
            return;
        }
    };

    // Show schema info if available
    let schema_key = format!("admin/versions/{hash}/schema.json");
    let our_schema = protocol::current_payload_schema(env!("GIT_COMMIT_HASH"));
    match s3.get_json::<PayloadSchema>(&schema_key) {
        Some(schema) => {
            let compat = if schema.fingerprint == our_schema.fingerprint {
                "compatible"
            } else {
                "different"
            };
            let hash_short = short_hash(hash);
            println!("Version:     {hash_short}");
            println!("Schema:      {compat} (fingerprint: {})", short_hash(&schema.fingerprint));
            println!("Types:       {}", schema.types.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(", "));
            println!();
        }
        None => {
            let hash_short = short_hash(hash);
            println!("Version:     {hash_short} (no schema metadata)");
            println!();
        }
    }

    let key = format!("admin/versions/{hash}/chat-history.json");
    let Some(entries) = s3.get_json::<Vec<PersistedHistoryEntry>>(&key) else {
        eprintln!("No chat history for version {hash_prefix}");
        return;
    };

    let restored = protocol::restore_entries(entries);
    let mut decoded = 0usize;
    let mut undecoded = 0usize;

    for entry in &restored {
        if entry.payload.is_empty() {
            continue;
        }
        let from = if entry.from.is_empty() {
            "[system]"
        } else {
            &entry.from
        };
        match protocol::deserialize::<ChatPayload>(&entry.payload) {
            Some(ChatPayload::Text(text)) => {
                println!("  {from}: {text}");
                decoded += 1;
            }
            None => {
                println!("  {from}: [undecoded: {} bytes]", entry.payload.len());
                undecoded += 1;
            }
        }
    }

    println!();
    println!(
        "{} messages: {decoded} decoded, {undecoded} undecoded",
        decoded + undecoded
    );
}

fn data_delete(bucket: &str, hash_prefix: &str) {
    let s3 = s3::S3Client::new(bucket);
    let hashes = list_version_hashes(&s3);

    let matching: Vec<&String> = hashes
        .iter()
        .filter(|h| h.starts_with(hash_prefix))
        .collect();

    let hash = match matching.len() {
        0 => {
            eprintln!("No version found matching '{hash_prefix}'");
            return;
        }
        1 => matching[0],
        _ => {
            eprintln!("Ambiguous prefix '{hash_prefix}', matches:");
            for h in &matching {
                let short = short_hash(h);
                eprintln!("  {short}");
            }
            return;
        }
    };

    let prefix = format!("admin/versions/{hash}/");
    let keys = s3.list_keys(&prefix);

    if keys.is_empty() {
        println!("No data found for version {hash_prefix}.");
        return;
    }

    eprint!(
        "Delete {} files for version {}? [y/N] ",
        keys.len(),
        short_hash(hash)
    );
    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err()
        || !input.trim().eq_ignore_ascii_case("y")
    {
        println!("Cancelled.");
        return;
    }

    let mut deleted = 0;
    for key in &keys {
        if s3.delete(key) {
            deleted += 1;
        }
    }
    println!("Deleted {deleted}/{} files.", keys.len());
}

fn data_prune(bucket: &str) {
    let s3 = s3::S3Client::new(bucket);
    let hashes = list_version_hashes(&s3);

    // Protect versions that are connected or deployed
    let mut protected: HashSet<String> = s3
        .get_json::<ConnectedUsers>("admin/connected.json")
        .map(|cu| cu.users.iter().map(|u| u.commit_hash.clone()).collect())
        .unwrap_or_default();

    if let Some(relay_hash) = deployed_relay_hash(&s3) {
        protected.insert(relay_hash);
    }

    let stale: Vec<&String> = hashes
        .iter()
        .filter(|h| !protected.contains(h.as_str()))
        .collect();

    if stale.is_empty() {
        println!("No stale versions to prune.");
        return;
    }

    println!("Stale versions (no connected clients, not deployed):");
    for hash in &stale {
        let short = short_hash(hash);
        println!("  {short}");
    }

    eprint!("\nDelete data for {} stale versions? [y/N] ", stale.len());
    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err()
        || !input.trim().eq_ignore_ascii_case("y")
    {
        println!("Cancelled.");
        return;
    }

    let mut total_deleted = 0;
    for hash in &stale {
        let prefix = format!("admin/versions/{hash}/");
        let keys = s3.list_keys(&prefix);
        for key in &keys {
            if s3.delete(key) {
                total_deleted += 1;
            }
        }
        let short = short_hash(hash);
        println!("  Deleted {} files for {short}", keys.len());
    }
    println!("Total: {total_deleted} files deleted.");
}

// =============================================================================
// Local log commands (from original arcade-cli)
// =============================================================================

fn data_dir_from_args() -> std::path::PathBuf {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len().saturating_sub(1) {
        if args[i] == "--data-dir" {
            return std::path::PathBuf::from(&args[i + 1]);
        }
    }
    std::path::PathBuf::from(".")
}

fn log_dir_from_args() -> std::path::PathBuf {
    data_dir_from_args().join("logs")
}

fn print_log_contents(contents: &str) {
    use chrono::{Local, TimeZone};
    for line in contents.lines() {
        let Some((timestamp_str, rest)) = line.split_once(' ') else {
            println!("{line}");
            continue;
        };
        let Ok(secs) = timestamp_str.parse::<i64>() else {
            println!("{line}");
            continue;
        };
        let dt = Local.timestamp_opt(secs, 0).single();
        match dt {
            Some(dt) => println!("{} {rest}", dt.format("%Y-%m-%d %H:%M:%S")),
            None => println!("{line}"),
        }
    }
}

fn list_logs(log_dir: &std::path::Path) {
    let entries = match std::fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Error reading {}: {e}", log_dir.display());
            return;
        }
    };

    let mut files: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "log")
        })
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();

    files.sort();

    if files.is_empty() {
        println!("No log files found in {}", log_dir.display());
    } else {
        for f in &files {
            println!("{f}");
        }
    }
}

fn print_latest_log(log_dir: &std::path::Path) {
    let entries = match std::fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Error reading {}: {e}", log_dir.display());
            return;
        }
    };

    let latest = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "log")
        })
        .max_by_key(|e| e.metadata().ok().and_then(|m| m.modified().ok()));

    match latest {
        Some(entry) => {
            let path = entry.path();
            println!("--- {} ---", path.display());
            match std::fs::read_to_string(&path) {
                Ok(contents) => print_log_contents(&contents),
                Err(e) => eprintln!("Error reading {}: {e}", path.display()),
            }
        }
        None => println!("No log files found in {}", log_dir.display()),
    }
}

// =============================================================================
// Self-update
// =============================================================================

const DOWNLOAD_BASE_URL: &str = "https://arcade.seanshubin.com";

fn cmd_update() {
    let current_exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Failed to determine current executable path: {e}");
            return;
        }
    };

    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };

    let binary_name = if cfg!(target_os = "windows") {
        "arcade-ops.exe"
    } else {
        "arcade-ops"
    };

    let url = format!("{DOWNLOAD_BASE_URL}/{platform}/{binary_name}");
    println!("Downloading from {url}...");

    let bytes = match download_binary(&url) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Download failed: {e}");
            return;
        }
    };

    // Replace the running executable.
    // On Windows, rename the current exe first (can't overwrite a running exe).
    let backup = current_exe.with_extension("old");
    if cfg!(target_os = "windows") {
        if let Err(e) = std::fs::rename(&current_exe, &backup) {
            eprintln!("Failed to rename current binary: {e}");
            return;
        }
    }

    if let Err(e) = std::fs::write(&current_exe, &bytes) {
        eprintln!("Failed to write new binary: {e}");
        // Restore backup on Windows
        if cfg!(target_os = "windows") {
            let _ = std::fs::rename(&backup, &current_exe);
        }
        return;
    }

    // Set executable permission on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&current_exe, std::fs::Permissions::from_mode(0o755));
    }

    // Clean up backup
    let _ = std::fs::remove_file(&backup);

    println!("Updated arcade-ops at {}", current_exe.display());
}

fn download_binary(url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut response = ureq::get(url).call()?;
    let mut bytes = Vec::new();
    response.body_mut().as_reader().read_to_end(&mut bytes)?;
    Ok(bytes)
}

// =============================================================================
// Helpers
// =============================================================================

fn check_version_mismatch(bucket: &str) {
    let s3 = s3::S3Client::new(bucket);
    if let Some(hb) = s3.get_json::<Heartbeat>("admin/heartbeat.json") {
        let ops_hash = env!("GIT_COMMIT_HASH");
        if hb.commit_hash != ops_hash {
            eprintln!(
                "WARNING: arcade-ops ({}) does not match deployed relay ({})",
                short_hash(ops_hash),
                short_hash(&hb.commit_hash)
            );
            eprintln!("  To update: arcade-ops update");
            eprintln!();
        }
    }
}

/// Get the deployed relay's commit hash from the heartbeat, if available.
fn deployed_relay_hash(s3: &s3::S3Client) -> Option<String> {
    s3.get_json::<Heartbeat>("admin/heartbeat.json")
        .map(|hb| hb.commit_hash)
}

fn short_hash(hash: &str) -> &str {
    const HASH_DISPLAY_LEN: usize = 8;
    if hash.len() > HASH_DISPLAY_LEN {
        &hash[..HASH_DISPLAY_LEN]
    } else {
        hash
    }
}

fn list_version_hashes(s3: &s3::S3Client) -> Vec<String> {
    let keys = s3.list_keys("admin/versions/");
    let hashes: HashSet<String> = keys
        .iter()
        .filter_map(|k| {
            k.strip_prefix("admin/versions/")
                .and_then(|rest| rest.split('/').next())
                .filter(|h| !h.is_empty())
                .map(|h| h.to_string())
        })
        .collect();
    let mut sorted: Vec<String> = hashes.into_iter().collect();
    sorted.sort();
    sorted
}

fn heartbeat_age(timestamp: &str) -> u64 {
    let Ok(ts) = chrono::DateTime::parse_from_rfc3339(timestamp) else {
        return u64::MAX;
    };
    let now = chrono::Utc::now();
    now.signed_duration_since(ts)
        .num_seconds()
        .max(0) as u64
}

fn format_duration(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m {seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn run_ssh(ssh_opts: &[&str], target: &str, command: &str) {
    let status = Command::new("ssh")
        .args(ssh_opts)
        .arg(target)
        .arg(command)
        .status();
    match status {
        Ok(s) if !s.success() => eprintln!("SSH command exited with {s}"),
        Err(e) => eprintln!("Failed to launch SSH: {e}"),
        _ => {}
    }
}

fn run_terraform(args: &[&str]) {
    let infra_dir = std::env::var("ARCADE_OPS_INFRA_DIR").unwrap_or_else(|_| "infra".into());
    let status = Command::new("terraform")
        .args(args)
        .current_dir(&infra_dir)
        .status();
    match status {
        Ok(s) if !s.success() => eprintln!("Terraform exited with {s}"),
        Err(e) => eprintln!("Failed to launch terraform: {e}"),
        _ => {}
    }
}
