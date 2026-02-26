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
