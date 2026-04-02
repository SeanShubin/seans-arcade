# LDtk 47-Tile Blob Autotile Layout

## Overview

LDtk's auto-layer rule wizard generates 47 rules for blob autotiling in a specific
layout. This layout was manually verified by matching each rule's 3×3 pattern in
the LDtk UI to its grid position.

## Grid Dimensions

- **12 columns × 5 rows** (60 cells, 47 filled, 13 blank)
- Column 5 is always blank (visual separator between left and right sections)
- Left section: columns 0–4 (5 wide)
- Right section: columns 6–11 (6 wide, with some blanks)

## Bitmask Convention

Neighbor bits, clockwise from N:

```
NW=128  N=1   NE=2
W=64          E=4
SW=32   S=16  SE=8
```

Diagonal bits are only meaningful when both adjacent cardinals are set
(e.g., NE only matters if N and E are both present).

## Layout Table

```
Col  Row  Mask  Binary      Neighbors
───  ───  ────  ──────────  ─────────────────────
 0    0    28   00011100    E SE S
 1    0   124   01111100    W SW S SE E
 2    0   112   01110000    W SW S
 3    0     4   00000100    E
 4    0    68   01000100    W E
 5    0    64   01000000    W
 6    0    92   01011100    W S SE E
 7    0   116   01110100    W SW S E
 8    0    95   01011111    W N NE E SE S
 9    0   245   11110101    N NW W SW S E
10    0    93   01011101    W N E S SE
11    0   117   01110101    N E S W SW

 0    1    31   00011111    N NE E SE S
 1    1   255   11111111    all
 2    1   241   11110001    N W S NW SW
 3    1     0   00000000    (none)
 4    1    16   00010000    S
 6    1    71   01000111    W N E NE
 7    1   197   11000101    W N E NW
 8    1   215   11010111    N S E W NW NE
 9    1   125   01111101    N S E W SW SE
10    1    87   01010111    N S E W NE
11    1   213   11010101    N S E W NW

 0    2     7   00000111    N NE E
 1    2   199   11000111    W N E NW NE
 2    2   193   11000001    W N NW
 3    2    85   01010101    N S E W
 4    2    17   00010001    N S
 6    2    29   00011101    N E S SE
 7    2   113   01110001    N W S SW
 8    2    21   00010101    N E S
 9    2    84   01010100    W S E
10    2   119   01110111    N S E W SW NE

 0    3   127   01111111    N S E W NE SW SE
 1    3   253   11111101    N S E W NW SW SE
 2    3    20   00010100    S E
 3    3    80   01010000    W S
 4    3     1   00000001    N
 6    3    23   00010111    N E S NE
 7    3   209   11010001    N W S NW
 8    3    69   01000101    W N E
 9    3    81   01010001    N S W
10    3   221   11011101    N S E W NW SE

 0    4   223   11011111    N S E W NW NE SE
 1    4   247   11110111    N S E W NE NW SW
 2    4     5   00000101    N E
 3    4    65   01000001    W N
```

Blank cells: (5,0) through (5,4), (11,2), (11,3), (4,4) through (11,4).

## Visual Organization

The layout groups tiles by visual similarity:

**Left section (cols 0–4):**
- Row 0: SE corner, full S edge, SW corner, E endcap, horizontal pipe
- Row 1: full E edge, center (all), full W edge, isolated, S endcap
- Row 2: NE corner, full N edge, NW corner, 4-inner-corners, vertical pipe
- Row 3: 3-inner (missing NW), 3-inner (missing NE), SE corner (no diag), SW corner (no diag), N endcap
- Row 4: 3-inner (missing SW), 3-inner (missing SE), NE corner (no diag), NW corner (no diag)

**Right section (cols 6–11):**
- Row 0: W endcap, S edge variants (with/without diags), complex edge combos
- Row 1: N edge variants, 2-inner-corner pairs (diagonal and adjacent)
- Row 2: edge+corner combos, 3-cardinal variants, diagonal inner corner pair
- Row 3: corner+diag variants, 3-cardinal endcaps, diagonal inner corner pair
- Row 4: (empty)

## Was This Derivable?

**No, this layout could not be reliably derived from the LDtk project file.**

The LDtk auto-layer rules use a pattern-matching system with wildcards (`-1`)
and a priority cascade (`breakOnMatch: false`, last match wins). The rules
do NOT map 1:1 to the 47 canonical blob masks because:

1. **Broad rules catch multiple masks**: The first rule (`[0,-1,0,-1,1,-1,0,-1,0]`)
   matches ANY cell where all 4 diagonal neighbors are empty, regardless of
   which cardinals are present. This covers 16 different canonical masks.

2. **Actual cell values vs canonical masks**: LDtk rules check actual neighbor
   cell values, not diagonal-gated canonical values. Two cells with the same
   canonical mask but different actual diagonal values can match different rules.

3. **The layout is a UI property**: The 12×5 grid arrangement is determined by
   the LDtk rule editor's panel layout, which depends on the panel width and
   how LDtk wraps rule boxes. This is not stored in the `.ldtk` file.

4. **Cross-referencing IntGrid data was insufficient**: The reference level only
   exercised 25 of 47 configurations, and the rule simulation produced many-to-one
   mappings due to the wildcard patterns.

The layout had to be manually mapped by visually matching each rule's 3×3 pattern
in the LDtk UI to its grid position.
