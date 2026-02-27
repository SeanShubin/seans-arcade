# Coordinate Systems and Pixels in Bevy

How world units, screen pixels, camera projection, and sprite textures relate to each other.

## The Two Coordinate Systems

**World units** — the coordinate space entities live in. A `Transform` position of `(100, 50, 0)` means 100 world units right, 50 up. World units have no inherent size — they're abstract numbers until a camera maps them to the screen.

**Screen pixels** — the physical dots on the monitor. A 1920x1080 window has 1920 columns and 1080 rows of pixels. On high-DPI displays, logical pixels and physical pixels differ (see DPI section below).

## The Camera Is the Bridge

The camera defines how world units map to screen pixels. With a default `Camera2d`:

- 1 world unit = 1 screen pixel
- The camera is centered at the origin
- +X is right, +Y is up

Change the camera's transform or projection and the mapping changes:

| Camera change | Effect |
|---|---|
| `Transform::from_xyz(100, 0, 0)` | World origin appears 100 pixels left of screen center |
| `OrthographicProjection { scale: 2.0, .. }` | 1 world unit = 0.5 screen pixels (zoomed out) |
| `OrthographicProjection { scale: 0.5, .. }` | 1 world unit = 2 screen pixels (zoomed in) |

The formula: `screen_pixels = world_units / projection_scale`

## What Lives in Which Coordinate System

| Thing | Coordinate system | Notes |
|---|---|---|
| `Transform` position | World units | Where an entity is in the game world |
| `Transform` scale | Multiplier on world size | `scale: 2.0` doubles the entity's world size |
| `Sprite` with `custom_size` | World units | `Vec2::new(15.0, 80.0)` = 15x80 world units |
| `Sprite` with no `custom_size` | World units (1 texel = 1 world unit) | A 16x16 texture occupies 16x16 world units |
| `Viewport` position/size | Physical pixels | Defines a sub-rectangle of the window |
| UI nodes (`Node`) | Logical pixels | Bevy UI layout, independent of game camera |
| Gizmo positions | World units | Where the gizmo appears in the world |
| Gizmo line width | Screen pixels | Constant width regardless of zoom |

## Sprites: Textures vs. World Size

A sprite's texture resolution and its world size are independent:

- A **16x16 pixel texture** with no `custom_size` occupies 16x16 world units.
- The same texture with `custom_size: Some(Vec2::new(64.0, 64.0))` occupies 64x64 world units — the texture is stretched to fit.
- A sprite with `custom_size` and **no texture** (just a `color`) is a solid rectangle in world units. No pixels to worry about.

With a default camera (1:1 mapping), a 16x16 texture with no `custom_size` appears as 16x16 screen pixels. Zoom the camera to 2x and it appears as 32x32 screen pixels, but it's still 16x16 world units.

## Pixel Art and Integer Scaling

When the camera maps 1 texel to a non-integer number of screen pixels, some texels get more pixels than others. A row of evenly spaced pixels in the art becomes unevenly spaced on screen. This causes **pixel shimmer** — visible jitter, especially during movement.

**The fix for pixel art:**

1. Use `ImagePlugin::default_nearest()` to prevent texture blurring.
2. Keep the camera scale at integer multiples: 1x, 2x, 3x, etc.
3. Design around a virtual canvas (e.g., 320x180) and scale it to the window.

**This doesn't apply to:**
- Solid-color sprites with `custom_size` (no texels to misalign)
- High-res textures where filtering hides the artifacts
- UI text and nodes (they use their own pixel-aligned layout)

## The Virtual Canvas Pattern

A common approach for pixel art games:

1. Pick a small virtual resolution (e.g., 320x180).
2. Set the camera projection so that 320x180 world units fills the screen.
3. Author all art at that resolution.
4. The window can be any size — the camera scales it up with integer scaling.

This decouples your game logic (always in world units at your virtual resolution) from the player's screen size.

## DPI and Scale Factor

High-DPI displays (Retina, 4K) have a **scale factor** — e.g., 2.0 means each logical pixel is 2x2 physical pixels.

- `Window::width()` / `Window::height()` — logical size
- `Window::physical_width()` / `Window::physical_height()` — actual pixel count

If you're creating a texture to fill the window pixel-for-pixel, use `physical_width/height`. If you're doing UI layout, use logical size.

## Direct Pixel Manipulation

You can bypass sprites entirely and write pixels directly to a texture:

1. Create an `Image` with your desired resolution.
2. Manipulate the `image.data` byte array (RGBA, 4 bytes per pixel).
3. Add it to `Assets<Image>` and display it as a sprite.
4. Update pixels at runtime via `ResMut<Assets<Image>>`.

The pixel index formula: `(row * width + col) * 4`

This gives framebuffer-level control. Performance is fine for small virtual canvases but expensive at full screen resolution — for that, use a shader.

## Practical Guidelines

- **Game objects** (paddles, balls, walls): think in world units. Let the camera handle screen mapping.
- **UI** (score, menus, status text): think in logical pixels. Use Bevy's UI system, which is camera-independent.
- **Art assets**: pick a texel density that matches your art style, use `custom_size` if you want to decouple texture resolution from world size.
- **Camera setup**: decide early how many world units should be visible. For pong with an 800-unit-wide arena, set the camera so 800 world units spans the window width.
- **Don't mix concerns**: game logic should never reference pixel counts. Screen adaptation is the camera's job.
