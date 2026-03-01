# Bevy Learnings

## egui Sync Ordering (Immediate-Mode UI)

When multiple egui panels need to stay in sync (e.g. color space sliders), syncing
*after* rendering means non-active panels show stale values for a frame. Fix: record
which panel changed this frame, then sync at the *top* of the next frame before any
panels render.

## Color Space Conversions

### Route through a single hub
Bevy's direct conversion paths can differ from chained ones (e.g. `Oklcha → Hsla`
vs `Oklcha → Srgba → Hsla` produce different results). Always convert to `Srgba`
first, then derive all other spaces from that single value.

### Clamp at the sRGB gamut boundary
Wide-gamut spaces (OKLCH, etc.) can produce sRGB values outside [0, 1]. Feeding
out-of-gamut values into HSL conversion produces garbage. Clamp to [0, 1] before
deriving other spaces.

### Don't write back to the source panel
Round-tripping a value through another color space introduces floating-point drift.
If you write the drifted value back into the source panel's sliders, egui reports
`changed`, which triggers another sync — infinite feedback loop. Skip the source
panel when writing back converted values.

## Sprite Sheet Seams (Rect Sampling)

When using `Sprite { rect: Some(...), .. }` to select a sub-region from a sprite
sheet, placing the rect exactly on pixel boundaries (e.g. `Rect::new(32.0, 16.0,
48.0, 32.0)`) can produce single-pixel black lines between tiled sprites.

### Why it happens

The GPU maps rect coordinates to texture coordinates (UVs) to decide which texels
to sample. With nearest-neighbor filtering, each screen pixel picks the single
closest texel. When a sample point lands exactly on the boundary between two texels,
floating-point rounding decides which side wins. At the edges of the sprite quad the
interpolated UV can drift slightly outside the intended range, sampling the
neighboring tile's pixel or transparent/black at the sheet edge.

### Fix: inset the rect and set custom_size

Insetting the rect alone shrinks the rendered sprite (e.g. from 16x16 to 15.8x15.8
world units), which introduces real gaps. The rect controls both the sampled region
*and* the default rendered size, so these two concerns must be decoupled.

Use `rect` with an inset for sampling, and `custom_size` to force the sprite back to
the exact tile dimensions:

    let inset = 0.1;
    let tile_rect = Rect::new(
        col * TILE_SIZE + inset,
        row * TILE_SIZE + inset,
        (col + 1.0) * TILE_SIZE - inset,
        (row + 1.0) * TILE_SIZE - inset,
    );

    Sprite {
        image: ground_image.clone(),
        rect: Some(tile_rect),
        custom_size: Some(Vec2::splat(TILE_SIZE)),
        ..default()
    }

- **`rect`** — inset by 0.1 texels to avoid boundary sampling at high zoom (7x+).
- **`custom_size`** — forces the sprite to render at exactly TILE_SIZE × TILE_SIZE
  world units, eliminating gaps between tiles.

### Alternatives

- **1 px padding** between tiles in the sheet — wastes space, requires modifying
  the asset.
- **Per-sprite sampler clamping** — not available in Bevy's default pipeline.
- **Epsilon inset + custom_size** — simplest, no asset changes needed.
