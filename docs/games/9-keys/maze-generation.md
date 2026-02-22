# 9 Keys — Maze Generation

Analysis of the theory and approach for generating mazes that satisfy the logical design constraints.

## Relevant Theory

**Lock-and-key puzzles in procedural generation** is the applicable research area. The key insight from the literature is separating two concerns:

1. **Mission graph** — the abstract dependency structure (key 1 leads to key 2 leads to key 3, then 4-6 in any order, etc.)
2. **Space graph** — the physical maze topology that embeds the mission graph

The logical design already defines the mission graph. The generation problem is embedding it into a grid maze.

Notable work in this area:
- Dormans & Bakkes (2011) — "Generating Missions and Spaces for Adaptable Play Experiences" — formalizes mission/space graph separation using graph grammars
- Zelda dungeon generation literature — 9 Keys is structurally similar to classic Zelda dungeon design (linear progression, branching, backtracking)

## Dependency Structure

The key acquisition order forms a partial order (a DAG):

- Keys 1-3 form a chain (1 before 2 before 3)
- Keys 4-6 are independent of each other but depend on key 3
- Keys 7-9 depend on earlier keys and require topological backtracking
- The final key depends on all other 8 keys

## Recommended Approach: Construction Over Rejection

Rather than generating a random maze and hoping it satisfies constraints (rejection sampling), build the dependency skeleton first and then fill in the maze. Rejection sampling would waste enormous time given the number of constraints.

### Construction Steps

1. Place the start and end.
2. Carve the linear path to keys 1-3, placing gates between them.
3. Branch from the main path for keys 4-6, each behind a gate requiring key 3.
4. For keys 7-9, place them behind gates on paths the player already traversed (this is the backtracking constraint).
5. Fill remaining space with maze structure (dead ends, loops, extra corridors).
6. Place Ariadne's Thread on an early-reachable space (accessible before any keys).

### Reachability Reasoning

The generator must simulate reachability at each stage of key acquisition. Specifically, for the backtracking requirement (keys 7-9), the generator needs to:

- Place a gate on a path the player must have already walked past.
- Put the key behind that gate.
- Ensure the player couldn't have had that key when they first passed the gate.

This means tracking what the player has and hasn't acquired at each point in the traversal.

## Difficulty Breakdown

| Aspect | Difficulty |
|---|---|
| Basic maze generation on a grid | Easy — well-known algorithms (recursive backtracking, Kruskal's, Prim's, etc.) |
| Linear key chain (keys 1-3) | Easy — gated corridor |
| Branching keys (4-6) | Moderate — multi-branch placement |
| Backtracking keys (7-9) | Hard — requires reachability reasoning |
| "Always solvable" guarantee | Easy if built by construction |
| Ariadne's Thread placement | Easy — any early-reachable space |
| Small but unpredictable mazes | Moderate — tuning problem |

## Overall Assessment

This is a constrained procedural generation problem with known solutions. The theory exists and the constraints are well-structured. The hardest part is the backtracking requirement for keys 7-9, which requires the generator to reason about player state at different points in the traversal. A construction-based approach (build the solution topology first, then fill in the maze) is strongly preferred over generate-and-validate.
