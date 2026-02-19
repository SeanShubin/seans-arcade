# gilrs Dual Gamepad Bug on Windows

Investigation into why the dashboard's second gamepad panel permanently shows "No gamepad detected" when both controllers are plugged in before launch.

## Environment

- Windows 11 Home (build 26100)
- Bevy 0.18.0
- bevy_gilrs 0.18.0
- gilrs 0.11.1
- gilrs-core 0.6.7
- Controllers: Xbox One Game Controller + Xbox 360 Game Controller

## The Input Stack

```
Application code (Query<&Gamepad>)
        |
    bevy_gilrs    -- converts gilrs events into Bevy GamepadConnectionEvents, spawns entities
        |
      gilrs       -- normalizes events, maps buttons, generates UUIDs
        |
    gilrs-core    -- platform backend; on Windows defaults to WGI (Windows Gaming Input)
        |
    Windows APIs  -- RawGameController / Gamepad / XInput
```

## Observed Behavior

With both controllers plugged in at launch, the dashboard only ever sees one gamepad entity. The second panel shows "No gamepad detected."

### Diagnostic Logging Output

Added `log_gamepad_connections` (logs every `GamepadConnectionEvent`) and `log_gamepad_count` (logs when entity count changes) to the dashboard.

**Startup (both controllers plugged in):**

```
gilrs::gamepad WARN  No mapping found for UUID 00000000-0000-0000-0000-000000000000
    Default mapping will be used.
bevy_input::gamepad INFO  Gamepad 25v0 connected.
dashboard INFO  Gamepad entity count changed: 0 -> 1
dashboard INFO  Gamepad connected: entity=25v0, name="Xbox One Game Controller", vendor=Some(8406), product=Some(16388)
```

Only 1 connection event fired. The second controller was never reported.

**After unplugging and replugging the second controller (~20 seconds later):**

```
gilrs::gamepad WARN  No mapping found for UUID 03000000-3735-0000-a010-000000000000
    Default mapping will be used.
bevy_input::gamepad INFO  Gamepad 26v0 connected.
bevy_input::gamepad INFO  Gamepad 26v0 disconnected.
dashboard INFO  Gamepad connected: entity=26v0, name="Xbox 360 Game Controller", vendor=Some(13623), product=Some(4256)
dashboard INFO  Gamepad disconnected: entity=26v0
```

The second controller connected and immediately disconnected in the same frame. It was never usable. The gamepad entity count never reached 2.

## Root Cause Analysis

### gilrs-core WGI Backend

gilrs-core defaults to the **WGI (Windows Gaming Input)** backend on Windows. This backend:

1. Calls `RawGameController::RawGameControllers()` at initialization to enumerate connected controllers
2. Spawns a background thread that registers `RawGameControllerAdded` and `RawGameControllerRemoved` event handlers
3. Polls gamepad state every ~8ms via the background thread
4. Sends events to the main thread via an MPSC channel

**The problem:** WGI represents the same physical controller in multiple ways -- as a `RawGameController` (generic) and as a `Gamepad` (Xbox-specific). Without proper deduplication:

- A controller can appear, then get "reconciled" by Windows into a different representation
- This produces a rapid Connected -> Disconnected -> Connected sequence
- gilrs-core has no deduplication logic based on hardware IDs or `NonRoamableId`

### The Nil UUID

The `00000000-0000-0000-0000-000000000000` UUID means gilrs couldn't identify one of the devices properly. UUIDs are generated from vendor/product IDs; a nil UUID suggests the IDs weren't available when gilrs tried to enumerate the device.

### bevy_gilrs Event Processing

bevy_gilrs processes gilrs events with no coalescing or filtering:

- **PreStartup:** `gilrs_event_startup_system` iterates already-connected gamepads from gilrs and spawns entities
- **PreUpdate:** `gilrs_event_system` processes all pending events in a `while let` loop
- If gilrs fires Connected + Disconnected in the same frame, both become Bevy events verbatim
- Entities are NOT deleted on disconnection (mappings persist), but the gamepad becomes unusable

There is no debouncing, deduplication, or frame-delay logic in bevy_gilrs.

### Why XInput Doesn't Have This Problem

The XInput backend uses a completely different approach:

- Polls 4 hardcoded controller slots (0-3) every ~10ms
- Uses simple state comparison to detect connection changes
- No event handlers, no device representation juggling
- Avoids the WGI multiple-representation problem entirely
- Limited to 4 Xbox-compatible controllers

## Known Upstream Issue

This is a known issue tracked in Bevy: https://github.com/bevyengine/bevy/issues/13853

The WGI backend reports three events (connected, disconnected, connected) when a gamepad connects after startup. Pre-connected gamepads may not all be enumerated.

## Possible Fixes

| Approach | Pros | Cons |
|---|---|---|
| Switch to XInput backend via Cargo feature | Reliable, proven, simple | Max 4 controllers, Xbox-only |
| Debounce connection events in application code | No dependency changes | Adds complexity, introduces delay |
| Upgrade gilrs when a fix lands upstream | Proper fix at the right layer | Not available yet |

## Current Mitigations in Dashboard

1. **UI messaging:** The disconnected panel now shows the total gamepad entity count seen by the engine and suggests unplugging/replugging
2. **Diagnostic logging:** `log_gamepad_connections` and `log_gamepad_count` systems print connection events and entity count changes to the console

## Status

Investigating. No fix applied yet -- deciding between XInput backend switch and application-level debouncing.
