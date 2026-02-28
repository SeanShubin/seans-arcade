#!/usr/bin/env bash
#
# Copies external assets into assets/external/ for local development.
# These files are gitignored and not committed to version control.
#
# Usage: ./setup-assets.sh /path/to/assets
#
# The argument should point to the root asset directory containing
# third-party asset packs (e.g. D:\keep\assets).

set -euo pipefail

if [ $# -lt 1 ]; then
    echo "Usage: $0 <asset-base-path>"
    echo "Example: $0 'D:\\keep\\assets'"
    exit 1
fi

ASSET_BASE="$1"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEST="$SCRIPT_DIR/assets/external"

# ---- Time Fantasy Characters ------------------------------------------------
# Source: timefantasy_characters asset pack
# Contents: individual frame PNGs for all characters, animals, NPCs, etc.
# Skipped: sheets/ and RPGMAKERMV/ (redundant formats), frames/base/ (artist template)

TF_SRC="$ASSET_BASE/unzipped/timefantasy_characters/frames"
TF_DEST="$DEST/timefantasy"

if [ ! -d "$TF_SRC" ]; then
    echo "ERROR: Time Fantasy frames not found at: $TF_SRC"
    exit 1
fi

echo "Copying Time Fantasy characters..."
mkdir -p "$TF_DEST"

for category in animals bonus1 chara chests military npc; do
    src_dir="$TF_SRC/$category"
    if [ -d "$src_dir" ]; then
        rm -rf "$TF_DEST/$category"
        cp -r "$src_dir" "$TF_DEST/$category"
        count=$(find "$TF_DEST/$category" -type f -name "*.png" | wc -l)
        echo "  $category: $count frames"
    fi
done

# ---- Time Fantasy Tilesets ----------------------------------------------------
# Source: TimeFantasy_TILES_6.24.17 asset pack
# Contents: sprite sheet PNGs (16x16 grid) for terrain, buildings, interiors, etc.
# Skipped: RPGMAKER/ (redundant format), guide.png and READMEs (documentation)

TF_TILES_SRC="$ASSET_BASE/unzipped/TimeFantasy_TILES_6.24.17/TILESETS"
TF_TILES_DEST="$DEST/timefantasy_tiles"

if [ ! -d "$TF_TILES_SRC" ]; then
    echo "ERROR: Time Fantasy tilesets not found at: $TF_TILES_SRC"
    exit 1
fi

echo "Copying Time Fantasy tilesets..."
rm -rf "$TF_TILES_DEST"
mkdir -p "$TF_TILES_DEST"

cp "$TF_TILES_SRC"/*.png "$TF_TILES_DEST/"
count=$(find "$TF_TILES_DEST" -maxdepth 1 -type f -name "*.png" | wc -l)
echo "  tilesets: $count sheets"

if [ -d "$TF_TILES_SRC/animated" ]; then
    cp -r "$TF_TILES_SRC/animated" "$TF_TILES_DEST/animated"
    count=$(find "$TF_TILES_DEST/animated" -type f -name "*.png" | wc -l)
    echo "  animated: $count sheets"
fi

# ---- Time Fantasy Icons (Dec 2019) -------------------------------------------
# Source: icons_12.26.19 asset pack
# Contents: fullcolor/ and tfcolor/ icon directories (sheets + individual 16/24/32px)
# Skipped: quick_guide.png, readme.txt (documentation)

TF_ICONS1_SRC="$ASSET_BASE/unzipped/icons_12.26.19"
TF_ICONS1_DEST="$DEST/timefantasy_icons1"

if [ ! -d "$TF_ICONS1_SRC" ]; then
    echo "ERROR: Time Fantasy icons (12.26.19) not found at: $TF_ICONS1_SRC"
    exit 1
fi

echo "Copying Time Fantasy icons (12.26.19)..."
rm -rf "$TF_ICONS1_DEST"
mkdir -p "$TF_ICONS1_DEST"

for subdir in fullcolor tfcolor; do
    if [ -d "$TF_ICONS1_SRC/$subdir" ]; then
        cp -r "$TF_ICONS1_SRC/$subdir" "$TF_ICONS1_DEST/$subdir"
        count=$(find "$TF_ICONS1_DEST/$subdir" -type f -name "*.png" | wc -l)
        echo "  $subdir: $count files"
    fi
done

# ---- Time Fantasy Icons (Aug 2020) -------------------------------------------
# Source: icons_8.13.20 asset pack
# Contents: fullcolor/, tfcolor/, and updated_layout/ icon directories
# Skipped: quick_guide.png, readme.txt (documentation)

TF_ICONS2_SRC="$ASSET_BASE/unzipped/icons_8.13.20"
TF_ICONS2_DEST="$DEST/timefantasy_icons2"

if [ ! -d "$TF_ICONS2_SRC" ]; then
    echo "ERROR: Time Fantasy icons (8.13.20) not found at: $TF_ICONS2_SRC"
    exit 1
fi

echo "Copying Time Fantasy icons (8.13.20)..."
rm -rf "$TF_ICONS2_DEST"
mkdir -p "$TF_ICONS2_DEST"

for subdir in fullcolor tfcolor updated_layout; do
    if [ -d "$TF_ICONS2_SRC/$subdir" ]; then
        cp -r "$TF_ICONS2_SRC/$subdir" "$TF_ICONS2_DEST/$subdir"
        count=$(find "$TF_ICONS2_DEST/$subdir" -type f -name "*.png" | wc -l)
        echo "  $subdir: $count files"
    fi
done

echo ""
echo "Done. External assets are in: assets/external/"
