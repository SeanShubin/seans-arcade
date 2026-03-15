# Autotile Blob Patterns (47-Tile Template)

## The Problem

When building tile-based maps, a single "wall" tile isn't enough. A wall tile needs different visual variants depending on what's next to it: top edges, corners, interior fills, isolated blocks, peninsulas, etc. Manually placing each variant is tedious and error-prone.

## Neighbor Configurations

Each tile in a grid has 8 neighbors: 4 cardinal (N/S/E/W) and 4 diagonal (NE/NW/SE/SW).

With 8 neighbors, each either same-type or different, there are 2^8 = 256 possible configurations. But most of those are visually redundant.

## Why 47 and Not 256

A diagonal neighbor only matters visually when both adjacent cardinal neighbors are also filled. Consider the NE diagonal: if the N or E neighbor is empty, there's already a visible edge there, and whether NE is filled or not doesn't change the tile's appearance. The diagonal only creates a visible inner corner when both N and E are filled but NE is empty.

This constraint collapses 256 configurations down to **47 visually distinct patterns**.

The 47 patterns cover every possible situation:

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

## How It Works in LDtk

LDtk implements this through **auto-layers**:

1. **IntGrid layer** — you paint abstract values (e.g., 1 = wall, 0 = empty)
2. **Auto-layer** — linked to the IntGrid, with a tileset and a set of rules
3. **Rules** — each rule defines a neighbor pattern (which neighbors must/must not be present) and which tile to place when that pattern matches

Rules are evaluated top-to-bottom, first match wins. You define 47 rules (one per pattern), assign each to the correct tile in your tileset, and then painting walls becomes: just paint "wall" on the IntGrid and every edge, corner, and interior tile resolves automatically.

### Reusing Rules Across Tilesets

If multiple tilesets use the same grid layout (same tile in the same position), you can duplicate an auto-layer and swap the tileset reference without redefining rules. Do this either:
- In the editor: duplicate the layer, change the tileset in layer settings
- In the JSON: copy the layer definition block and change `tilesetDefUid`

## Standard Tileset Layout

Most 47-tile blob tilesets follow a conventional layout on a spritesheet. Searching for "47 tile blob tileset template" yields reference images showing the standard arrangement. When creating new tilesets, following this layout means your LDtk rules transfer directly.

## Relationship to Other Tile Counts

| Tiles | Method  | Considers                                         |
| ----- | ------- | ------------------------------------------------- |
| 16    | Minimal | 4 cardinal neighbors only, no diagonals           |
| 47    | Blob    | 4 cardinal + 4 diagonal (with diagonal reduction) |
| 256   | Full    | All 8 neighbors, no reduction                     |

The 16-tile approach is simpler but produces visible artifacts at inner corners. The 256-tile approach is overkill — the extra tiles are visually identical to one of the 47. The 47-tile blob is the sweet spot.

## Tools

- **LDtk** — auto-layer rules with visual pattern editor
- **Tiled** — supports Wang tiles (similar concept, different terminology)
- **RPG Maker** — hardcoded autotile patterns (less flexible than LDtk's user-defined rules)
