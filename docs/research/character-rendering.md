# Character Rendering

How to render the main character in a Zelda-like game. This is an open question — prototyping is needed to evaluate the tradeoffs in practice.

## Goals

1. **Zelda-like aesthetic** — the game should feel like a top-down action-adventure
2. **Camera flexibility** — support top-down 2D and isometric 3D views from the same assets, without re-authoring art per camera angle
3. **Resolution independence** — art scales cleanly to any display resolution without becoming blurry or losing quality
4. **No art skill investment** — use free assets, asset packs, or tools approachable to a non-artist; don't depend on developing drawing or modeling skills
5. **Parallax and depth** — visual depth through layering or real 3D, not flat tilemaps

## Tensions Between Goals

**Goal 1 vs. Goal 3 (Zelda aesthetic vs. resolution independence)** — The classic Zelda look is pixel art. Pixel art is inherently resolution-dependent: its charm comes from the fixed pixel grid. Scaling pixel art produces either blurry (bilinear filtering) or chunky (nearest-neighbor) results. Resolution independence requires smooth-scaling art, which looks fundamentally different from pixel art.

**Goal 1 vs. Goal 2 (Zelda aesthetic vs. camera flexibility)** — Pixel art sprites are authored for a specific viewing angle. A 4-directional walk cycle drawn for top-down does not work at an isometric angle. Camera flexibility requires either 3D models (which view correctly from any angle) or re-drawing sprites per viewpoint (which multiplies art requirements).

**Goal 2 vs. Goal 4 (camera flexibility vs. no art investment)** — 3D models provide camera flexibility, but free 3D character assets suitable for top-down Zelda-likes are rare. The more specific the visual requirements, the less likely off-the-shelf assets exist. 2D sprite packs for Zelda-likes are abundant; 3D equivalents are not.

**Goal 3 vs. Goal 1 (resolution independence vs. Zelda aesthetic)** — Same as 1 vs. 3. These are the most directly opposed goals. Achieving both requires a rendering technique (like HD-2D shaders) that bridges the gap at significant engineering cost.

Goals 4 and 5 are not in tension with each other or with most other goals — they constrain the solution space but don't conflict.

## Approaches

### A: 2D Sprite Sheets (Traditional Pixel Art)

Frame-by-frame animation from sprite sheets. The classic approach for 2D games.

| Goal | Status |
|------|--------|
| Zelda-like aesthetic | Yes — this is the Zelda look |
| Camera flexibility | No — sprites are drawn for one viewing angle |
| Resolution independence | No — pixel art is fixed resolution |
| No art skill investment | Yes — large free asset library (itch.io, OpenGameArt) |
| Parallax and depth | Partial — manual layer separation, faked depth |

**Tools:** Aseprite (sprite authoring), Bevy `TextureAtlas` + `AnimationIndices` (runtime).

### B: 3D Models, Isometric/Top-Down Camera

Polygonal 3D models viewed from above. Camera angle is a runtime parameter.

| Goal | Status |
|------|--------|
| Zelda-like aesthetic | No — reads as a 3D game, not a Zelda-like |
| Camera flexibility | Yes — camera angle is just a transform |
| Resolution independence | Yes — 3D renders at native resolution |
| No art skill investment | Partial — free low-poly packs exist (Kaykit, Kenney, Quaternius) but selection is limited for Zelda-like characters; Mixamo provides free humanoid animations |
| Parallax and depth | Yes — real 3D depth, parallax is free |

**Tools:** Blender (modeling), Mixamo (animation), Bevy `Scene` + `AnimationPlayer` (runtime).

**Key problem:** A 3D model viewed from directly above shows the top of the character's head. Silhouette collapses to a circle. The face, armor, weapon detail — all invisible. Isometric angles (30-45 degrees) are where 3D models become readable, but this moves away from the classic top-down Zelda view.

### C: 3D Models with Pixel Art Shader (HD-2D)

3D geometry rendered through a shader that produces a pixel art aesthetic. Used by Octopath Traveler, Triangle Strategy.

| Goal | Status |
|------|--------|
| Zelda-like aesthetic | Partial — evokes retro feel but reads as its own style, not classic Zelda |
| Camera flexibility | Yes — underlying geometry is 3D |
| Resolution independence | Yes — shader output resolution is a parameter |
| No art skill investment | No — requires custom shader development and 3D models authored to look good through the shader |
| Parallax and depth | Yes — real 3D |

**Tools:** Blender (modeling), custom Bevy shader (rendering). Few off-the-shelf solutions exist.

**Key problem:** The shader work is substantial custom rendering engineering. The models need to be authored with the shader in mind. This is the highest-effort approach.

### D: Voxel Models

3D models built from voxels (3D pixels). Essentially pixel art extended into three dimensions.

| Goal | Status |
|------|--------|
| Zelda-like aesthetic | No — reads as a voxel game (Crossy Road, Minecraft-adjacent) |
| Camera flexibility | Yes — voxel models are 3D |
| Resolution independence | Yes — rendered as 3D geometry |
| No art skill investment | Yes — MagicaVoxel is free and approachable for non-artists; voxel art is more forgiving than polygon modeling or hand-drawn art |
| Parallax and depth | Yes — real 3D |

**Tools:** MagicaVoxel (authoring, free), bevy_vox_scene or similar (runtime).

**Key problem:** The aesthetic is distinctive and doesn't look like Zelda. It commits you to a specific visual identity.

### E: 2D Skeletal Animation (Spine, DragonBones)

A character is a hierarchy of 2D art pieces (torso, arms, legs, head) rigged to bones and animated by transforming the bones.

| Goal | Status |
|------|--------|
| Zelda-like aesthetic | No — reads as paper-cutout animation (Hollow Knight, Dead Cells) |
| Camera flexibility | No — art pieces are drawn for one viewing angle |
| Resolution independence | Yes — if source art is high-res, scales smoothly |
| No art skill investment | Partial — fewer free assets than sprite sheets; requires learning rigging |
| Parallax and depth | Partial — depth via layer ordering, not real 3D |

**Tools:** Spine ($70+, proprietary), DragonBones (free), bevy_spine or similar (runtime).

## Summary

| Approach | Zelda | Camera | Resolution | No Art Skill | Parallax |
|----------|-------|--------|------------|-------------|----------|
| A: Sprite sheets | Yes | No | No | Yes | Partial |
| B: 3D models | No | Yes | Yes | Partial | Yes |
| C: HD-2D shader | Partial | Yes | Yes | No | Yes |
| D: Voxel models | No | Yes | Yes | Yes | Yes |
| E: Skeletal 2D | No | No | Yes | Partial | Partial |

No approach satisfies all five goals. The tensions between Zelda aesthetic, camera flexibility, and resolution independence prevent a single solution from covering everything. Prototyping is needed to evaluate which tradeoffs are acceptable in practice.
