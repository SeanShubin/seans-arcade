# Bevy Command Timing: Resource Not Available Same Frame

**Date:** 2026-03-07

## Symptom

App crashes on first launch after entering name and relay secret:

```
Encountered an error in system `arcade::net::send_hello`:
Parameter `Res<'_, NetSocket>` failed validation: Resource does not exist
```

## Root Cause

`start_connection()` inserts `NetSocket` via `commands.insert_resource()` and sets `ConnectionState` to `Connecting` in the same frame. Bevy evaluates run conditions (which check `ConnectionState`) before applying queued commands (which insert the resource). So `send_hello` passes its run condition (`is_connecting` returns true) but the `NetSocket` resource doesn't exist yet.

This only happens when `start_connection()` is called from a UI handler (keyboard input system) rather than from `setup_network()` during `Startup`. During `Startup`, commands are applied before `Update` systems run. During `Update`, commands from one system aren't applied until after the current schedule pass.

## How to Debug

1. Enable Bevy's `debug` feature in `Cargo.toml` — without it, panic messages say "Enable the debug feature to see the name" instead of showing the failing system.
2. Set up file logging (custom `LogPlugin` layer writing to `{config-dir}/arcade.log`) so the user can share the log file.
3. The panic message names the exact system and parameter that failed.

## Fix

Change systems that access `NetSocket` from `Res<NetSocket>` to `Option<Res<NetSocket>>` and early-return if `None`:

```rust
fn send_hello(net: Option<Res<NetSocket>>, ...) {
    let Some(net) = net else { return };
    // ...
}
```

This is safe because the run condition already gates when the system is active — the `Option` just handles the one-frame gap where the state changed but the resource hasn't materialized yet.

## Rule

When a system's run condition depends on state set in the same frame as a `commands.insert_resource()`, the resource won't be available until the next frame. Either:

- Use `Option<Res<T>>` on the consumer side (preferred — defensive, no ordering dependency)
- Or ensure the resource is inserted in an earlier schedule stage (e.g., `Startup` vs `Update`)
