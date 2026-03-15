# Torus Position Wrapping: Premature Representation Collapse

**Date:** 2026-03-13

## Symptom

Three successive bugs in `examples/screen_transition.rs`, each fix revealing the next:

1. **Scale 10 on Y axis (10 rows):** Camera teleports ~2048px when avatar crosses the point diametrically opposite the camera home position.
2. **After fix #1:** Camera scrolls 1:1 with avatar instead of using dead-zone/buffer behavior when the view matches or exceeds the arena dimension.
3. **After fix #2:** Avatar visually warps to the opposite side of the screen at scales exceeding the arena dimension (e.g., scale 11+ on Y with 10 rows).

## Root Cause

The arena is a torus (positions wrap). There are two ways to represent positions on a torus:

1. **Quotient space:** Force positions into `[0, period)` via `rem_euclid`. Every distance/offset calculation must use `wrap_offset` to account for the discontinuity at the boundary.
2. **Covering space:** Positions live on an unbounded plane. Points that differ by the period are equivalent, but that equivalence is only exploited when needed (rendering).

The original code used approach #1: `move_avatar` wrapped the avatar to `[0, ARENA_PX)`, `update_camera` wrapped the camera position, and every offset computation used `wrap_offset(delta, period)`. This is **premature representation collapse** — a projection that belongs at the rendering boundary was baked into the data model.

### Why it kept breaking

`wrap_offset` maps a real-number offset to `[-period/2, period/2)`. This is a mathematical discontinuity — there is no continuous map from a circle to a line segment. At small scales, the avatar never reaches the discontinuity (±period/2 from home), so it stays dormant. Each bug appeared when the view grew large enough to expose it:

**Bug 1:** At scale 10 on Y (view = arena height), `view_half = period/2`. The avatar can reach the discontinuity. When `wrap_offset` jumps from `+5119` to `-5120`, `axis_scroll` flips from positive to negative scroll, jerking the camera.

**Bug 2:** The fix for bug 1 special-cased `view >= arena` to track the avatar directly (`cam_pos = avatar`), bypassing the dead-zone logic. This changed behavior — the camera scrolled constantly instead of only in buffer zones.

**Bug 3:** After switching to unbounded positions for camera math, `wrap_avatar` still used `wrap_offset(delta, ARENA_PX_H)` to position the avatar relative to the camera. At scale 11+, the avatar's legitimate offset from the camera exceeded `period/2`, so `wrap_offset` snapped it to the wrong side.

Each fix peeled back one layer of the original wrapping but left others in place. Every remaining `wrap_offset` or `rem_euclid` in non-rendering code was a latent bug waiting for a scale large enough to trigger it.

## Fix

Remove all position wrapping from movement and camera logic. Positions are unbounded:

- `move_avatar`: adds delta directly, no `rem_euclid`
- `update_camera`: uses plain subtraction (`avatar - home`), no `wrap_offset`
- `axis_home`: shifts home by `±TILE_PX` with simple arithmetic
- `snap_home`: rounds to nearest grid point, no wrapping

Wrapping only happens in rendering systems:
- `wrap_tiles`: uses `wrap_offset` to position tiles relative to camera (one period)
- Ghost tile entities fill additional copies beyond one period
- `update_window_title`: uses `rem_euclid` to map position → tile grid for display

## The Smell

**When internal math repeatedly inverts a transformation you applied to your own data, the transformation is on the wrong side of the abstraction boundary.**

The code applied `rem_euclid` to positions (collapsing the covering space to the quotient space), then every consumer had to call `wrap_offset` to recover the information that was discarded (signed distance on the torus). `wrap_offset` is literally the inverse of `rem_euclid` for small deltas — and it was called in `axis_home`, `update_camera`, `wrap_avatar`, and `snap_home`. Four call sites, all undoing the same transformation, all vulnerable to the same discontinuity.

## Rule

**Represent the full unconstrained state internally. Apply projections only at boundaries (rendering, serialization, display).**

The torus equivalence is a rendering concern: which tiles to draw where. It is not a property of the avatar's position or the camera's position. By collapsing the equivalence into the position representation, every system that computed distances or offsets had to reason about wrapping — and every such system was a place where wrapping could go wrong.

The signal to watch for: if you find yourself writing an inverse function (`wrap_offset`) and calling it from multiple systems to undo a transformation you applied in an earlier system (`rem_euclid`), the transformation is in the wrong place. Move it to the boundary.
