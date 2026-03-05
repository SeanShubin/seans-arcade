# Sprite and Spritesheet Metadata Formats

How sprite sheets describe their frames, regions, and animations. Ranges from "no metadata, the code just knows the grid" to rich skeletal animation runtimes.

## The Spectrum

| Approach               | Metadata file          | Who uses it                        | Complexity |
| ---------------------- | ---------------------- | ---------------------------------- | ---------- |
| Hardcoded grid offsets | None                   | Programmers with uniform sheets    | Lowest     |
| RPG Maker conventions  | None (filename-based)  | RPG Maker asset artists            | Low        |
| TexturePacker JSON/XML | Frame rects + names    | Artists handing off to programmers | Low        |
| Aseprite JSON export   | Frames + tags + timing | Pixel artists                      | Low        |
| Tiled TSX/TMX          | Tile properties + anim | Level designers                    | Medium     |
| Spine / DragonBones    | Bones + meshes + keys  | Animators doing skeletal 2D        | High       |

## No Metadata (Uniform Grid)

The sprite sheet is a uniform grid and the code selects frames by column/row index. No external file describes the layout — frame size and animation mapping are hardcoded.

- **Used by:** Programmers working with simple, regular sheets
- **Bevy API:** `TextureAtlasLayout::from_grid(tile_size, cols, rows, padding, offset)` or manual `Rect` with `Sprite::rect`
- **Limitation:** Every frame must be the same size. Animation timing and names live in code.

This is what `sprite_walk.rs` does — the ground tile is selected by `GROUND_TILE_COL` / `GROUND_TILE_ROW`, and character frames are individual PNGs rather than a sheet.

## RPG Maker Conventions (No Metadata File)

RPG Maker uses no external metadata file. The engine hardcodes the expected sheet layout, and filenames signal which layout variant to use. This matters because many free pixel art asset packs (including Time Fantasy, which this project uses) ship in RPG Maker format as their primary or only sheet layout.

### Character Sheets

Each character has a 3-column x 4-row grid of frames (3 walk frames x 4 directions). The direction order top-to-bottom is: down, left, right, up. The three columns are: stand, walk1, walk2 (or equivalently: left-foot, stand, right-foot).

**Multi-character sheets** pack 8 characters into one PNG in a 4x2 arrangement (4 characters across, 2 rows). The full image is therefore a 12-column x 8-row grid. Each character occupies a 3x4 sub-grid.

**Single-character sheets** are signaled by a `$` prefix in the filename (e.g., `$BigMonster.png`). These contain just one character's 3x4 grid, allowing larger sprite sizes.

**Filename prefixes:**
- `$` — single-character sheet (one 3x4 grid instead of the 4x2 multi-character layout)
- `!` — "above characters" flag (renders on top of the player; used for tall objects like doors)
- `!$` — both: single-character, rendered above

### Tileset Sheets (MV/MZ Tile Naming)

RPG Maker MV/MZ uses a lettered tile-slot system. Each slot expects a specific image size and layout:

| Slot   | Purpose                      | Layout                                                                                                 |
| ------ | ---------------------------- | ------------------------------------------------------------------------------------------------------ |
| **A1** | Animated tiles (water, lava) | Autotile sub-tiles arranged for 3-frame animation                                                      |
| **A2** | Ground tiles                 | Autotile format — each terrain is a block of sub-tiles the engine combines based on neighbor adjacency |
| **A3** | Wall top autotiles           | Autotile format for top-down wall edges                                                                |
| **A4** | Wall face autotiles          | Autotile format for wall faces (side view)                                                             |
| **A5** | Normal tiles (no autotile)   | Simple 16x16 grid — flowers, rocks, decorations on a ground                                            |
| **B**  | Object tiles layer 1         | Simple grid — fences, barrels, furniture, trees                                                        |
| **C**  | Object tiles layer 2         | Simple grid — building walls, structures                                                               |
| **D**  | Object tiles layer 3         | Simple grid — rooftops, overlays                                                                       |
| **E**  | Object tiles layer 4         | Simple grid — additional overlay objects                                                               |

**Autotile** means the engine automatically selects which sub-tile to draw based on neighboring tiles. The artist provides a block of sub-tile variations; the engine composes them. Slots A1–A4 use this system. Slots A5 and B–E are simple grids where each cell is one complete tile.

**Tile size:** 48x48 pixels in MV/MZ, 32x32 in VX/Ace, 16x16 in the original Time Fantasy source art.

### Icon Sheets

RPG Maker expects a single large icon sheet (e.g., 16 columns x N rows) where icons are indexed left-to-right, top-to-bottom. The engine references icons by numeric index. The sheet size determines how many icons are available:

- 32x32 icons for VX/Ace
- 24x24 icons for MV (MV's default icon size)
- 16x16 at native pixel-art resolution

### What This Means for Non-RPG-Maker Use

RPG Maker format assets have no metadata file to parse. To use them outside RPG Maker:

1. **Character sheets:** Know the grid layout (3 cols x 4 rows per character, 4x2 characters per sheet). Extract frames by grid math or split into individual PNGs.
2. **Autotile sheets (A1–A4):** Complex — the sub-tile arrangement follows RPG Maker's specific autotile algorithm. Using these outside RPG Maker requires either reimplementing autotile logic or manually picking individual tiles from the sheet.
3. **Simple tile sheets (A5, B–E):** Easy — treat as a uniform grid.
4. **Icon sheets:** Easy — treat as a uniform grid indexed by number.

Many asset artists (including Time Fantasy) provide multiple formats: RPG Maker sheets, generic tilesheets (simple 16x16 grids), and individual frame PNGs. The generic and individual formats are far easier to consume outside RPG Maker.

## Aseprite (.ase / .aseprite + JSON)

The most popular pixel art editor. Exports a packed sprite sheet PNG alongside a JSON metadata file describing every frame.

**JSON contains:**
- Frame rectangles (position and size in the packed sheet)
- Frame durations (per-frame timing)
- Tags (named animation ranges — "walk-down", "idle", "attack")
- Slices (named sub-regions within frames, used for hitboxes or anchor points)
- Layer info

**Two JSON variants:**
- **Array mode** — frames as an ordered list
- **Hash mode** — frames keyed by filename string

**Bevy integration:** Community crates (`bevy_aseprite_ultra`, `bevy_asepriti`) parse the JSON and build `TextureAtlasLayout` + animation data automatically.

**Key advantage:** The artist defines animation names, frame timing, and hitbox slices directly in the editor. The programmer consumes structured data instead of hardcoding offsets.

## TexturePacker (JSON / XML / many formats)

The industry-standard sprite packing tool. Takes individual images or folders, outputs a tightly packed sheet plus metadata in your choice of format (~30+ export formats including JSON, XML, CSS, Unity, Cocos2d, Godot, etc.).

**JSON contains:**
- Frame rectangles (position, size, whether the frame was rotated to pack tighter)
- Original source filename per frame
- Trimmed transparent pixels (original size vs trimmed size + offset)
- Pivot points (optional)

**Does NOT contain:** Animation definitions, timing, or tags. TexturePacker is a packing tool, not an animation tool — it answers "where is each frame?" but not "which frames form an animation?" That mapping lives in code or a separate file.

**Free alternatives:** Free Texture Packer, ShoeBox — similar output formats.

**Bevy integration:** Parse the JSON, build `TextureAtlasLayout` from the frame rects. No widely-adopted crate; straightforward to write a loader.

## Tiled (.tmx / .tsx)

A tilemap editor, not strictly a sprite tool, but defines tilesets with per-tile metadata that covers animation and collision.

**TSX (tileset) contains:**
- Tile size and image source
- Per-tile properties (custom key-value pairs — "walkable", "damage", "type")
- Per-tile collision shapes (polygons, rectangles)
- Per-tile animation sequences (list of frame tile IDs + durations)

**TMX (map) contains:**
- Tile layers (which tile ID goes where)
- Object layers (spawn points, trigger zones, paths)
- Layer properties and ordering

**Bevy integration:** `bevy_ecs_tilemap` and `bevy_ecs_tiled` load TMX/TSX files directly.

**Key advantage:** Level designers define collision, animation, and custom properties per tile in a visual editor. The code reads structured data instead of maintaining parallel lookup tables.

## Spine (.spine-json / .skel)

A commercial 2D skeletal animation runtime. Characters are hierarchies of image parts (head, torso, arms, legs) rigged to a bone skeleton and animated by transforming bones over time.

**Format contains:**
- Bone hierarchy (parent-child skeleton)
- Slots and attachments (which image piece attaches to which bone)
- Skins (swappable attachment sets — different armor, hair, weapons on the same skeleton)
- Animations as keyframed bone transforms (translate, rotate, scale per bone per time)
- Mesh deformation (for stretchy/bendy parts)
- Events (trigger points within animations — "footstep", "attack-hit")
- IK constraints, path constraints

**Two formats:**
- **JSON** — human-readable, larger
- **Binary (.skel)** — smaller, faster to parse

**Cost:** Editor license starts at $70 (Essential) / $340 (Professional). Runtime is open source.

**Bevy integration:** `bevy_spine` crate.

**Key advantage:** Smooth animation from far fewer source images than frame-by-frame. One skeleton supports many skins. Animations can blend and layer. Commonly used in commercial 2D games (Hollow Knight uses a similar approach).

**Key limitation:** The art style reads as "paper cutout" or "puppet" animation, distinct from pixel art frame-by-frame.

## DragonBones (JSON)

Open-source alternative to Spine. Same concept — 2D skeletal animation with bones, slots, skins, and keyframed transforms.

**Format:** JSON (documented open spec). Editor is free.

**Bevy integration:** Limited community support compared to Spine.

**Key advantage over Spine:** Free editor and open format. **Key disadvantage:** Smaller ecosystem, fewer tutorials, less polish.

## Summary: What to Choose

| Situation                                     | Format                  |
| --------------------------------------------- | ----------------------- |
| Uniform grid sheet, programmer-driven         | No metadata (grid math) |
| Asset pack made for RPG Maker                 | RPG Maker conventions   |
| Pixel artist using Aseprite                   | Aseprite JSON export    |
| Many individual images to pack efficiently    | TexturePacker JSON      |
| Tilemap with per-tile collision and animation | Tiled TSX/TMX           |
| Smooth skeletal 2D animation (commercial)     | Spine                   |
| Smooth skeletal 2D animation (free)           | DragonBones             |

This project built a custom TOML-based format for sprite sheet metadata — see the [format spec](../architecture/sprite-metadata-format.md) and [pipeline workflow](../architecture/sprite-pipeline.md). Tiled becomes relevant when building tilemaps with collision data.

## Appendix: Time Fantasy Asset Pack Layout

The project uses Time Fantasy asset packs by finalbossblues (timefantasy.net). The source ZIPs live in `D:\keep\assets\unzipped\` and the project copies the usable parts into `assets/external/`. Understanding the directory structure requires knowing the RPG Maker conventions above.

### Source Packs

#### `TimeFantasy_TILES_6.24.17/` — Tile Pack

```
TILESETS/              ← Generic format: simple 16x16 grids, easy to use anywhere
  world.png              terrain, buildings, castles for the world map
  outside.png            outdoor objects: fences, trees, barrels, rocks
  terrain.png            ground terrain variations
  castle.png             castle interiors and walls
  desert.png             desert terrain
  dungeon.png            cave/ruin interiors
  house.png              house interiors
  inside.png             general interior tiles
  water.png              water tiles (8x8 autotile chunks for RPG Maker)
  animated/              multi-frame tiles
    doors.png              door open/close animation frames
    fireplace.png          fireplace animation frames
    puzzle.png             switch/trap animation frames
    torch.png              torch animation frames

RPGMAKER/              ← RPG Maker format: autotile layout, harder to use outside RM
  RMMV/                  48x48 upscaled for RPG Maker MV
    tileA1_*.png           animated autotiles (water with different borders)
    tileA2_*.png           ground autotiles (terrain transitions)
    tileA4_*.png           wall autotiles
    tileA5_*.png           simple ground decoration grids (flowers, rocks)
    tileB_*.png            object tiles (trees, furniture, fences)
    tileC_*.png            building tiles (house walls)
    tileD_*.png            overlay tiles (rooftops in various colors, castles)
  RMVX/                  32x32 for RPG Maker VX/Ace (same slot naming)
```

**For non-RPG-Maker use:** The `TILESETS/` folder is the one to use. These are plain 16x16 grids — pick tiles by grid coordinate. The `RPGMAKER/` folder is only useful if you reimplement autotile logic.

#### `timefantasy_characters/` — Character Pack

```
frames/                ← Individual PNGs per frame — easiest to use anywhere
  base/                  template character (naked base sprite)
    down_stand.png, down_walk1.png, down_walk2.png
    up_stand.png, up_walk1.png, up_walk2.png
    left_stand.png, left_walk1.png, left_walk2.png  (no right — mirror left)
    right_stand.png, right_walk1.png, right_walk2.png
  chara/                 hero/villain characters (chara2_1 through chara5_8)
    chara2_1/              each subfolder has: {dir}_stand.png, {dir}_walk1.png,
    chara2_2/              {dir}_walk2.png for 4 directions, plus emotes:
    ...                    laugh1-3.png, nod1-3.png, shake1-3.png, pose1-3.png,
                           surprise.png
  military/              soldiers/knights (same frame naming)
  npc/                   townspeople (same frame naming)
  animals/               cats and dogs
  bonus1/                bonus characters from timefantasy.net
  chests/                treasure chest open/close frames

sheets/                ← RPG Maker multi-character sheets (3x4 per char, 8 chars per sheet)
  chara2.png             8 characters in one 12-col x 8-row grid
  chara3.png             (number matches the character group)
  npc1.png, npc2.png     NPC sheets
  military1.png          military character sheets
  $tf_template.png       single-character template ($ prefix = one 3x4 grid)
  animals1.png, chests.png, emote*.png, animation*.png

RPGMAKERMV/            ← 48x48 upscaled versions of the sheets for RPG Maker MV
  characters/            same filenames as sheets/, upscaled 3x
    !$fireplace.png      ! prefix = renders above player, $ = single-character
    !$torch.png
    !doors.png           ! prefix, multi-character layout
    chara1.png ... chara5.png
    emote1.png ... emote5.png
    military1.png ... military3.png
    npc1.png ... npc4.png
  expansion/             bonus content (animals, fairies, lava animation, etc.)
```

**For non-RPG-Maker use:** The `frames/` folder is the easiest — individual PNGs named by direction and frame. This is what `sprite_walk.rs` uses. The `sheets/` folder works if you do grid math (3 cols x 4 rows per character, 8 characters per sheet).

#### `icons_12.26.19/` and `icons_8.13.20/` — Icon Packs

```
fullcolor/             ← Normal colors, best for general use
  icons_full_16.png      single sheet, 16x16 per icon
  icons_full_24.png      single sheet, 24x24 (RPG Maker MV size)
  icons_full_32.png      single sheet, 32x32 (RPG Maker VX/Ace size)
  individual_16x16/      individual icon PNGs: icon001.png through icon999+.png
  individual_24x24/
  individual_32x32/

tfcolor/               ← Muted palette matching Time Fantasy art style
  tficons_limited_16.png   same layout, muted colors
  tficons_limited_24.png
  tficons_limited_32.png
  individual_*/            same individual icons, muted colors

(icons_8.13.20 also has:)
updated_layout/        ← Reorganized sheets with buff/debuff icons in RPG Maker's expected positions
  icons_full_16.png
  icons_full_32.png
  icons_tf_16.png
  icons_tf_32.png
```

**Icon categories** (top to bottom in the sheet — see `quick_guide.png`): UI/HUD, skills/magic/elements, potions/herbs/containers/food, general items (keys, coins, books, scrolls), weapons, shields/armor/equipment/accessories, faction banners/flags, crafting/mining/fishing/farming items and gemstones.

**For non-RPG-Maker use:** Use the `individual_*` folders for single icons, or the full sheets with grid math (icons indexed left-to-right, top-to-bottom).

### What the Project Uses

The project copies into `assets/external/`:

| Project path                   | Source                                | Format used                |
| ------------------------------ | ------------------------------------- | -------------------------- |
| `external/timefantasy/`        | `timefantasy_characters/frames/`      | Individual PNGs            |
| `external/timefantasy_tiles/`  | `TimeFantasy_TILES_6.24.17/TILESETS/` | Generic 16x16 grid         |
| `external/timefantasy_icons1/` | `icons_12.26.19/`                     | Both sheets and individual |
| `external/timefantasy_icons2/` | `icons_8.13.20/`                      | Both sheets and individual |

The project uses the non-RPG-Maker formats exclusively — individual character frames and generic tilesheets. The RPG Maker formatted sheets and autotile images are not used.
