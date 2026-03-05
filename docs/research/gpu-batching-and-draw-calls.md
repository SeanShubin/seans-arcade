# GPU Batching and Draw Calls

Why `stress_balls.rs` (sprites) is dramatically faster than `bouncy_balls.rs` (Mesh2d + ColorMaterial) at scale, and the GPU concepts behind it.

## Draw Calls

A draw call is a CPU-to-GPU command: "bind this state, render this geometry." The expensive part is state setup — binding shaders, textures, uniform buffers, and blend modes — not the actual triangle rasterization. Fewer draw calls = better performance.

## GPU Batching

When multiple entities share identical pipeline state (same shader, same texture, same blend mode), the renderer can pack them into a single draw call. Per-instance data — position, scale, rotation, color tint — goes into an instance buffer. The GPU reads each entity's data from that buffer without rebinding anything.

Bevy's sprite renderer is purpose-built for this. It hardcodes knowledge of which fields vary per-instance, so it can batch aggressively.

## Textures vs Tint Color

A texture is an image (grid of pixels) loaded into GPU memory. Tint is a per-component multiply applied at render time:

    output = texture_pixel × tint_color

Different tint colors do NOT create different textures. All sprites sharing the same source image batch together regardless of tint. A white pixel texture (1.0, 1.0, 1.0, 1.0) tinted with any color produces that color exactly: `1.0 × tint = tint`.

## Why ColorMaterial Doesn't Batch

Each `Handle<ColorMaterial>` creates a separate uniform buffer on the GPU. The renderer treats each material as an opaque, independent configuration — it can't introspect the struct to notice that only the color field differs between instances.

There's no mechanism in the `Material2d` trait to declare fields as "instanceable." This is a Bevy architectural choice, not a GPU limitation. (For comparison, Unity's SRP Batcher solves this by packing same-shader materials into a shared constant buffer.)

## Limits of Sprite Tinting

Built-in per-instance data that preserves batching:
- **Tint** (color multiply)
- **Transform** (position, rotation, scale)
- **Custom size**
- **Flip X / Flip Y**
- **Texture atlas index**

Tint is the only color transformation available. There's no hue shift, palette swap, additive blend, or other color operation built in. For multi-color variation (e.g., two independently-colored regions on one sprite), tinting alone isn't enough.

Custom shaders can extend per-instance data, but the additional parameters must be packed into the instance buffer to preserve batching.

## Indexed Color / Palette-Based Rendering

A well-established technique for achieving full per-sprite color control without breaking batching. This is how 8-bit and 16-bit era games (NES, SNES, GBA) handled color variation — and the same principle applies on modern GPUs.

### How It Works

1. **Source texture**: Every pixel is a unique "index color" — not the final display color, but a lookup key. A 16x16 sprite has up to 256 unique index values.
2. **Palette texture**: A shared lookup texture where each row is a complete color mapping. Row 0 might map index colors to a red team palette, row 1 to blue, etc.
3. **Shader**: Reads the index from the source texture, then samples the palette texture to get the final color:

```glsl
vec4 index = texture(sprite_texture, uv);
vec4 final_color = texture(palette_texture, vec2(index.r, palette_row));
```

4. **Per-instance palette selection**: The palette row is selected using an existing per-instance field (e.g., texture atlas index or a custom instance attribute).

Since all sprites share the same source texture and the same palette texture, the GPU sees identical pipeline state → one draw call for everything.

### Why It Preserves Batching

The color variation lives entirely inside a shared texture, not in per-entity uniform buffers. From the renderer's perspective, nothing differs between entities except the standard per-instance data (transform, tint, atlas index). No state rebinding needed.

### Sweet Spot vs Limits

The technique works best when many entities choose from a **bounded set of palettes**:

| Scenario | Palette rows | Fits easily? |
|----------|-------------|-------------|
| 32K sprites, 8 team colors | 8 | Trivial |
| 32K sprites, 50 character variants | 50 | Easy |
| 32K sprites, each completely unique | 32K | Pushing texture size limits |

If every entity has a truly unique color mapping, you're storing one palette row per entity. A 16x16 sprite (256 index colors) with 32K entities means a palette texture of 256x32K pixels. This still batches into one draw call (better than 32K draw calls with ColorMaterial), but you're now bounded by maximum texture dimensions and VRAM instead of draw call overhead.

### Comparison with Other Approaches

| Approach | Color control | Batching | Complexity |
|----------|--------------|----------|------------|
| Sprite tint | 1 multiply color | Batches | Built-in |
| Per-entity ColorMaterial | Full per-entity | No batching | Simple but slow |
| Indexed color + palette texture | Full per-palette | Batches | Custom shader required |

Indexed color is the standard solution when you need more color variation than tinting provides but can't afford per-entity draw calls.

## Practical Impact: stress_balls vs bouncy_balls

| Example | Rendering approach | Draw calls | Performance |
|---------|--------------------|------------|-------------|
| `stress_balls` | 1 shared white-pixel texture, tinted per-instance | 1 draw call for all sprites | 32K+ entities at 240 FPS |
| `bouncy_balls` | Per-entity `ColorMaterial` + trails | ~5x entities, each its own draw call | Hits limits much sooner |

The sprite approach keeps the GPU pipeline bound once and streams instance data. The material approach rebinds state for every entity.

See [stress-balls-performance-case-study.md](stress-balls-performance-case-study.md) for related findings on vsync stair-stepping and FixedUpdate behavior.
