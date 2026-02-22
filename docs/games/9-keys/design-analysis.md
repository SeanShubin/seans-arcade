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

## Limited Visibility and Argus's Eye

Starting with 1x1 visibility transforms the maze from a spatial puzzle into an exploration-under-uncertainty game. The player can't see what's coming — every move is a decision made with incomplete information. This makes the maze feel larger and more threatening than it actually is, which works in favor of the small maze size preference.

Argus's Eye upgrades are placed at phase transitions (after keys 3, 6, and at the end). This means visibility expands exactly when the maze opens up:

- **1x1 during Phase 1:** The linear section is navigated blind. This reinforces the tutorial — there's only one path, so limited vision doesn't create decision paralysis, but it does teach the player to pay attention to what they've seen.
- **3x3 during Phase 2:** When branching begins and real decisions matter, the player can now see adjacent spaces. Route planning becomes possible rather than pure guessing.
- **5x5 during Phase 3:** Backtracking through previously visited areas is less tedious when you can see more of the surrounding maze. The wider view helps the player identify where they need to go.
- **7x7 after winning:** A reward for completion. The post-win maze feels like a different experience with wide visibility.

The activation level toggle adds a subtle competitive layer. Skilled players who've memorized the maze layout might turn the eye off to reduce visual noise. In multiplayer, keeping your eye off means other players can't infer your location from your behavior — you're navigating confidently through spaces they'd expect you to stumble through.

Making Argus's Eye personal-only (not visible to other players) is the right call. If expanded vision were visible to others, it would reveal player positions and undermine the fog-of-war tension that limited visibility creates.

## Portals

Portals are the game's only cooperative mechanic, and they're carefully constrained to prevent abuse while enabling meaningful collaboration:

- **Explored-space requirement on the creator:** The player who spawns a portal must have explored both endpoints. But any player can *use* any portal (subject to the key requirement). This means a more experienced player who has explored both locations can open a portal for a less experienced player to travel to a space they've never visited — enabling a "catch up" mechanic where veterans help newcomers skip distance they've already covered.
- **Key requirement for traversal:** Using a portal requires the same keys as walking there. This preserves the key progression — portals are shortcuts through known territory, not skeleton keys that bypass gates.
- **Persistence tied to presence:** At least one of the two related players must stay at an endpoint. This creates a cost — helping someone else means one player is standing still while their timer runs. Cooperation isn't free.
- **One pair per player:** Prevents portal networks that would trivialize maze navigation.

The design tension is interesting: portals are most useful when one player is ahead and another is behind. The ahead player can open a portal back to the behind player's location (having already passed through it), letting the behind player jump forward. But the key requirement means the behind player can only use portals to skip *distance*, not *progression* — they still need the right keys. A veteran who's already completed the maze and has all keys can offer the most valuable portals, but they've already won — their motivation is purely cooperative.

### Cooperative Speed-Running: Anchoring and Parallel Search

The most powerful cooperative strategy emerges from the portal rules without being explicitly designed:

1. Players arrive at a gate nobody has the key for.
2. One player anchors at the gate.
3. The remaining players fan out searching for the key.
4. The first player to find the key picks it up, then creates a portal back to the anchor (the finder has explored both endpoints — the gate and the key location).
5. The anchor traverses the portal to the key location, picks up the key.
6. Both regroup at the gate and pass through.

This is **parallel search** — multiplayer headcount converts directly into search speed. With N players, N-1 searchers cover ground simultaneously. The first to find the key reunites the group via portal. Search time divides by roughly N-1.

This means coordinated groups solve new mazes faster than any solo player could, and the advantage scales with group size. This isn't a post-win trick — it accelerates the first solve.

A secondary cooperative advantage is the **post-win taxi**: a player who has already completed the maze has no running timer, all keys, and full maze exploration. They can go to any key location and open a portal for a new player, effectively teleporting them from key to key. This dramatically accelerates subsequent players' runs at zero cost to the helper.

Portals also interact with the "watching other players" dynamic. Without portals, seeing another player is passive information. With portals, seeing another player is an invitation to cooperate. This shifts the multiplayer model from pure ghost racing to something with an optional cooperative layer.

## Multiplayer

Seeing other players creates psychological pressure and implicit information (if someone is heading a direction you haven't explored, that's a hint). Portals add an optional cooperative dimension without undermining independent play.

One nuance: "no exploration sharing" is true at the system level, but at the human level, watching other players is implicit exploration sharing. A player who follows someone else gets free pathfinding. This means the first player to enter a maze has a slight information disadvantage. Whether this is a feature (drafting as a valid strategy, like in racing) or a concern depends on how competitive the game is intended to be. Limited visibility mitigates this — you can only follow a player you can actually see, which at 1x1 means they'd need to be in your exact space.

## Visible Seed

Without visible seeds, timer scores are meaningless — you'd be comparing times across different mazes. The seed makes competition concrete: "beat my time on seed X." This is essential infrastructure for the timer-based scoring model.

## Triple-Redundant Key Identity (Shape + Color + Name)

Three visual identification channels mean colorblind players can use shape and name, players who don't read closely can use color and shape, and so on. This is inclusive design that doesn't compromise aesthetics.

## Shortcut Gates: Decoupling Solo and Cooperative Reward

Each key unlocks a shortcut gate near itself that provides a quick return path to familiar territory. This is the design's cleanest solution to the solo-vs-cooperative tension:

- **Solo experience:** The outbound trip to find a key involves searching through dead ends and branches — this is the challenge. But once the key is found, the key itself opens a shortcut home. The player is never punished with a long retrace of their search path. Finding the key feels like a reward, and the shortcut confirms it.
- **Cooperative experience:** Multiple players fan out and search in parallel. The first to find the key portals back to the group. The shortcut gate is irrelevant to them — they never needed the return trip.

This decoupling means the generator can make keys genuinely hard to *find* (deep in branching sub-paths with dead ends) without making the solo experience tedious. The cooperative advantage comes purely from search parallelization, not from avoiding return-trip punishment. Solo players are rewarded with satisfying shortcuts; cooperative players are rewarded with speed. Neither is punished.

## Small Maze Size Preference

This directly mitigates the two main design risks:

1. **Backtracking tedium.** Phase 3 asks players to retread old ground. In a small maze, backtracks are short enough to feel strategic rather than like padding.
2. **Linear start slog.** On repeated plays, the forced-order first 3 keys could feel slow in a large maze. Small mazes keep this phase brief.

## Item Pickup as a Choice

Making item pickup voluntary rather than automatic adds a layer of intentionality. Every item in the maze — Ariadne's Thread, Argus's Eye upgrades — represents a decision: is this worth my time? For speedrunners, skipping optional items is a meaningful optimization. For new players, picking everything up is the safe play. The game never forces the player's hand, which keeps agency with the player at every step.

## Design Philosophy Summary

The design reflects a clear philosophy: respect the player's time, reward spatial thinking, and let skill express itself through efficiency rather than survival. The constraints build a progression from guided to free to demanding, with optional tools, visibility upgrades, and an optional cooperative layer on top. Limited visibility creates tension and uncertainty; Argus's Eye relieves that tension at the pace of progression; portals add a social dimension without compromising independent play. Every mechanic is either mandatory (keys and gates) or voluntary (Thread, Eye, portals), giving players control over their own difficulty curve.
