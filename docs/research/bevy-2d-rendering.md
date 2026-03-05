# Bevy Rendering

How Bevy turns your components into pixels on the screen.

## The GPU Only Understands Triangles

Every visual you see in a Bevy app — sprites, circles, text, UI buttons — ultimately becomes triangles sent to the GPU. The GPU is a massively parallel processor optimized for one job: take a list of triangles defined by vertices, run a small program (shader) on each vertex to position it on screen, then run another small program on each pixel covered by each triangle to determine its color.

This is true for both 2D and 3D. A 2D sprite is two triangles forming a rectangle. A 2D circle mesh is a fan of triangles approximating the curve. A 3D cube is 12 triangles (2 per face). The GPU doesn't know or care whether your game is "2D" or "3D" — it always processes triangles in 3D space, then projects them onto a 2D screen.

## 2D vs 3D: What's Actually Different

From the GPU's perspective, nothing. Both 2D and 3D rendering go through the same vertex-to-fragment pipeline. The differences are in how Bevy sets things up:

| Aspect        | 2D                                         | 3D                                                             |
| ------------- | ------------------------------------------ | -------------------------------------------------------------- |
| Camera        | Orthographic projection (no perspective)   | Perspective projection (things shrink with distance)           |
| Z-axis        | Used for draw order only                   | Represents actual depth in the scene                           |
| Lighting      | None by default                            | PBR lighting, shadows, ambient occlusion                       |
| Render phases | `Transparent2d`                            | `Opaque3d`, `AlphaMask3d`, `Transparent3d`                     |
| Materials     | `ColorMaterial` (color + optional texture) | `StandardMaterial` (PBR with roughness, metallic, normal maps) |
| Mesh types    | `Mesh2d` wrapper                           | `Mesh3d` wrapper                                               |

A `Camera2d` is just a `Camera` with an orthographic projection. You could render 3D meshes with it (they'd look flat). You could put a perspective camera on 2D sprites (they'd have depth distortion). The "2D" and "3D" labels are conventions, not fundamental distinctions.

## The Rendering Pipeline

Bevy uses a **pipelined dual-world architecture**. The main world runs game logic for frame N+1 while the render world simultaneously processes frame N on the GPU. Data flows through four phases:

1. **Extract** — Copy relevant components (transforms, meshes, materials, cameras) from the main world into the render world. This is the only synchronization point.
2. **Prepare** — Convert extracted data into GPU-ready buffers: vertex arrays, uniform buffers (transform matrices, material properties), textures.
3. **Queue** — Organize entities into render phases, sort them (back-to-front for transparency), batch compatible draw calls.
4. **Render** — Execute the render graph: bind pipelines, bind resources, issue `draw_indexed()` calls. The GPU processes vertices through vertex shaders, rasterizes triangles into fragments, runs fragment shaders to compute color, and writes the result to the framebuffer.

You don't interact with this pipeline directly in normal Bevy code. You spawn entities with the right components and Bevy handles the rest.

## Bevy's 2D Rendering Approaches

From highest-level (simplest) to lowest-level (most control):

### 1. Sprites — Textured Rectangles

The most common 2D primitive. A `Sprite` is a textured quad (two triangles).

```rust
commands.spawn(Sprite {
    image: asset_server.load("ball.png"),
    ..default()
});
```

Sprites support texture atlases, nine-slice scaling, anchoring, flipping, and custom sizing. Bevy automatically batches sprites that share a texture into fewer draw calls. This is the fastest path for rendering many images.

**Use for**: game objects with artwork, tiled backgrounds, sprite sheets, anything image-based.

### 2. Mesh2d + ColorMaterial — Filled Shapes

For shapes defined by geometry rather than images. Bevy provides primitive shapes (`Circle`, `Rectangle`, `Triangle2d`, `RegularPolygon`, etc.) that convert to meshes.

```rust
let mesh = meshes.add(Circle::new(1.0));
let material = materials.add(ColorMaterial::from_color(Color::srgb(1.0, 0.0, 0.0)));
commands.spawn((Mesh2d(mesh), MeshMaterial2d(material), Transform::default()));
```

The mesh defines the triangles; the material defines how to color them. One mesh can be shared across many entities (each with its own material and transform). Scale via `Transform` to avoid creating multiple mesh sizes.

**Use for**: filled geometric shapes, colored primitives, anything where you want solid GPU-rendered shapes without a texture.

### 3. Gizmos — Immediate-Mode Debug Lines

Gizmos draw lines, circles, arrows, and other debug visuals. They're resubmitted every frame (nothing is retained) and don't create entities.

```rust
fn draw(mut gizmos: Gizmos) {
    gizmos.circle_2d(Vec2::ZERO, 50.0, Color::WHITE);
    gizmos.line_2d(Vec2::ZERO, Vec2::new(100.0, 0.0), Color::RED);
}
```

Gizmos only draw **outlines** — there is no filled circle or filled rectangle gizmo. They render as line strips (sequences of thin quads). They're not batched as aggressively as sprites and are more expensive per-visual than mesh-based rendering.

**Use for**: debug visualization, trail effects, overlays, anything ephemeral that doesn't need to be a persistent entity. Not suitable for primary game rendering at scale.

### 4. Direct Pixel Manipulation — CPU Framebuffer

You can create an `Image`, write RGBA bytes directly to its data array, and display it as a sprite. This gives framebuffer-level control.

```rust
let mut image = Image::new_fill(
    Extent3d { width: 320, height: 180, depth_or_array_layers: 1 },
    TextureDimension::D2,
    &[0, 0, 0, 255],
    TextureFormat::Rgba8UnormSrgb,
    RenderAssetUsages::all(),
);
// Set pixel at (x, y):
let i = (y * width + x) as usize * 4;
image.data[i..i+4].copy_from_slice(&[r, g, b, a]);
```

This runs on the CPU. Practical for small virtual canvases (retro-style games at 320x180), but too slow for full-resolution per-pixel updates every frame.

See [coordinate-systems-and-pixels.md](coordinate-systems-and-pixels.md) for details on the pixel index formula and texture-to-world mapping.

**Use for**: retro framebuffer effects, procedural texture generation, cellular automata, anything that genuinely needs per-pixel CPU control.

### 5. Custom Shaders — GPU Programs

For full control, write custom WGSL shaders that run on the GPU. Implement the `Material2d` trait to define your own vertex and fragment shaders with custom uniforms.

This is the most powerful and most complex approach. The GPU executes your shader for every pixel in parallel, making it ideal for effects that are expensive on CPU (noise, fractals, post-processing, distortion).

**Use for**: visual effects, procedural generation, post-processing, anything the built-in materials can't express.

## Can I Just Set a Pixel?

Yes, in several ways:

| Method                        | Where it runs         | Speed         | Flexibility                                   |
| ----------------------------- | --------------------- | ------------- | --------------------------------------------- |
| `Image::data` byte array      | CPU                   | Slow at scale | Full per-pixel control                        |
| 1x1 sprite with `custom_size` | GPU (sprite pipeline) | Fast          | One "pixel" per entity — impractical for many |
| Fragment shader               | GPU                   | Fastest       | Full control via math, no per-pixel API       |

The mental model shift: GPUs don't have a "set pixel at (x,y)" API. Instead, you define geometry (triangles) and programs (shaders) that determine what color each covered pixel should be. The fragment shader *is* the per-pixel logic — it runs once for every pixel the triangle covers, in parallel across thousands of GPU cores.

If you want to "just draw a red dot at (100, 50)", the simplest Bevy approach is a small sprite or a `Mesh2d` circle positioned there — not because it's the only way, but because fighting the GPU's triangle-based model is slower than embracing it.

## Vector vs Raster

**Raster** (pixel-based): Sprites and direct pixel manipulation. The visual is a grid of colored pixels. Scaling up makes it blurry or pixelated. Resolution-dependent.

**Vector** (geometry-based): Mesh2d shapes and gizmos. The visual is defined by math (vertices, curves). Scales cleanly to any size — the GPU re-rasterizes at the target resolution. Resolution-independent.

Bevy doesn't have a true vector graphics renderer (like SVG). `Mesh2d` shapes are tessellated into triangles at creation time — a `Circle` is really a polygon with enough sides to look smooth. If you scale a circle mesh up enormously, you may see the polygon edges. Increasing the segment count in the mesh (or recreating it) fixes this.

Text is a hybrid: glyphs are vector outlines in the font file, rasterized to bitmaps at a specific size, then rendered as textured quads. See [coordinate-systems-and-pixels.md](coordinate-systems-and-pixels.md) for text rendering details.

## Choosing an Approach

| Situation                                     | Approach                                             |
| --------------------------------------------- | ---------------------------------------------------- |
| Game objects with artwork                     | Sprite                                               |
| Solid-colored geometric shapes                | Mesh2d + ColorMaterial                               |
| Debug overlays, trails, guidelines            | Gizmos                                               |
| Retro pixel-art framebuffer                   | Direct pixel manipulation                            |
| Complex visual effects                        | Custom shader                                        |
| Many identical shapes, different colors/sizes | Shared Mesh2d, per-entity material + transform scale |
| Text                                          | `Text2d` (Bevy handles rasterization)                |

## Performance Notes

- **Sprites** are the fastest for textured quads. Automatic batching means 1000 sprites sharing a texture atlas can render in one draw call.
- **Mesh2d** entities each require a material binding. Fewer draw calls than unbatched sprites, but no automatic batching across entities.
- **Gizmos** are rebuilt from scratch every frame on the CPU. Fine for dozens of shapes, expensive for thousands.
- **Draw call count** is often the bottleneck, not triangle count. The GPU can handle millions of triangles easily; it's the CPU overhead of setting up each draw call that limits throughput.
- **Z-ordering** in 2D uses the `Transform` z-coordinate. All 2D content goes through the `Transparent2d` phase (sorted back-to-front), so overlapping transparent entities render correctly but can't be batched as aggressively as opaque 3D.
