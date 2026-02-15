# Non-Programming Skills for Game Development

The non-programming work is often what kills solo game projects. This document lists what's needed beyond code and how to achieve it without the requisite skill set.

## Art

**What you need:**
- Sprites - player, enemies, items, projectiles, each needing multiple animation frames per direction. A Zelda-like Link needs ~20-30 frames minimum (4 directions x walk cycle + attack + death).
- Tilemap art - ground, walls, water, doors, stairs, decorative variants. A minimal tileset is 30-50 tiles.
- UI elements - hearts, item icons, font, menus.

**Options without drawing skill:**
- **Asset packs** - OpenGameArt.org, itch.io asset packs. Many free retro-style tilesets and character sprites exist specifically for Zelda-like games.
- **Aseprite** ($20) - pixel art editor designed for game sprites. Pixel art is more forgiving than high-res art because the low resolution hides imprecision. 16x16 pixel characters are achievable without art training.
- **LDtk or Tiled** - tilemap editors. You paint maps by placing tiles, no drawing involved. Both export formats Bevy can load.
- **AI image generation** - can produce concept art but struggles with consistent pixel-art sprite sheets. Not reliable for production assets yet.

## Sound Effects

**What you need:** sword swing, hit, enemy death, item pickup, door open, low health beep, menu select. A Zelda-like needs 20-40 distinct sounds.

**Options:**
- **sfxr/jsfxr** (free) - procedural retro sound generator. Click buttons, tweak sliders, export .wav. Designed exactly for this. You can generate all the sounds a retro game needs in an afternoon.
- **Freesound.org** - community library of free sound effects.
- **Record and process** - a surprising number of game sounds are everyday objects recorded and pitch-shifted.

## Music

**What you need:** overworld theme, dungeon theme, boss fight, game over, victory. 3-5 tracks minimum.

**Options:**
- **OpenGameArt.org** - free music tracks, many in retro style.
- **Mod trackers** (MilkyTracker, OpenMPT) - compose music by placing notes on a grid, not by playing an instrument. The tracker format is how most retro game music was actually made. Steep learning curve but no musical performance skill required.
- **AI music generation** - tools like Suno or Udio can generate retro-style game loops. Quality varies. Licensing varies.
- **Commission** - Fiverr, game dev communities. A few chiptune loops is a small job.
- **Placeholder silence** - ship without music, add it last. Many games play fine without music during development.

## Level/World Design

**What you need:** room layouts, enemy placement, item placement, difficulty curve, puzzle design, dungeon structure.

This is a design skill, not an art skill. It's about systems and flow, not aesthetics.

**Options:**
- **LDtk** (free) - level editor built for 2D games. Has Bevy integration. You design rooms visually by painting tiles and placing entity markers.
- **Graph paper / spreadsheet** - the original Zelda was designed on grid paper. Each screen is 16x11 tiles. You can plan room layouts in a spreadsheet before building them.
- **Play the original** - study what makes the original's world design work. Room pacing, how enemies are introduced, how keys gate progression.

## Game Feel / Polish

**What you need:** screen shake, hit flash, knockback, invincibility frames, particle effects, transitions.

This IS programming. These are systems you build in Bevy. The design intuition comes from playing games critically and noticing what happens when you hit an enemy, take damage, or pick up an item.

**Resource:** "Game Feel" by Steve Swink, or the YouTube talk "Juice it or lose it" - both explain how small effects make games feel responsive.

## Fonts

**What you need:** at least one font for UI text.

**Options:**
- **Google Fonts** - filter by "pixel" style. Many free options.
- Bevy's built-in text rendering handles .ttf/.otf files directly.

## What You Can Safely Ignore

- Story/writing - not needed for a learning project
- Localization - not needed
- Accessibility - important for real games, skip for learning
- Marketing assets - not shipping this

## Recommended Approach

Use free asset packs for art and sound. Spend your time on programming. If the game works with placeholder art, upgrading the visuals later is straightforward - swap the sprite sheet, the code doesn't change. The reverse (beautiful art, broken code) is much harder to fix.
