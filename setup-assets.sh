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

TF_SRC="$ASSET_BASE/timefantasy_characters/frames"
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

echo ""
echo "Done. External assets are in: assets/external/"
