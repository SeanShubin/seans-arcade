# Maze Key/Gate Design

Research notes on procedural maze generation with colored keys and gates.

## Core Concept

A maze is partitioned into **regions** separated by **gates**. Each gate requires a specific **key** (color-coded) to open. The player collects keys to unlock gates and eventually reach a goal. The dependency structure between keys determines the puzzle's complexity and replayability.

The maze wraps in all directions (torus topology). This has significant implications for region boundaries — see [Wrapping Topology](#wrapping-topology).

## Configuration Knobs

### 1. Number of Keys

How many distinct keys exist in the maze.

- **Low (2-3)**: Easy to hold the full puzzle in your head. Good for tutorial or casual play. The current `maze.rs` example uses 2.
- **Medium (4-6)**: Requires planning. The player can't always remember which gates they've seen for which colors.
- **High (7-9)**: Complex enough that the player needs to explore systematically. Risk of feeling tedious if the maze is too large or backtracking is excessive.

More keys means more regions, more gates, and a longer critical path. The maze must be large enough to give each key meaningful territory — too many keys in a small maze feels cramped.

**Representation**: Integer, 1-9.

### 2. Gates per Key

How many gates share the same color/key. Once you have a key, all gates of that color open.

- **1 gate per key**: Each key unlocks exactly one chokepoint. Linear feel — each key solves one obstacle. Simple to reason about.
- **2-3 gates per key**: Getting a key triggers a burst of exploration. Multiple new areas open at once. The player has choices about which newly-opened area to explore first. This is where non-linearity emerges naturally.
- **Many gates per key**: The key feels powerful and rewarding. But it can make the dependency graph shallower — if one key opens 5 areas, those 5 areas are all at the same dependency depth.

Trade-off: more gates per key = more player choice at each step, but less sequential depth. Fewer gates per key = more sequential depth, but more linear feel.

On a wrapping map, this knob interacts with topology — a single gate can always be walked around on a torus. See [Wrapping Topology](#wrapping-topology).

**Representation**: Not directly configured. The generator determines how many gates each key needs based on the region layout and wrapping topology. On a torus, sealing off a region requires enough gates to form a complete barrier — the generator places as many as needed. The player-facing effect (burst exploration vs linear) emerges from the dependency graph shape, not from a gate count setting.

### 3. Dependency Depth

The longest chain of key dependencies: "need K1 to reach K2, need K2 to reach K3, ..." The depth is the length of the longest such chain.

- **Depth 1**: All keys are reachable from the start. Only the goal is gated. Trivial puzzle — just explore and collect.
- **Depth 2-3**: Some keys are behind gates. Moderate puzzle. The player must plan a route but won't get deeply stuck.
- **Depth equal to key count**: Fully linear — each key unlocks exactly the next key. Maximum sequential challenge but zero player choice in ordering.

Depth controls the minimum number of "unlock a gate, explore, find a key" cycles the player must complete. It's the puzzle's mandatory length.

**Representation**: Not directly configured. Depth is a property that emerges from the dependency graph shape. The graph shape is the real input — see [Graph Shape](#graph-shape). Depth can be used as a constraint to validate that a generated graph meets design intent ("reject graphs with depth less than 3").

### 4. Dependency Width

How many keys are simultaneously obtainable at any point in the puzzle. Width is the maximum number of keys reachable (but not yet collected) at any game state.

- **Width 1**: Linear. Only one key is ever available. The player has no choice about what to do next.
- **Width 2-3**: The player chooses which branch to explore first. Creates replayability — different players may solve in different orders.
- **Width equal to key count**: All keys are reachable from the start. No dependency structure at all.

Width and depth are inversely related for a fixed number of keys. A 9-key puzzle with depth 9 has width 1 (linear). A 9-key puzzle with depth 1 has width 9 (flat). The interesting designs live in between.

**Representation**: Not directly configured. Like depth, width emerges from the graph shape. Can be used as a validation constraint ("width must be at least 2 at every dependency level").

### 5. Backtracking Requirement

Whether the player must revisit previously-explored areas after obtaining a new key.

- **No backtracking (forward-only)**: Every gate is encountered right before the area it guards. The player never needs to remember where a locked gate was. Simpler navigation, but limits the dependency graph — gates can only appear on the forward path.
- **Local backtracking**: Gates are near (but not immediately before) the key that opens them. The player backtracks a short distance. Moderate complexity.
- **Global backtracking (Metroidvania-style)**: Gates may be far from where the key is found. The player must remember gate locations across the entire map and revisit distant areas. Deep exploration, but risks frustration without a map or gate tracker.

This is one of the most impactful knobs for player experience. Forward-only feels like a corridor. Global backtracking feels like an open world. The choice depends heavily on maze size — global backtracking in a small maze is fine, but in a large maze it needs UI support (map, gate markers).

On a wrapping map, backtracking is inherently cheaper — the player is never more than half the map away from anything. This makes global backtracking more viable at smaller map sizes than it would be on a bounded map.

**Representation**: Enum — `ForwardOnly | Local | Global`. This acts as a placement constraint for the generator. `ForwardOnly` means every gate must be on the path between the key and the region it guards. `Local` means the gate must be within N cells of the key. `Global` means no distance constraint.

### 6. Multi-Key Gates

Whether a gate can require more than one key to open.

- **Single-key gates**: Each gate requires exactly one key. Simple, clear feedback. The standard model.
- **Multi-key gates**: A gate requires 2+ specific keys. Creates convergence points — the player must complete multiple branches before proceeding. Forces a "gather then unlock" pattern.
- **Any-of gates**: A gate opens with any one of several keys. Creates redundancy — multiple solutions to the same obstacle. More forgiving but harder to balance.

Multi-key gates add a combinatorial element. They're powerful for creating "boss door" moments (need 3 of 9 keys to reach the final area) but complicate generation — the generator must ensure all required keys are reachable before the gate.

**Representation**: Enum — `SingleKey | MultiKey(max) | AnyOf(max)`. `SingleKey` means every gate takes exactly one key. `MultiKey(max)` allows gates requiring up to `max` specific keys. `AnyOf(max)` allows gates that accept any one of up to `max` keys. Could also allow both multi-key and any-of in the same maze, but that's likely overcomplicating things for the player.

### 7. Key-to-Gate Distance

How far (in maze traversal) a key is from the gate(s) it opens.

- **Adjacent**: Key is right next to its gate. Trivial — no puzzle, just a speedbump. Can work for teaching mechanics.
- **Same region**: Key is in the same area as its gate but requires exploration to find. Moderate challenge.
- **Different region**: Key is behind a different gate entirely. Requires the player to explore one branch to unlock progress in another. Creates interesting cross-cutting dependencies.

Longer distance = harder to connect cause and effect. The player finds a key and must remember (or re-explore to find) where the matching gate was. Short distance = obvious and satisfying but less puzzle depth.

**Representation**: Enum — `Adjacent | SameRegion | AnyReachable`. This constrains where the generator can place keys relative to their gates. `Adjacent` means within a few cells. `SameRegion` means the key and at least one of its gates share a region. `AnyReachable` means no constraint beyond reachability — the key can be anywhere the player can reach before needing it.

### 8. Dead Ends and Red Herrings

Paths that lead nowhere — no key, no gate, no goal.

- **None**: Every path leads to something useful. Efficient but predictable — the player knows any path they take will pay off.
- **Few**: Some wrong turns. Adds mild exploration challenge without frustration.
- **Many**: The maze is mostly dead ends with keys hidden among them. Classic maze difficulty — finding the right path is the challenge, not the key/gate logic.

Dead ends interact with backtracking. In a forward-only maze, dead ends just cost time. In a backtracking maze, dead ends waste time AND make it harder to remember where the real gates were.

On a wrapping map, dead ends only arise from internal walls — there are no map-edge dead ends. The generator must intentionally create them.

**Representation**: Float, 0.0-1.0 — the fraction of total corridor space that belongs to dead-end branches. 0.0 means a "perfect" maze with no wasted paths. 1.0 would be mostly dead ends (impractical but defines the scale). The generator builds the critical path and key placements first, then fills remaining space with dead-end branches until the target ratio is met.

### 9. Maze Size per Region

How much maze exists between gates — the exploration surface area within each "key zone."

- **Small (1-2 rooms)**: Fast progression. Get key, open gate, repeat. The puzzle is the dependency logic, not navigation.
- **Medium (5-10 rooms)**: Each region is a mini-maze. Finding the key within a region is a sub-challenge.
- **Large (many rooms)**: Each region is a substantial exploration. Risk of losing the thread of the larger key/gate puzzle while navigating within a single region.

This knob controls pacing. Small regions make the key/gate puzzle feel fast and puzzle-like. Large regions make it feel like an adventure game where keys are rare milestones.

**Representation**: Integer — cells per region (target, not exact). The generator allocates this many maze cells to each region when partitioning space. Total maze size is roughly `cells_per_region * (num_keys + 1)` (one region per key plus the start region). Could also be expressed as total maze size with even distribution, but per-region is more intuitive for tuning pacing.

### 10. Key Consumption

Whether keys are consumed when used or persist.

- **Consumed (use-once)**: Opening a gate uses up the key. If there are 3 red gates and 1 red key, the player must choose which gate to open. Adds a strategic layer but risks soft-locks if the player opens the "wrong" gate.
- **Persistent (use-forever)**: Once collected, a key opens all matching gates permanently. The current model in `maze.rs`. No risk of soft-lock. Simpler to generate valid puzzles.
- **Limited copies**: Multiple copies of the same key exist, each consumed on use. Middle ground — the player has some but not unlimited uses.

Consumed keys add strategic depth but make generation much harder. The generator must ensure no game state leads to a soft-lock (stuck with no way to reach the goal). Persistent keys are dramatically easier to validate.

**Representation**: Enum — `Persistent | Consumed | LimitedCopies(n)`. `Persistent` is the safe default. `Consumed` requires the generator to verify no soft-locks exist across all reachable game states — exponentially harder validation. `LimitedCopies(n)` places `n` copies of each key; the generator must verify that `n` copies suffice for all required gates on every valid path.

### Graph Shape

Dependency depth and width are not configured directly — they emerge from the shape of the dependency graph. The graph shape is the real design input.

Possible representations:

- **Enum of named shapes**: `Linear | Branching(width) | Diamond | Hub`. Each shape implies a depth/width profile. Simple to configure, but limited expressiveness.
  - `Linear`: K1 → K2 → K3 → ... (depth = N, width = 1)
  - `Branching(w)`: Tree with max `w` children per node. Player chooses which branch to explore.
  - `Diamond`: Branches that reconverge — parallel paths that merge at multi-key gates.
  - `Hub`: Central area connects to independent branches. Low depth, high width.

- **Constraint-based**: Specify `min_depth`, `max_depth`, `min_width`, `max_width` and let the generator find a valid graph. More flexible, but the generator must search for solutions.

- **Hand-authored**: Provide the DAG explicitly as a list of edges. Maximum control, no generation needed for the dependency structure. Appropriate when the puzzle design matters more than replayability.

For procedural generation with replayability, the enum approach is probably the right starting point — pick a shape, randomize within that shape.

## Interactions Between Knobs

Some knobs amplify or conflict with each other:

| Combination | Effect |
|-------------|--------|
| High depth + no backtracking | Forces a very long linear corridor — each region feeds into the next |
| High width + small regions | Lots of short parallel branches — feels like a hub with spokes |
| Multi-key gates + consumed keys | Extremely hard to generate without soft-locks |
| Many dead ends + global backtracking | Frustrating without a map — the player can't distinguish dead ends from locked-gate paths they should remember |
| High gates-per-key + low depth | Burst-exploration pattern — long stretches of free exploration punctuated by key finds |
| Large regions + many keys | Very long play time — each key takes significant effort to find |

## Wrapping Topology

The maze wraps in all directions (torus topology). This affects region boundaries and gate placement significantly.

### The Bypass Problem

On a bounded map, a single gate in a corridor creates a hard boundary — the player cannot get past without the key. On a torus, any single gate can be bypassed by walking the other way around the map. A wall running north-south across the map doesn't create two sides — the player can walk off the east edge and appear on the west side, past the wall.

This means **region boundaries must form closed loops** on the torus. A line of walls with a gate in it only works as a boundary if the line wraps all the way around the map and back to itself, forming a complete ring with no gaps (except gated ones).

### Implications for Generation

- **Minimum gates per region boundary**: To seal off a region on a torus, the boundary must be a closed curve. If the boundary has N gaps (corridors passing through it), all N must be gated with the same key. A simple east-west wall spanning the full map width with 2 corridor openings needs 2 gates of the same color.
- **Region shapes**: On a bounded map, regions can be any connected shape touching the border. On a torus, regions must be bounded by closed curves. Rectangular regions work well — a rectangle's boundary is a closed loop.
- **Nested regions**: Regions can be nested — a small region inside a larger one. The inner region's boundary is a closed loop inside the outer region. This naturally creates depth in the dependency graph (must enter outer region before inner).
- **Concentric rings**: A natural pattern on a torus is concentric rectangular rings. Each ring is gated by a different key. The innermost ring contains the goal. Depth equals the number of rings.
- **Validation**: After placing all walls and gates, the generator must verify that removing only the gates for key K (and all previously obtainable keys) does not connect the key-K region to the outside. Flood fill from the start with the current key set; verify that gated regions are not reachable without their keys.

### Advantages of Wrapping

- **No edge effects**: Every cell has 4 neighbors. No corners or borders that create artificial dead ends or safe zones. The maze is uniform everywhere.
- **Shorter backtracking**: Maximum distance between any two points is half the map in each dimension. Global backtracking is less punishing.
- **Disorientation**: Without edges, the player can't use map boundaries for orientation. This makes the maze feel larger and more mysterious. Can be mitigated with landmarks or a minimap.
- **Seamless regions**: Region boundaries don't interact with map edges. Every boundary is purely internal walls and gates.

## Generation Strategy (Overview)

1. **Design the dependency DAG** — choose depth, width, branching pattern
2. **Assign keys to DAG nodes** — each node is a key, edges are "must have before"
3. **Partition maze space into regions** — one region per DAG node plus the start region. On a torus, each region boundary must be a closed loop
4. **Place gates at region boundaries** — enough gates per boundary to seal all corridor openings. All gates on one boundary share the same key color
5. **Generate maze geometry per region** — standard maze algorithms within each region
6. **Place keys in valid regions** — a key must be in a region reachable without that key
7. **Add dead ends** — fill remaining space with non-essential paths up to the target dead-end ratio
8. **Validate** — flood-fill reachability check: for each key set, verify only the correct regions are accessible. On a torus, this catches any bypass paths that go "around the back"
