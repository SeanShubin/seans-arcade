# Hostname Not Resolved: Client Silently Connects to Localhost

**Date:** 2026-03-08

## Symptom

Client prints `relay secret entered, connecting...` then immediately floods stderr with:

```
arcade: recv error: An existing connection was forcibly closed by the remote host. (os error 10054)
```

The relay shows zero incoming packets. The relay is running, the firewall is open, DNS resolves correctly, and a manual UDP test from the same Windows machine reaches the VM.

## Investigation

1. **Relay side:** `sudo docker ps` — container running. `sudo docker logs arcade-relay` — listening on `0.0.0.0:7700`. No errors.
2. **Firewall:** `aws lightsail get-instance-port-states` — UDP 7700 open to `0.0.0.0/0`.
3. **DNS:** `nslookup relay.seanshubin.com` — resolves to the correct static IP on both client and server.
4. **Packet capture:** `sudo tcpdump -i any udp port 7700 -n` on the VM — zero packets when the client runs.
5. **Manual UDP test:** PowerShell `UdpClient.Send()` to `relay.seanshubin.com:7700` — tcpdump sees the packet arrive. So the network path works.
6. **Conclusion:** The client is not sending to the relay. It's sending somewhere else.

## Root Cause

`config.toml` had `relay_address = "relay.seanshubin.com:7700"`. The client parsed this with `SocketAddr::parse()`, which only accepts `IP:port` — it does not perform DNS resolution. The parse failed:

- In `setup_network()` (startup with relay secret already saved): `.expect()` would panic.
- In `handle_connect_request()` (after entering relay secret at runtime): `.unwrap_or_else()` silently fell back to `127.0.0.1:7700`.

The client was sending UDP packets to localhost. Windows error 10054 (`WSAECONNRESET`) is the OS reporting that nothing is listening on that local port — the ICMP "port unreachable" response from the loopback interface.

## Why It Was Hard to Find

- The fallback to `127.0.0.1:7700` was silent — no log, no warning.
- Error 10054 looks like a network/firewall problem, directing investigation at the relay and infrastructure.
- The manual UDP test proved the network path was fine, which narrowed the problem to the client itself.
- The relay showing zero packets was the key clue — combined with the working manual test, it meant the client was sending to the wrong destination.

## Fix

Replace `SocketAddr::parse()` with `ToSocketAddrs::to_socket_addrs()`, which performs DNS resolution:

```rust
use std::net::ToSocketAddrs;

let relay_addr = config
    .relay_address
    .to_socket_addrs()
    .expect("failed to resolve relay address")
    .next()
    .expect("relay address resolved to no addresses");
```

## Rule

`SocketAddr::parse()` (and `.parse::<SocketAddr>()`) only accepts numeric `IP:port` strings. To support hostnames, use `ToSocketAddrs`. Any place that reads a user-configured address string and might contain a hostname must use `ToSocketAddrs`, not `SocketAddr::parse()`.
