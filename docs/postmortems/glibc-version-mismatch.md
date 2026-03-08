# GLIBC Version Mismatch: Relay Crash-Loops on Lightsail

**Date:** 2026-03-08

## Symptom

Client connects but stays at `"Alice | Connecting..."` indefinitely. The relay never responds.

## Investigation

1. **Client side:** App launches, version check passes, UI renders. Status bar shows `"Alice | Connecting..."` — the Hello packets are being sent but no Welcome comes back.
2. **Relay side:** `sudo docker ps` — container status is `Restarting (1) 11 seconds ago`. The relay is crash-looping.
3. **Relay logs:** `sudo docker logs arcade-relay` — every attempt prints:
   ```
   relay: /lib/x86_64-linux-gnu/libc.so.6: version `GLIBC_2.38' not found (required by relay)
   relay: /lib/x86_64-linux-gnu/libc.so.6: version `GLIBC_2.39' not found (required by relay)
   ```

## Root Cause

The relay binary is built on `ubuntu-latest` in GitHub Actions, which uses Ubuntu 24.04 (glibc 2.39). The Docker image was `debian:bookworm-slim` (Debian 12), which ships glibc 2.36. The binary was dynamically linked against a newer glibc than the container provided.

This broke when `ubuntu-latest` rolled forward to a newer Ubuntu version. The previous CI runs built against an older glibc that was compatible with Bookworm.

## Why It Was Hard to Find

- The client gave no error — it just showed "Connecting..." as if the relay were unreachable or slow.
- The relay container had `--restart unless-stopped`, so it kept restarting and `docker ps` showed it as running (with the `Restarting` status only visible if you read the full output).
- The glibc error is a dynamic linker failure that happens before `main()`, so no application-level logging could catch it.

## Fix

Changed `Dockerfile.relay` from `debian:bookworm-slim` to `debian:trixie-slim` (Debian 13), which ships glibc 2.40 and covers both 2.38 and 2.39.

## Rule

The Docker base image's glibc version must be >= the glibc version on the CI build runner. When `ubuntu-latest` rolls forward, it can silently break containers built on older Debian base images. Either:

- Pin the CI runner to a specific Ubuntu version (e.g., `ubuntu-24.04` instead of `ubuntu-latest`)
- Use a Docker base image that tracks the same or newer glibc (e.g., `debian:trixie-slim` or `ubuntu:24.04`)
- Build a fully static binary (musl) to eliminate the glibc dependency entirely
