# Procedural Materials and Stickers

Two complementary systems for surface appearance: procedural materials for the base surface, and stickers for placed geometric details.

## Procedural Materials (Base Surface)

A procedural material is a function from position to color. It has no UV mapping, no texture files, and no seams.

```
color = material_function(position, normal, parameters)
```

### Why Seams Disappear

The function is continuous in space. Two adjacent pixels — even on different tiles, different faces, different shapes — produce continuous output because they have adjacent position inputs. The function doesn't know about mesh boundaries, tile edges, or part seams. Continuity is automatic.

### Three Cases of Seamlessness

**Tile-to-tile (autotile blobs):** The right edge of tile (3,5) and the left edge of tile (4,5) share world positions. Same input to the noise function → same output → invisible seam.

**Face-to-face (3D shapes):** The top face and side face of a box meet at an edge. Evaluating the material function in 3D world space at that edge produces the same color from both faces. No UV mapping needed — this is triplanar projection.

**Shape-to-shape (arm meets chassis):** Different meshes sampling the same spatial function produce continuous patterns. A rust stain flows naturally from chassis to arm.

### Coordinate Spaces

**World space** — pattern is fixed in the world. Objects moving through it appear to "swim" through the texture. Good for terrain, environment.

**Object space** — pattern moves with the root entity. All parts of a robot share one coordinate system, so patterns flow across part boundaries. Different robots get different patterns naturally. Good for entity materials.

**Part space** — each part has its own noise variation with a seed derived from its position in the hierarchy. Good for per-part detail (scratches, wear) that shouldn't repeat across parts.

### What the Material Definition Looks Like

```ron
(
    base_color: (0.45, 0.45, 0.50),
    noise_pattern: Perlin,
    noise_scale: 10.0,
    noise_strength: 0.1,
    color_variation: (0.05, 0.03, 0.02),
    coordinate_space: Object,
)
```

No texture files. No UV coordinates. The shape can change freely — different proportions, new parts, procedural generation — and the material still works.

### The Tradeoff

Procedural materials cannot paint a specific design at a specific location. They produce patterns that are spatially continuous but not intentional. A logo, a marking, a serial number, a faction emblem — these require placed, deliberate graphics. That's what stickers are for.

## Stickers (Placed Details)

A sticker is a geometric shape projected onto a surface at a specific position and orientation. It layers on top of the procedural material without replacing it.

### What a Sticker Is

- A 2D shape (circle, rectangle, polygon, SDF-defined) with a color or pattern
- Placed at a 3D position on the surface
- Projected along a direction (usually the surface normal)
- Has a finite extent — it doesn't fill the surface, it occupies a specific area
- Composited on top of the base material (alpha blending, additive, multiply)

### How It Differs from a Material

| | Procedural Material | Sticker |
|---|---|---|
| Coverage | Entire surface | Specific area |
| Position | Derived from spatial coordinates | Explicitly placed |
| Purpose | "What is this made of" | "What was put on this" |
| Continuity | Automatic (spatial function) | Self-contained (bounded shape) |
| Examples | Metal, stone, rust, wood grain | Logo, stripe, warning label, battle scar, faction emblem |

### How Stickers Work with Procedural Materials

Stickers don't conflict with procedural materials because they operate at different levels:

1. **Base layer:** procedural material evaluates at every surface point → base color
2. **Sticker layer:** for each sticker, project its shape onto the surface, evaluate the sticker's pattern within its bounds → overlay color + alpha
3. **Composite:** blend sticker over base using the sticker's alpha and blend mode

The procedural material doesn't know stickers exist. The sticker doesn't know about the material. They compose independently.

### Sticker Definition

```ron
(
    shape: Circle(radius: 0.05),
    color: (0.9, 0.1, 0.1),
    position: (0.0, 0.7, 0.26),  // world or object space
    normal: (0.0, 0.0, 1.0),     // projection direction
    blend: Alpha,
)
```

### Types of Stickers

**Geometric** — SDF-defined shapes. Circle, star, chevron, stripe. Resolution-independent, crisp at any zoom.

**Pattern** — a procedural function evaluated within the sticker bounds. A camo patch, a warning stripe (repeating diagonal lines), a grid overlay. Still no texture files — just a different function than the base material, bounded to a specific area.

**Damage** — positioned by gameplay, not data. A bullet impact, a scorch mark, a scratch. Same rendering as geometric stickers but placed dynamically at runtime.

### What Stickers Enable

- **Faction identity** — colored chevrons, emblems, number markings. Same robot body plan, different stickers → different faction
- **Individuality** — racing stripes, custom marks, kill tallies. Players (or procedural generation) customize robots with stickers
- **Communication** — warning stripes around hazardous parts, arrows indicating facing, color coding for part function
- **Damage storytelling** — accumulated battle scars as stickers. The robot's history is visible on its surface

## The Two Systems Together

The base material says "this is brushed steel." The stickers say "this steel has a red stripe, a dent from last fight, and a faction number."

Neither system uses texture files. Both are evaluated from functions and geometry. Both are resolution-independent. Both survive shape changes — if the chassis gets wider, the material stretches naturally (it's spatial) and the stickers stay at their placed positions.

## Relationship to Existing Decisions

- **All assets are algorithmic** — materials are noise functions, stickers are SDF shapes, no art files
- **Shape description as data** — material and sticker definitions live in the same RON format alongside shape geometry
- **Hybrid rasterizer + SDF** — materials and stickers can be pre-baked to textures (rasterizer tier) or evaluated at runtime (SDF tier)
- **Visual language over realism** — stickers communicate function and identity, materials communicate substance
