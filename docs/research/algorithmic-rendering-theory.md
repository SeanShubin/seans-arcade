# Algorithmic Rendering Theory Roadmap

Core theory areas for rendering all game assets algorithmically — no art files, only code that generates visuals either pre-computed or at runtime.

## Existing Research

These topics already have dedicated docs in this project:

- [Autotile blob patterns](autotile-blob-patterns.md) — 47-tile blob sets, bitmask indexing, Wang tiles
- [Software rasterizer vs distance field](software-rasterizer-vs-distance-field.md) — two approaches for CPU bevel rendering
- [Procedural animation](procedural-animation.md) — vector graphics character animation with Bevy Mesh2d
- [Procedural vs authored design](procedural-vs-authored-design.md) — what can and can't be automated in level design

## Theory Areas

### 1. Procedural Noise

The foundation for terrain, biomes, and texture generation.

- **Simplex noise** — gradient noise for height, moisture, temperature maps. Preferred over Perlin in 2D+ (fewer artifacts, cheaper in higher dimensions).
- **Fractal Brownian Motion (fBM)** — layering octaves of noise at different frequencies and amplitudes for natural-looking detail. Parameters: octaves, lacunarity (frequency multiplier), gain (amplitude multiplier).
- **Domain warping** — feeding noise output into the coordinates of other noise. Creates organic, non-repetitive shapes (river bends, coastlines, cave walls).
- **Voronoi / Worley noise** — cell-based noise. Useful for biome region boundaries, cracked earth, stone textures, cellular patterns.

### 2. Signed Distance Fields (SDFs)

The primary tool for generating shapes algorithmically.

- **Basic SDF primitives** — circle, box, line segment, polygon. Each is a function `f(point) → distance` where negative means inside.
- **Boolean operations** — union (min), intersection (max), subtraction. Combine primitives into complex shapes.
- **Smooth blending (smooth-min)** — `smin(a, b, k)` blends two SDFs with controllable roundness. Essential for organic shapes (rocks, foliage, terrain features).
- **Transformations** — translate, rotate, scale, repeat, mirror. Applied by transforming the input point before evaluating the SDF.
- **Anti-aliasing from SDFs** — `smoothstep(-pixel_size, pixel_size, distance)` gives exact coverage-based AA for free.

### 3. Biome Blending

Smooth transitions between terrain types.

- **Biome weight maps** — each biome has a continuous weight at every point (from noise layers). Weights sum to 1.0 at each location.
- **Interpolation methods** — bilinear for grid-aligned data, barycentric for Voronoi-based regions.
- **Material blending** — blend color, normal, roughness per-pixel using biome weights. Avoids hard tile edges.
- **Transition zones** — configurable blend width between biomes. Narrow = distinct borders, wide = gradual shift.
- **Height-based blending** — use terrain height to influence biome selection (sand at low elevation, snow at peaks) and blend at boundaries.

### 4. Pseudo-3D (Top-Down with Depth)

Making a 2D game look 3D from a fixed overhead angle.

- **Normal mapping** — encode surface orientation as RGB. Light the scene with a directional light to create the illusion of 3D relief. Normals can be computed analytically from SDFs or height fields.
- **Height-based parallax** — shift sprite layers based on a height value. Walls and trees appear to "lean away" from the camera center.
- **Ambient occlusion** — darken edges and creases. For tile-based worlds, precompute AO from the neighbor bitmask (same lookup as autotiling).
- **Drop shadows** — objects at higher elevation cast shadows offset by a fixed vector. Simple but effective depth cue.
- **Elevation layering** — render at multiple Z layers (ground, objects, canopy) with shadows between layers.

### 5. Vector Graphics & Rasterization

Turning math into pixels.

- **Bezier curves** — cubic and quadratic curves for smooth organic outlines (leaves, rivers, paths). De Casteljau's algorithm for evaluation.
- **Scanline fill** — rasterizing filled polygons. Winding number rule for complex shapes with holes.
- **Coverage-based anti-aliasing** — compute pixel coverage analytically (from SDF or edge equations) rather than supersampling.
- **Stroke rendering** — offset curves for outlines. Variable-width strokes for hand-drawn aesthetics.

### 6. Parametric Animation

Motion from math, not keyframes.

- **Easing functions** — `smoothstep`, `ease-in-out`, `bounce`, `elastic`. Map linear time to shaped curves.
- **Sinusoidal displacement** — wind in grass/trees as `sin(time + position * phase)`. Spatial phase offset prevents synchronization.
- **Inverse kinematics (2-bone)** — procedural limb positioning. Given a target point, compute joint angles analytically.
- **Procedural state machines** — idle/walk/attack states where each state's visuals are parametric functions, not canned sequences.
- **Sprite sheet baking** — when pre-computing animation frames, sample the parametric function at fixed intervals and write to a texture atlas.

### 7. Color & Material Theory

Generating palettes and textures algorithmically.

- **HSL/HSV color space** — shift hue for biome variation, saturation for atmosphere (fog, distance), lightness for elevation. Far easier to work with algorithmically than RGB.
- **Procedural textures** — combine noise layers with color ramps. Grass = green ramp over high-frequency noise. Stone = gray ramp over Voronoi cells.
- **Palette quantization** — map continuous colors to a limited palette for a stylized look. Ordered dithering for smooth gradients with few colors.
- **Lighting model** — even for 2D, a simple Lambertian `dot(normal, light_dir)` with ambient term gives convincing shading when combined with normal maps.

### 8. Key Math Primitives

Building blocks used across all the above.

- **smoothstep(edge0, edge1, x)** — smooth interpolation with zero derivatives at endpoints. Used everywhere.
- **smooth-min / smooth-max** — blend SDF shapes with controllable roundness parameter `k`.
- **Polar coordinates** — radial patterns, flowers, stars, circular elements. `(r, θ)` from `(x, y)`.
- **2D rotation matrix** — `[[cos θ, -sin θ], [sin θ, cos θ]]` for orienting shapes.
- **Hash functions** — cheap deterministic pseudo-random values from integer coordinates. For scattering details (pebbles, grass tufts) without storing positions.
- **Remapping** — `lerp`, `inverse_lerp`, `remap(value, in_min, in_max, out_min, out_max)`. Convert between ranges constantly.

## Recommended Prototype Order

Each prototype exercises a theory area and produces something visible.

1. **Noise visualizer** — render Simplex, fBM, Voronoi, domain warping to screen. Interactive parameters. *(done: `noise_visualizer.rs`)*
2. **SDF shape composer** — draw shapes from SDF primitives with boolean ops and smooth blending. *(done: `sdf_composer.rs`)*
3. **Normal map lighting** — generate normal maps from height/SDF, apply directional lighting for pseudo-3D. *(done: `normal_map_lighting.rs`)*
4. **Biome blender** — generate a multi-biome map with smooth transitions between rasterized tiles. *(done: `biome_blender.rs`)*
5. **Parametric character** — animate a vector graphics character with easing functions and procedural state. *(done: `procedural_hero_shapes.rs` → `procedural_hero_walk.rs` → `procedural_hero_3d.rs`)*

Note: Autotile blob tile generation is already covered by `texture_lab.rs` (software rasterizer approach). Per the hybrid rendering decision, tiles are pre-rendered with the rasterizer; SDFs are used at runtime for blending, normal maps, and lighting — not for generating the tiles themselves.
