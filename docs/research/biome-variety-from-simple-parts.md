# Biome Variety from Simple Component Parts

How to maximize perceived biome variety from the fewest procedural inputs.

## The Multiplication Principle

With N noise axes and K divisions each, you get up to K^N biomes. Most combinations are nonsensical ("underwater desert"), so the real count is always less. The goal is to maximize the ratio of meaningful combinations to total parameters.

## What Axes Buy You

**Elevation** (mandatory) — The only axis with hard physical rules. Water flows down. ~4 natural bands: submerged, shoreline, land, peak.

**Moisture** — Highest-value second axis. At every elevation band, moisture creates 2-3 meaningful splits (dry/moderate/wet). 4 x 3 = 12 biomes from two noise layers.

**Drainage** (Dwarf Fortress's key insight) — Only matters where moisture is already high. High rain + low drainage = swamp. High rain + high drainage = forest. Doesn't subdivide deserts. Adds 4-5 new biomes, not a full multiplicative explosion. One noise layer, a few threshold checks.

**Temperature** — In a top-down fantasy world without latitude, mostly redundant with elevation (higher = colder). Could decouple for magical regions (frozen lowland, volcanic hot peak). Probably 2 divisions (warm/cold) that modify existing biomes rather than creating new categories.

## Diminishing Returns

| Axes | Theoretical max | Meaningful biomes | New noise layers |
|------|----------------|-------------------|-----------------|
| 1 (elevation) | 4 | 4 | 1 |
| 2 (+moisture) | 12 | 10-12 | 1 |
| 3 (+drainage) | 36 | 15-18 | 1 |
| 4 (+temperature) | 72 | 20-25 | 1 (or derived) |
| 5+ | 144+ | diminishing fast | 1 each |

After 3 axes, most new combinations are "the same biome but slightly different." Each distinct biome needs a distinct autotile set to feel different.

## Non-Noise Multipliers

Instead of more noise axes, these recontextualize existing biomes cheaply:

**Age/erosion** — Same rocky terrain looks different young (sharp, angular) vs old (rounded, weathered). A tint/texture modifier, not a new biome. One parameter, applied to all biomes.

**Magical corruption** — Fantasy equivalent of Dwarf Fortress's Good/Evil axis. Corrupted forest shares base tileset with palette shift and overlay effects. Multiplies visual variety without new autotile sets.

**Season** — Every biome gets 2-4 visual variants for free. Same tiles, different color ramps.

**Civilization influence** — Farmland is "grassland + human intent." Roads are "any biome + path overlay." Layers on top of biomes rather than replacing them.

## Recommended Setup

**3 noise layers** (elevation, moisture, drainage) = ~15-18 base biomes

**2 overlay systems** (corruption level, civilization influence) = each multiplies visual variety without new autotile generation

**1 color ramp modifier** (season or time-of-day) = free visual variety from the same tiles

Result: 15 base biomes x 2-3 corruption states x seasonal variation = visually feels like 60+ environments, built from 3 noise layers, ~15 autotile sets, and palette math.

**Key insight:** Noise axes create new biomes (expensive — each needs a tileset). Overlay systems multiply perceived variety (cheap — palette shifts and additive effects on existing tilesets).

## Dwarf Fortress Reference

Dwarf Fortress uses 6 parameters (elevation, rainfall, drainage, temperature, salinity, volcanism) plus 2 overlay axes (savagery, alignment). The core is a Rainfall x Drainage grid that temperature then modifies into climate variants. This produces ~40+ distinct biome types.

The critical design choice: **parameter interaction, not enumeration**. DF doesn't define 40 biomes and place them. It defines continuous fields and lets biomes emerge from their intersection. Biome boundaries form naturally along parameter gradients.

See also: [algorithmic-rendering-theory.md](algorithmic-rendering-theory.md) for the broader theory roadmap.
