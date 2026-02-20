# Procedural vs Authored Level Design

Can the excellent design of Zelda and Metroidvanias be fully automated with a well-tuned algorithm?

## What Can Be Automated

**Room connectivity and key-lock graphs.** The structure of "you need item X to reach area Y" is a directed graph. Algorithms can generate valid key-lock orderings - ensure the player can always reach the next key before the lock that needs it, prevent softlocks, guarantee completability. This is a solved problem. Roguelikes like Dead Cells and Spelunky do this procedurally every run.

**Room geometry.** Wave Function Collapse, Binary Space Partitioning, and other procedural generation algorithms can produce tilemap layouts that are structurally valid - connected corridors, rooms of varying sizes, no unreachable areas. No Man's Sky and Minecraft prove this works at massive scale.

**Enemy placement.** Difficulty scaling by distance from start, enemy type selection based on which items the player has at that point in the progression graph, spawn budgets per room. This is constraint satisfaction - algorithmic.

**Item distribution.** Place items so the critical path isn't too long, optional paths reward exploration, and the player has enough resources to survive. Again, constraint satisfaction.

## What Can't Be Automated (Yet)

**Pacing.** The original Zelda's overworld has a rhythm: tense combat room, then a quiet corridor, then a puzzle, then a reward. Super Metroid's Norfair feels oppressive on purpose - the music, the enemy density, the lack of save rooms all work together. An algorithm can vary room types, but the emotional arc of a dungeon - tension, release, anticipation, surprise - requires understanding human psychology in a way current algorithms don't model well.

**Teachable moments.** Zelda introduces each item in a room designed specifically to teach you how to use it. The boomerang room has enemies positioned so the boomerang's path demonstrates its behavior. Hollow Knight's Crystal Heart is introduced in a long horizontal shaft with no floor. These are authored experiences - the designer anticipated the player's mental state and constructed a specific lesson. Procedural generation doesn't know what the player doesn't know yet.

**Spatial storytelling.** In Super Metroid, you glimpse the Wrecked Ship before you can reach it. The map layout itself creates curiosity: "what's over there?" Hollow Knight's map has areas that visually overlap on the map but are separated by progression locks, creating a sense of a world folding in on itself. This requires a designer thinking about the player's mental map over time.

**The "aha" moment.** The best Metroidvanias give you an item and suddenly every dead end you remember becomes a possibility. The double jump doesn't just unlock one door - it recontextualizes the entire map. Algorithmically, this is "item X unlocks doors A, B, C, D." Experientially, it's the player feeling clever for remembering that ledge from two hours ago. The algorithm can create the locks, but the feeling depends on whether the player noticed the lock before they had the key - and that's a question of visual design, camera positioning, and level layout rhythm.

## The Honest Assessment

You could generate a **completable** Metroidvania procedurally. The key-lock graph would be valid, the rooms would be navigable, the difficulty would scale. It would be functional.

It would not be Hollow Knight. The difference is authorial intent - a designer placing a bench (save point) right before a hard boss because they know you'll die five times. A designer making the path to a boss long enough that you dread the walk back but short enough that you don't quit. A designer hiding a secret behind a breakable wall that you'll only notice because the wall texture is slightly different, and the difference is so subtle that finding it feels like your discovery, not the game's gift.

That gap between "functional" and "memorable" is where human design lives. Algorithms are excellent assistants for the structural work, and a hybrid approach (procedurally generate candidate layouts, then hand-tune the best ones) is probably the practical sweet spot for a solo developer.

## What This Means For a Learning Project

Hand-design your levels. It's a small number of rooms, and the design process teaches you how these games think. If you later want to explore procedural generation, that's an excellent experiment on its own - implementing a key-lock graph generator or Wave Function Collapse in Bevy would be a substantial and rewarding project.
