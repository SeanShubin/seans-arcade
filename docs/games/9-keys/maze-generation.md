# 9 Keys — Maze Generation

Analysis of the theory and approach for generating mazes that satisfy the logical design constraints.

## Relevant Theory

**Lock-and-key puzzles in procedural generation** is the applicable research area. The key insight from the literature is separating two concerns:

1. **Mission graph** — the abstract dependency structure (key 1 leads to key 2 leads to key 3, then 4-6 in any order, etc.)
2. **Space graph** — the physical maze topology that embeds the mission graph

The logical design already defines the mission graph. The generation problem is embedding it into a grid maze.

Notable work in this area:
- Dormans & Bakkes (2011) — "Generating Missions and Spaces for Adaptable Play Experiences" — formalizes mission/space graph separation using graph grammars, using Zelda dungeons as the primary case study
- Mark Brown's "Boss Keys" series — design analysis of Zelda dungeon structure, documenting the linear-then-branching-then-backtracking progression pattern that 9 Keys follows

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
3. Branch from the main path for keys 4-6, each behind a gate requiring key 3. Add sub-branches and dead ends within each branch so keys require searching, not just walking.
4. For keys 7-9, place them behind gates on paths the player already traversed (this is the backtracking constraint). Spread these gates apart rather than clustering them.
5. For each key, place a shortcut gate near the key that the key itself opens, providing a short return path to familiar territory (a junction, main path, or the key's corresponding progression gate).
6. Fill remaining space with maze structure (dead ends, loops, extra corridors).
7. Place Ariadne's Thread on an early-reachable space (accessible before any keys).
8. Place the first Argus's Eye upgrade between key 3 and keys 4-6.
9. Place the second Argus's Eye upgrade between key 6 and keys 7-9.
10. Place the third Argus's Eye upgrade in the final space.

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
| Argus's Eye placement | Easy — constrained to phase transitions and final space |
| Cooperative tuning (branch depth, key hiding, gate spread) | Moderate — balancing solo fairness with cooperative reward |
| Small but unpredictable mazes | Moderate — tuning problem |

## Cooperative Play Incentives

The maze generator should reward cooperative play without punishing solo players. The principle: solo should feel fair and complete; cooperation should feel noticeably faster. The maze should never feel tedious alone — it should feel *satisfying* alone and *exciting* together.

### Design Levers

**Branching with depth, not length.** Phase 2 branches should contain sub-branches and dead ends so that finding each key is a search problem, not a walk-to-the-end problem. This rewards parallel search (multiple players cover sub-branches simultaneously) without punishing solo players — a solo player still finds the key through methodical exploration, it just takes longer. The key is that dead ends should be short enough that a solo player doesn't feel cheated, but numerous enough that a group covers ground meaningfully faster.

**Key hiding over key distance.** Place keys off the main path of their branch — behind a turn, down a spur, past a fork. This makes keys harder to *find* rather than harder to *reach*. Finding is parallelizable (more searchers = faster); reaching is not. A solo player who explores methodically will find every key; a group that fans out will find it sooner.

**Spread backtracking gates apart.** In Phase 3, place the backtracking gates far from each other rather than clustered near a central junction. This creates more ground to cover when searching for the right gate to revisit, which rewards the anchoring strategy — one player holds position while others search different areas of the maze. For a solo player, this just means more walking between backtrack targets, which is acceptable if individual distances are reasonable.

**Keys unlock their own shortcuts.** When a player finds a key, a gate near that key (opened by that same key) should provide a shortcut back to familiar territory — a junction, the main path, or the gate the key was needed for. This eliminates backtracking tedium for solo players: the outbound trip is a search through dead ends and branches, but the return trip is a quick shortcut through a newly opened gate. This is the critical design lever that decouples solo fairness from cooperative reward. Solo players are rewarded with a satisfying "aha, a shortcut home" moment. Cooperative players skip the search entirely via parallel coverage and portals — the shortcut is irrelevant to them because they never needed the return trip in the first place.

### What to Avoid

**Don't create mazes that feel broken solo.** Every lever above must be tuned so that the solo experience remains satisfying. If a solo player feels like the maze was designed for groups, the design has failed. The solo player should feel clever for finding the key; the group should feel clever for finding it faster.

**Don't make cooperation mandatory.** No part of the maze should require multiple players. Every key, gate, and item must be reachable by a single player. Cooperation is a speed optimization, never a requirement.

**Don't make return trips punishing.** Every key should have a shortcut return path. The solo experience should never require retracing a long outbound search path. If a solo player feels punished for not having a portal partner, the maze has failed.

### Tuning Guideline

The cooperative advantage should come entirely from **parallel search** — finding keys faster by covering more ground simultaneously. It should not come from avoiding return trips, because the shortcut gates already eliminate return trip tedium for solo players. This cleanly separates the two experiences: solo players are rewarded with shortcut gates, cooperative players are rewarded with search parallelization. Neither is punished.

The target is roughly: a coordinated pair should complete the maze 20-30% faster than a skilled solo player, and a coordinated group of 3-4 should see diminishing but real returns beyond that. This should emerge from the topology rather than being engineered precisely — the generation parameters (branch depth, key hiding, gate spread) are tuning knobs, not formulas.

## Overall Assessment

This is a constrained procedural generation problem with known solutions. The theory exists and the constraints are well-structured. The hardest part is the backtracking requirement for keys 7-9, which requires the generator to reason about player state at different points in the traversal. A construction-based approach (build the solution topology first, then fill in the maze) is strongly preferred over generate-and-validate.
