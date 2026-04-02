# Software Rasterizer vs Distance-Field Renderer

Two fundamentally different approaches for rendering beveled tile edges in a CPU pixel buffer.

## Context

When rendering autotile bevels to a pixel buffer (for `texture_lab`), each pixel in the bevel zone needs a color that represents a sloped surface catching light. The question is how to compute that color.

## Software Rasterizer

Each bevel edge is modeled as a geometric quad with four vertex colors (precomputed from the lighting model). For each pixel, the renderer asks: **"Which quad am I inside?"** Then it bilinearly interpolates the vertex colors based on the pixel's position within that quad.

At convex corners where two bevel quads overlap (e.g., north and west), one quad wins entirely. The pixel gets 100% of that quad's interpolation. This matches what a GPU does when drawing overlapping triangles at the same Z depth.

This is the approach used in `beveled_block.rs` (via GPU mesh rendering) and now in `texture_lab.rs` (via CPU).

### Characteristics

- Each pixel is assigned to exactly one primitive
- Corner behavior is discontinuous (one quad wins)
- Gradient shape is determined by quad geometry
- Cost scales with geometry count, not pixel count
- Per-pixel cost is cheap (a few multiplies to interpolate)

## Distance-Field Renderer

There are no quads. For each pixel, the renderer asks: **"How far am I from each boundary?"** Then it uses those distances to weight normal contributions from all nearby edges simultaneously. The bevel shape emerges from the distance math.

At convex corners, both the north edge and west edge contribute normals weighted by inverse distance. The result is a smoothly blended diagonal — no single edge "wins."

### Characteristics

- Each pixel is influenced by multiple boundaries simultaneously
- Corner behavior is smooth (weighted blend)
- Gradient shape is determined by falloff curve (linear, sqrt, etc.)
- Cost scales with pixel count times boundary count
- No geometry setup cost

## Why They Produce Different Results

The two approaches agree at three locations:
1. **The exact corner point** — both edges contribute equally, producing the same diagonal
2. **Along cardinal edges** — only one edge contributes in both approaches
3. **The face** — no bevel in either approach

They disagree at every other point in the corner overlap zone, because:
- The rasterizer picks one quad's interpolation (discontinuous selection)
- The distance-field blends contributions from both edges (continuous blend)

## Which We Chose

The software rasterizer, because:
- It exactly matches `beveled_block`'s GPU rendering, making the two examples visually identical
- It eliminates the need for falloff curve and interpolation mode toggles — the quad geometry determines the gradient naturally
- The resulting bevels have sharper, more defined corners that look better for tile-based art
