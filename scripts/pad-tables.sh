#!/usr/bin/env bash
# Align all markdown tables in the docs/ directory.
# Uses the pad_tables example which recursively finds .md files.

set -euo pipefail
cd "$(dirname "$0")/.."
cargo run --example pad_tables
