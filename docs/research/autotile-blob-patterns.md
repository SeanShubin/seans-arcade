# Autotile Systems: Blob Patterns, Wang Tiles, and How They Relate

## The Problem

When building tile-based maps, a single "wall" tile isn't enough. A wall tile needs different visual variants depending on what's next to it: top edges, corners, interior fills, isolated blocks, peninsulas, etc. Manually placing each variant is tedious and error-prone. Auto-tiling solves this by automatically selecting the correct tile variant based on a cell's neighbors.

There are two major families of auto-tiling: **blob patterns** (47-tile) and **Wang tiles** (16-tile). They solve the same problem differently.

## Blob Patterns (47-Tile)

### Neighbor Configurations

Each tile in a grid has 8 neighbors: 4 cardinal (N/S/E/W) and 4 diagonal (NE/NW/SE/SW).

With 8 neighbors, each either same-type or different, there are 2^8 = 256 possible configurations. But most of those are visually redundant.

### Why 47 and Not 256

A diagonal neighbor only matters visually when both adjacent cardinal neighbors are also filled. Consider the NE diagonal: if the N or E neighbor is empty, there's already a visible edge there, and whether NE is filled or not doesn't change the tile's appearance. The diagonal only creates a visible inner corner when both N and E are filled but NE is empty.

This constraint collapses 256 configurations down to **47 visually distinct patterns**.

### Pattern Categories

| Category               | Count | Description                                               |
| ---------------------- | ----- | --------------------------------------------------------- |
| Isolated               | 1     | No same-type neighbors                                    |
| Peninsulas             | 4     | One cardinal neighbor (N/S/E/W)                           |
| Straight edges         | 4     | Two opposite cardinal neighbors                           |
| Outer corners          | 4     | Two adjacent cardinal neighbors, no diagonal between them |
| Outer corners + inner  | 4     | Two adjacent cardinal + the diagonal between them         |
| T-junctions            | 4     | Three cardinal neighbors, variations on diagonal fills    |
| T-junction variants    | 8     | Three cardinal with different diagonal combinations       |
| Four-cardinal variants | 16    | All four cardinal filled, 0-4 diagonals missing           |
| End caps               | 2     | Thin connecting pieces                                    |

(Exact groupings vary by source, but they always total 47.)

### How Blob Tiles Work

Each cell looks at all 8 neighbors and computes a bitmask. The bitmask maps to one of the 47 patterns. The pattern selects the tile. Terrain boundaries fall on cell edges — the border between "wall" and "floor" runs along the grid lines.

### Strengths and Weaknesses

- **Strength:** Handles inner corners perfectly (the diagonal check catches the case where two walls meet at a corner but the diagonal is empty)
- **Strength:** One tile = one terrain type, easy to reason about
- **Weakness:** Requires 47 tiles per terrain type — asset-heavy
- **Weakness:** Transitions are always on cell boundaries, which can look rigid

## Wang Tiles (16-Tile)

### Origin

Wang tiles were proposed by mathematician Hao Wang in 1961. The original concept: square tiles with colored edges, placed so adjacent edges always match. No rotation or reflection allowed. Wang studied them for questions in computability theory, but game developers adopted the core idea for terrain transitions.

### How They Work in Games

Game developers use two variants:

**Corner-matching (2-corner Wang tiles):** Each tile's four corners are assigned a terrain type. Adjacent tiles must agree on shared corners. With 2 terrain types and 4 corners, there are 2^4 = **16 possible tiles**. This is the most common variant in games.

**Edge-matching (2-edge Wang tiles):** Each tile's four edges are assigned a terrain type. Adjacent tiles must agree on shared edges. Also 2^4 = 16 tiles, but the visual result is different — edge-matching tends to produce path/maze patterns, while corner-matching produces patch/terrain patterns.

### The Key Difference from Blob Tiles

In Wang corner tiles, the terrain boundary runs **through the center of each tile**, not along the grid lines. Each tile contains pieces of up to 4 different terrain types meeting at the center. This means:

- The visual transition between terrains is smoother — it happens mid-tile
- You get equivalent visual coverage with 16 tiles instead of 47
- But each tile is more complex to draw (it contains transition art, not just one terrain)

### Strengths and Weaknesses

- **Strength:** Only 16 tiles needed (vs 47 for blob)
- **Strength:** Smoother terrain transitions — boundaries don't snap to grid lines
- **Strength:** Well-suited for top-down terrain (grass/dirt/water transitions)
- **Weakness:** Each tile contains mixed terrain, harder to author by hand
- **Weakness:** Less intuitive — painting "wall at position X" doesn't map as directly to a single tile

## How They Relate

Both systems solve neighbor-aware tile selection. The fundamental difference is **where the terrain boundary falls**:

| Aspect            | Blob (47-tile)          | Wang corner (16-tile)           |
| ----------------- | ----------------------- | ------------------------------- |
| Boundary location | On cell edges           | Through cell centers            |
| Tiles needed      | 47                      | 16                              |
| What each tile is | One terrain type        | Up to 4 terrain types blended   |
| Best for          | Walls, platforms, solid | Terrain, ground cover, biomes   |
| Inner corners     | Explicit tiles for them | Handled by corner matching      |
| Neighbor check    | 8 neighbors (bitmask)   | 4 corners (shared with adj.)    |
| Asset complexity  | More tiles, simpler art | Fewer tiles, complex transition |

They're complementary. A game might use blob patterns for dungeon walls (hard edges, clear boundaries) and Wang tiles for overworld terrain (soft grass-to-dirt blending). You can use both in the same project.

### The 16 / 47 / 256 Spectrum

| Tiles | Method       | Considers                                         | Trade-off                                   |
| ----- | ------------ | ------------------------------------------------- | ------------------------------------------- |
| 16    | Wang corner  | 4 corners, boundary through center                | Fewest tiles, smoothest transitions         |
| 16    | Minimal blob | 4 cardinal neighbors only, no diagonals           | Simple but inner corners look wrong         |
| 47    | Full blob    | 4 cardinal + 4 diagonal (with diagonal reduction) | Sweet spot for hard-edged terrain           |
| 256   | Exhaustive   | All 8 neighbors, no reduction                     | Overkill — extra tiles look identical to 47 |

## How It Works in LDtk

LDtk implements blob-style auto-tiling through **auto-layers**:

1. **IntGrid layer** — you paint abstract values (e.g., 1 = wall, 0 = empty)
2. **Auto-layer** — linked to the IntGrid, with a tileset and a set of rules
3. **Rules** — each rule defines a neighbor pattern (which neighbors must/must not be present) and which tile to place when that pattern matches

Rules are evaluated top-to-bottom, first match wins. You define 47 rules (one per pattern), assign each to the correct tile in your tileset, and then painting walls becomes: just paint "wall" on the IntGrid and every edge, corner, and interior tile resolves automatically.

### Reusing Rules Across Tilesets

If multiple tilesets use the same grid layout (same tile in the same position), you can duplicate an auto-layer and swap the tileset reference without redefining rules. Do this either:
- In the editor: duplicate the layer, change the tileset in layer settings
- In the JSON: copy the layer definition block and change `tilesetDefUid`

## How It Works in Tiled

Tiled uses the term **Wang sets** (formerly "terrain sets"). You define terrain types, then assign corner or edge terrain labels to each tile. When painting, Tiled picks tiles whose terrain labels match their neighbors. Tiled supports both corner-matching and edge-matching modes.

## Standard Tileset Layout

Most 47-tile blob tilesets follow a conventional layout on a spritesheet. Searching for "47 tile blob tileset template" yields reference images showing the standard arrangement. When creating new tilesets, following this layout means your LDtk rules transfer directly between tilesets.

## Further Reading

- Boris the Brave's [Classification of Tilesets](https://www.boristhebrave.com/2021/11/14/classification-of-tilesets/) — formal taxonomy of tileset types
- [Wang tile (Wikipedia)](https://en.wikipedia.org/wiki/Wang_tile) — mathematical background
- [Excalibur.js Autotiling Technique](https://excaliburjs.com/blog/Autotiling%20Technique/) — practical implementation walkthrough
