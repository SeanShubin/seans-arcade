# 9 Keys — Design Analysis

Analysis of the game design reasoning behind the constraints in the logical design.

## Three-Phase Key Structure (3-3-3)

The key progression creates a scaffolded difficulty curve across three phases:

**Phase 1 — Linear (keys 1-3):** Functions as an implicit tutorial. The player can't make wrong decisions — there's only one path forward. They learn the core loop (explore, find key, open gate) with zero cognitive overhead about which direction to go. Every maze is different due to random generation, but every player gets onboarded the same way.

**Phase 2 — Branching (keys 4-6):** Training wheels off. The player makes real decisions — which branch first? This phase teaches route planning and spatial awareness in a forgiving context, since any order works and no choice is wrong.

**Phase 3 — Backtracking (keys 7-9):** The mastery test. The player must re-evaluate spaces they already dismissed, remember where locked gates were, and plan efficient routes back. This rewards spatial memory and punishes mindless wandering.

The phase sizes being equal (3-3-3) gives each phase proportional weight. 9 total is the smallest number that divides into three meaningful phases — fewer keys would make phases too brief, more would risk the game dragging.

## Starting Next to the Final Gate

This is the "show the goal before the journey" technique. The player sees the end from the very first moment. Every key they collect feels like measurable progress toward something concrete they've already seen. This creates forward motivation throughout the entire game.

## No Loss Condition with Timer Scoring

This is "easy to finish, hard to master." Anyone can complete the maze — the game respects the player's time and never punishes exploration. But the timer creates competitive depth for skilled players. This is the right model for a maze game specifically — being stuck in a maze with a death timer would be frustrating rather than fun. The timer adds pressure without adding punishment.

## Ariadne's Thread as Optional Item

This is a self-selecting difficulty mechanism. The Thread helps less confident players navigate, while skilled players can skip it. There's a subtler layer: if the Thread isn't on the optimal path, picking it up costs time. This creates a risk/reward tradeoff:

- New player: "I'll grab the Thread, the navigation help is worth the detour."
- Experienced player: "I'll skip it, I can read the maze faster without the detour."

This produces natural skill expression without explicit difficulty settings.

## Multiplayer with No Interaction (Ghost Racing)

Seeing other players creates psychological pressure and implicit information (if someone is heading a direction you haven't explored, that's a hint) without enabling griefing. It's a clean competitive model.

One nuance: "no exploration sharing" is true at the system level, but at the human level, watching other players is implicit exploration sharing. A player who follows someone else gets free pathfinding. This means the first player to enter a maze has a slight information disadvantage. Whether this is a feature (drafting as a valid strategy, like in racing) or a concern depends on how competitive the game is intended to be.

## Visible Seed

Without visible seeds, timer scores are meaningless — you'd be comparing times across different mazes. The seed makes competition concrete: "beat my time on seed X." This is essential infrastructure for the timer-based scoring model.

## Triple-Redundant Key Identity (Shape + Color + Name)

Three visual identification channels mean colorblind players can use shape and name, players who don't read closely can use color and shape, and so on. This is inclusive design that doesn't compromise aesthetics.

## Small Maze Size Preference

This directly mitigates the two main design risks:

1. **Backtracking tedium.** Phase 3 asks players to retread old ground. In a small maze, backtracks are short enough to feel strategic rather than like padding.
2. **Linear start slog.** On repeated plays, the forced-order first 3 keys could feel slow in a large maze. Small mazes keep this phase brief.

## Design Philosophy Summary

The design reflects a clear philosophy: respect the player's time, reward spatial thinking, and let skill express itself through efficiency rather than survival. The constraints build a progression from guided to free to demanding, with optional tools and competitive structure layered on top.
