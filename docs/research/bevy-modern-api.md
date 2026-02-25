# Bevy Modern API Reference

Bevy evolves rapidly. Between 0.14 and 0.18 (the project's current version), several APIs were replaced by better alternatives. Old patterns still appear in tutorials, examples, and AI suggestions. This document gives a quick reference: when you want to do X, here's the current way, and here's what to avoid.

## 1. Spawning Entities

**Current (0.15+):** Spawn components directly. Required components auto-insert.
```rust
commands.spawn(Sprite { ... });           // Transform, Visibility auto-inserted
commands.spawn(Camera2d);                 // Camera, Transform, etc. auto-inserted
commands.spawn(Text2d::new("hello"));     // TextLayout, Transform auto-inserted
```

**Avoid:** All `*Bundle` types — `SpriteBundle`, `Camera2dBundle`, `Camera3dBundle`, `NodeBundle`, `TextBundle`, `Text2dBundle`, `PbrBundle`, `PointLightBundle`, etc. These are deprecated.

## 2. Choosing Colors

**Current constructors — pick based on what you need:**

| Need | Constructor |
|---|---|
| Specific color (matches hex) | `Color::srgb()` / `srgba()` |
| Tweak hue/saturation/lightness | `Color::hsl()` / `hsla()` |
| Perceptual palettes, gradients | `Color::oklch()` / `oklcha()` / `oklab()` / `oklaba()` |
| Shader math, lighting | `Color::linear_rgb()` / `linear_rgba()` |

**Avoid:**

| Old | Replaced by | Why |
|---|---|---|
| `Color::rgb()` / `rgba()` | `Color::srgb()` / `srgba()` | Renamed in 0.14 for explicit color space |
| `Color::rgb_linear()` | `Color::linear_rgb()` | Renamed in 0.14 |
| `Color::lab()` / `laba()` | `Color::oklab()` / `oklaba()` | Oklab is more perceptually accurate |
| `Color::lch()` / `lcha()` | `Color::oklch()` / `oklcha()` | Same improvement |
| `Color::hsv()` / `hsva()` | `Color::hsl()` / `hsla()` | HSL's lightness is more intuitive than HSV's value |
| `Color::hwb()` / `hwba()` | `Color::hsl()` / `hsla()` | HWB is niche, HSL is more widely understood |
| `Color::xyz()` / `xyza()` | (don't use) | Reference color space, not for picking colors |

## 3. Events / Messages

**Current (0.17+):** `MessageWriter` / `MessageReader` / `Messages<M>`.
```rust
fn send(mut writer: MessageWriter<MyEvent>) { writer.write(MyEvent { ... }); }
fn recv(mut reader: MessageReader<MyEvent>) { for event in reader.read() { ... } }
```

**Avoid:** `EventWriter::send()`, `EventReader`, `Events<E>` — renamed in 0.17.

## 4. Parent-Child Relationships

**Current (0.16+):** `ChildOf` component. `despawn()` is recursive by default.
```rust
commands.entity(child).insert(ChildOf(parent));
commands.entity(parent).despawn();  // children despawned too
```

**Avoid:** `Parent` component, `despawn_recursive()`, `set_parent()` — replaced in 0.16.

## 5. Queries

**Current (0.16+):** `query.single()` returns `Result`.
```rust
let transform = query.single()?;
```

**Avoid:** `query.get_single()` (deprecated, `single()` now does the same), old `query.single()` that panicked.

## 6. Computed Components (Read-Only)

These components are computed by Bevy. Never set them manually.

| Component | Set by | You should use |
|---|---|---|
| `GlobalTransform` | Bevy's transform propagation | Write `Transform`, read `GlobalTransform` |
| `InheritedVisibility` | Bevy's visibility propagation | Set `Visibility` only |
| `ViewVisibility` | Bevy's rendering | Set `Visibility` only |

## 7. Audio Volume

**Current (0.16+):** `Volume::Linear(1.0)` or `Volume::Decibels(0.0)`.

**Avoid:** `Volume(1.0)`, `Volume::ZERO` → now `Volume::SILENT`.

## 8. Camera HDR

**Current (0.17+):** Separate `Hdr` marker component.
```rust
commands.spawn((Camera3d, Hdr));
```

**Avoid:** `Camera { hdr: true }` — HDR split into a separate component.

## 9. Text Justification

**Current (0.17+):** `Justify`.

**Avoid:** `JustifyText` — renamed for consistency.

## 10. bevy_egui Context Access

**Current (bevy_egui 0.39 / Bevy 0.18):** Register UI systems in `EguiPrimaryContextPass`, not `Update`. `ctx_mut()` returns `Result`.
```rust
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

app.add_plugins(EguiPlugin::default())
   .add_systems(EguiPrimaryContextPass, ui_system);

fn ui_system(mut contexts: EguiContexts) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    egui::CentralPanel::default().show(ctx, |ui| { /* ... */ });
}
```

**Avoid:** `EguiPlugin` as unit struct (now has fields, use `EguiPlugin::default()`), registering egui systems in `Update` (panics: "Called `available_rect()` before `Context::run()`"), `ctx_mut()` without handling `Result`.

## Sources

- [Bevy 0.14 to 0.15 migration](https://bevy.org/learn/migration-guides/0-14-to-0-15/)
- [Bevy 0.15 to 0.16 migration](https://bevy.org/learn/migration-guides/0-15-to-0-16/)
- [Bevy 0.16 to 0.17 migration](https://bevy.org/learn/migration-guides/0-16-to-0-17/)
- [Bevy 0.17 to 0.18 migration](https://bevy.org/learn/migration-guides/0-17-to-0-18/)
- [Bevy Color API](https://docs.rs/bevy/latest/bevy/prelude/enum.Color.html)
