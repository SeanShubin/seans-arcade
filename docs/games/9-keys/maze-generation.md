# 9 Keys — Maze Generation

Analysis of the theory and approach for generating mazes that satisfy the logical design constraints.

## Relevant Theory

**Lock-and-key puzzles in procedural generation** is the applicable research area. The key insight from the literature is separating two concerns:

1. **Mission graph** — the abstract dependency structure (key 1 leads to key 2 leads to key 3, then 4-6 in any order, etc.)
2. **Space graph** — the physical maze topology that embeds the mission graph

The logical design already defines the mission graph. The generation problem is embedding it into a grid maze.

Notable work in this area:
- Dormans & Bakkes (2011) — "Generating Missions and Spaces for Adaptable Play Experiences" — formalizes mission/space graph separation using graph grammars, using Zelda dungeons as the primary case study
- Mark Brown's "Boss Keys" series — design analysis of Zelda dungeon structure, documenting linear-then-branching-then-convergence progression patterns similar to 9 Keys

## Dependency Structure

The key acquisition order forms a partial order (a DAG):

- Keys 1-3 form a chain (1 before 2 before 3)
- Keys 4-6 are independent of each other but depend on key 3
- Keys 7-9 depend on earlier keys via convergence paths (requiring multiple earlier keys to reach)
- The final key depends on all other 8 keys

## Recommended Approach: Construction

Build the dependency skeleton first, then fill in the remaining space with maze structure. Rejection sampling — generating random mazes and discarding those that violate constraints — would waste enormous time given the number of constraints.

### Construction Steps

1. Place the start and end.
2. Carve the linear path to keys 1-3, placing gates between them.
3. Branch from the main path for keys 4-6, each behind a gate requiring key 3. Add sub-branches and dead ends within each branch so keys require searching, not just walking.
4. For keys 7-9, place them behind convergence paths — routes requiring passage through two or more gates needing different earlier keys. Spread these paths across the torus rather than clustering them.
5. For each key, place a shortcut gate near the key that the key itself opens, providing a short return path to familiar territory (a junction, main path, or the key's corresponding progression gate).
6. Classify gates: progression gates (those enforcing key dependency order) are hard — ensure their walls form complete topological cuts with no wrap-around bypass. All other gates are soft.
7. Fill remaining space with maze structure (dead ends, loops, extra corridors).
8. Place Ariadne's Thread on an early-reachable space (accessible before any keys).
9. Place the Ariadne's Thread upgrade mid-Phase 2, reachable after some but not all of keys 4-6.
10. Place the first Argus's Eye upgrade between key 3 and keys 4-6.
11. Place the second Argus's Eye upgrade between key 6 and keys 7-9.
12. Place the third Argus's Eye upgrade in the final space.

### Reachability Reasoning

The generator must simulate reachability at each stage of key acquisition. Specifically:

- For hard gates, verify that the walls form a complete topological cut across the torus — flood-fill from one side without the key must not reach the other side via any wrap-around path.
- For convergence paths (keys 7-9), verify that no subset of the required keys is sufficient to reach the key — all required gates on the path must be necessary.
- For soft gates, verify that the bypass route (wrapping around) is meaningfully longer than the gated short path, so finding the key provides a real advantage.

## Difficulty Breakdown

| Aspect                                                     | Difficulty                                                                     |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Basic maze generation on a grid                            | Easy — well-known algorithms (recursive backtracking, Kruskal's, Prim's, etc.) |
| Linear key chain (keys 1-3)                                | Easy — gated corridor                                                          |
| Branching keys (4-6)                                       | Moderate — multi-branch placement                                              |
| Convergence keys (7-9)                                     | Moderate — multi-gate path placement and reachability verification             |
| Hard gate verification                                     | Moderate — must confirm complete topological cuts on the torus                 |
| Toroidal topology                                          | Moderate — wrap-around adjacency throughout, coordinate arithmetic             |
| "Always solvable" guarantee                                | Easy if built by construction                                                  |
| Ariadne's Thread placement                                 | Easy — any early-reachable space                                               |
| Argus's Eye placement                                      | Easy — constrained to phase transitions and final space                        |
| Cooperative tuning (branch depth, key hiding, gate spread) | Moderate — balancing solo fairness with cooperative reward                     |
| Small but unpredictable mazes                              | Moderate — tuning problem                                                      |

## Cooperative Play Incentives

The maze generator should reward cooperative play without punishing solo players. The principle: solo should feel fair and complete; cooperation should feel noticeably faster. The maze should never feel tedious alone — it should feel *satisfying* alone and *exciting* together.

### Design Levers

**Branching with depth, not length.** Phase 2 branches should contain sub-branches and dead ends so that finding each key is a search problem, not a walk-to-the-end problem. This rewards parallel search (multiple players cover sub-branches simultaneously) without punishing solo players — a solo player still finds the key through methodical exploration, it just takes longer. The key is that dead ends should be short enough that a solo player doesn't feel cheated, but numerous enough that a group covers ground meaningfully faster.

**Key hiding.** Place keys off the main path of their branch — behind a turn, down a spur, past a fork. This makes keys harder to *find* rather than harder to *reach*. Finding is parallelizable (more searchers = faster); reaching is not. A solo player who explores methodically will find every key; a group that fans out will find it sooner.

**Spread convergence paths across the torus.** In Phase 3, place the convergence paths far from each other rather than clustered. The toroidal topology gives the generator more placement freedom — paths can wrap around the torus to create spatial separation even in a small grid. This rewards the anchoring strategy — one player holds position while others search different areas. For a solo player, individual distances remain reasonable because wrapping provides shorter routes than a flat grid would.

**Keys unlock their own shortcuts.** When a player finds a key, a gate near that key (opened by that same key) should provide a shortcut back to familiar territory — a junction, the main path, or the gate the key was needed for. This eliminates backtracking tedium for solo players: the outbound trip is a search through dead ends and branches, but the return trip is a quick shortcut through a newly opened gate. This is the critical design lever that decouples solo fairness from cooperative reward. Solo players are rewarded with a satisfying "aha, a shortcut home" moment. Cooperative players skip the search entirely via parallel coverage and portals — the shortcut is irrelevant to them because they never needed the return trip in the first place.

On a toroidal maze, shortcuts are especially rewarding. A shortcut gate can connect two regions that are an entire wrap-around apart, collapsing large distances into a few steps. The player who finds a key and unlocks a shortcut may suddenly have fast access to a distant part of the torus. This also interacts with soft gates: a player who bypassed a soft gate via wrapping still benefits from later finding the key, because the key unlocks shortcuts that make future traversals faster.

### What to Avoid

**Don't create mazes that feel broken solo.** Every lever above must be tuned so that the solo experience remains satisfying. If a solo player feels like the maze was designed for groups, the design has failed. The solo player should feel clever for finding the key; the group should feel clever for finding it faster.

**Don't make cooperation mandatory.** No part of the maze should require multiple players. Every key, gate, and item must be reachable by a single player. Cooperation is a speed optimization, never a requirement.

**Don't make return trips punishing.** Every key should have a shortcut return path. The solo experience should never require retracing a long outbound search path. If a solo player feels punished for not having a portal partner, the maze has failed.

### Tuning Guideline

The cooperative advantage should come entirely from **parallel search** — finding keys faster by covering more ground simultaneously. It should not come from avoiding return trips, because the shortcut gates already eliminate return trip tedium for solo players. This cleanly separates the two experiences: solo players are rewarded with shortcut gates, cooperative players are rewarded with search parallelization. Neither is punished.

The target is roughly: a coordinated pair should complete the maze 20-30% faster than a skilled solo player, and a coordinated group of 3-4 should see diminishing but real returns beyond that. This should emerge from the topology rather than being engineered precisely — the generation parameters (branch depth, key hiding, gate spread) are tuning knobs, not formulas.

## Overall Assessment

This is a constrained procedural generation problem with known solutions. The theory exists and the constraints are well-structured. The main challenges are verifying hard gates form complete topological cuts on the torus and ensuring convergence paths for keys 7-9 genuinely require multiple earlier keys.
