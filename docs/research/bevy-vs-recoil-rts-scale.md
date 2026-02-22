# Bevy vs Recoil: RTS at Scale

A comparison of Bevy's ECS architecture against the Recoil engine (used by Beyond All Reason) for large-scale RTS games.

## The Recoil Engine

Beyond All Reason runs on the **Recoil Engine**, a hard fork of the **Spring RTS engine** (originally called "TA Spring" — built to recreate Total Annihilation in 3D). The fork happened because Spring's leadership was pushing toward OpenGL 4 while dropping OpenGL 3 support in a way the BAR developers disagreed with, and more fundamentally because Spring had architectural limitations incompatible with the scale BAR needed.

### Spring's Limitations (Why the Fork Was Necessary)

- **Single-threaded simulation** — originally designed with no threading, and "nearly impossible to execute multithreading without causing desyncs" due to lockstep networking requiring bit-identical simulation across all clients.
- **Pathfinding bottleneck** — the single biggest performance problem at scale. Moving 1,000+ units caused significant FPS drops even on flat, open maps.
- **Legacy rendering** — loaded unit models individually, applying transformations piece-by-piece, resulting in thousands of small draw calls and GPU stalling.
- **Hard unit cap of 32,000** with practical performance limits far below that.
- **32-bit default on Windows** limiting memory addressability.

### What Recoil Fixed

**Multithreaded pathfinding:** Work is distributed by request — each thread handles a complete pathfinding request. All requests within a frame are queued and dispatched at once. Uses HAPFS (Hybrid A* Pathfinding System). Result: **4-5x performance improvement** over single-threaded pathfinding.

**GPU-instanced OpenGL 4 renderer:** Models are uploaded to GPU buffers during game load. Units are grouped by model type and submitted via a single render call per faction. Result: **2-10x FPS improvement** (2x in light gameplay, 3-8x in heavy end-game, up to 10x in stress tests).

**Combined improvement: 3-10x overall performance** over Spring.

### Recoil's Architecture

- **Deterministic lockstep networking** — only player inputs are transmitted, not game state. Each client runs an identical simulation. Extremely bandwidth-efficient, enabling 100+ player matches.
- **Simulation tick rate** — approximately 30 simulation frames per second. Rendering runs independently as fast as hardware allows.
- **Staggered SlowUpdate** — expensive per-unit operations distributed across multiple frames to prevent CPU spikes.
- **Memory pooling** — custom allocators for units, features (wrecks/debris), and projectiles to minimize allocation overhead.
- **QuadField spatial partitioning** — used for collision detection, unit/feature location searches, and area-of-effect calculations.
- **Staged unit deletion** — units deleted in stages to prevent crashes from dangling references.
- **Lua scripting layer** — majority of game-specific code written in Lua, with the C++ engine providing the simulation backbone.
- **Threading model** — separate threads for rendering, loading, sound, networking, and a watchdog thread. Pathfinding is multithreaded. Core simulation remains primarily single-threaded for determinism.

### Proven Scale

- Regular competitive matches with thousands of units.
- Community mega-events demonstrated 110+ players in a single match, 10,000-15,000+ units simultaneously.
- Theoretical cap of 160 players tested during a community celebration event.
- The 32,000 unit hard cap remains from Spring, but practical limits depend on hardware.

## Bevy's ECS Architecture

### Theoretical Advantages

**Archetype-based storage:** Entities sharing the same component set are grouped in contiguous memory. Iteration over components like `(Position, Velocity)` is cache-friendly — data is packed in dense arrays. This is the single biggest architectural win for large entity counts.

**Automatic parallel system scheduling:** The scheduler inspects each system's function signature to determine data access. Systems that don't conflict (no overlapping mutable access) run in parallel automatically. A movement system, combat system, and resource system can all run simultaneously if they touch different components.

**Internal parallelism (par_iter):** Within a single system, entity iteration can be split across threads. Automatic fallback to single-threaded for small entity counts.

**Dual storage strategies:** Table storage (default, fast iteration) and Sparse Set storage (fast add/remove) allow tuning per-component. Frequently-toggled markers use Sparse Set; stable data uses Table.

**GPU-driven rendering (Bevy 0.16):** Moved most rendering decisions to the GPU. Roughly 3x improvement over Bevy 0.15 in heavy scenes.

### Limitations for Large-Scale RTS

**Determinism (the critical problem):** Bevy has five identified sources of non-determinism:

1. Floating-point inconsistencies across platforms (x87 vs SSE, different rounding modes)
2. System execution order is non-deterministic by default when systems have no explicit ordering
3. Entity iteration order within queries is not guaranteed stable
4. PRNG across threads/systems can diverge
5. Command/EventReader evaluation order is tied to system topology

For lockstep networking, bit-for-bit determinism is required. The Bevy team acknowledges this requires "a concerted, organized effort in both Bevy and its dependencies" and it remains an open problem.

**No built-in pathfinding:** Community plugins exist (flowfield-based approaches, which are the correct model for RTS), but none are battle-tested at thousands of units.

**Physics at scale:** Frame rate drops around ~1,000 rigidbodies with standard physics plugins. A large-scale RTS would need custom lightweight collision (spatial hashing, simplified shapes) rather than full physics simulation.

**API instability:** Breaking changes approximately every 3 months. A multi-year RTS project faces continuous migration cost.

**No proven RTS at scale:** The most ambitious Bevy RTS attempt (Digital Extinction) was archived in February 2026 as "embarrassingly bare bones." No large-scale RTS has shipped in Bevy.

## Comparison

| Factor | Recoil | Bevy |
|---|---|---|
| Simulation parallelism | Core loop single-threaded (for determinism), pathfinding/rendering threaded | Automatic multi-system parallelism, per-entity parallelism within systems |
| Cache performance | Traditional C++ with manual optimization | Archetype storage provides inherently cache-friendly iteration |
| Deterministic networking | Battle-tested lockstep at 100+ players | Five unsolved sources of non-determinism |
| Pathfinding | Mature multithreaded HAPFS, proven at 10K+ units | Community flowfield plugins, untested at scale |
| Rendering | GPU-instanced OpenGL 4, 2-10x over legacy | GPU-driven rendering (0.16), 3x over previous |
| Proven scale | 10,000-15,000 units, 110+ players, shipped | No large-scale RTS shipped |
| RTS-specific features | SlowUpdate staggering, spatial partitioning, terrain deformation, ballistic simulation, 20+ years of optimization | General-purpose; all RTS features must be built or sourced from community |
| Extensibility | Lua scripting over C++ engine | Rust with full ECS composability |
| Stability | Mature, stable API | Breaking changes every ~3 months |

## Assessment

Bevy's ECS architecture is theoretically superior for parallelizing game simulation. Archetype storage and automatic system parallelism are real advantages that Recoil's primarily single-threaded simulation core cannot match. If Bevy's determinism problems were solved, it could potentially spread RTS simulation across cores in ways Recoil cannot.

But theoretical superiority means nothing against 20 years of battle-tested RTS-specific engineering. Recoil is a purpose-built engine with solved problems — deterministic lockstep, massive-scale pathfinding, proven 10K+ unit counts in production. Bevy is a general-purpose engine where the RTS-critical infrastructure (determinism, pathfinding, large-scale collision) is either unsolved or immature.

Recoil's fundamental limitation is that its core simulation is single-threaded — it can only scale vertically on one core. Bevy's fundamental limitation is that its parallelism breaks determinism — it can scale horizontally but can't guarantee identical results. Both are hard problems. Recoil chose to accept the single-threaded constraint and optimize within it. Bevy hasn't yet been forced to make that choice for RTS workloads.

Could Bevy eventually be better for large-scale RTS? Possibly — if someone solves deterministic ECS scheduling, builds production-quality RTS pathfinding, and proves it at scale. That's a multi-year research-and-engineering effort, not a game development project.
