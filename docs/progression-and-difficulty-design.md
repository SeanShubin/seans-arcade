# Progression and Difficulty Design

Player agency over difficulty and progression is a family of related design concepts. They range from designer-controlled linear paths to fully player-authored challenge runs.

## 1. Key-Lock Graphs (Formal Progression Structure)

Established academic theory exists. Key work by **Joris Dormans** ("Adventures in Level Design" and "Engineering Emergence") and **Mark Brown** (Game Maker's Toolkit on YouTube, "Boss Keys" series which maps the lock-and-key structure of every Zelda dungeon).

A key-lock graph is a directed graph where:
- **Nodes** are areas or rooms
- **Edges** are connections between them
- **Locks** are barriers on edges requiring a specific key
- **Keys** are items/abilities that open locks

"Key" and "lock" are abstract. A key can be a literal key, a hookshot, the ability to swim, a boss kill that opens a door, or even knowledge (the player learns a pattern). A lock is anything that gates progress.

**The critical constraint:** the graph must be completable - there must exist an ordering where the player can reach every key before encountering the lock it opens, without requiring an item that's behind the lock that item opens. This is formally a topological sort problem. If the dependency graph has a cycle, the game is broken.

Zelda's dungeons are mostly linear key-lock chains with occasional branches. Metroid is a single massive key-lock graph spanning the entire world with many optional branches.

## 2. Mega Man: Vulnerability Graphs (Soft Gating)

Mega Man doesn't use key-lock graphs. All 8 stages are accessible from the start. Instead, it uses a vulnerability graph - a rock-paper-scissors structure:

- Beat Fire Man, get Fire weapon
- Fire weapon is super effective against Ice Man
- Ice weapon is super effective against Electric Man
- And so on in a cycle

There's no hard gate. You can beat Ice Man with the default weapon. It's just significantly harder. The "intended" path is the one that chains weaknesses, but the game never forces it.

This is **soft gating**: difficulty serves as the gate rather than impossibility. The player can break the intended sequence if they're skilled enough. The graph defines an optimal path, not the only path.

## 3. Player-Directed Difficulty Ordering (Emergent Difficulty)

When the player chooses the order, they're unknowingly choosing their difficulty curve.

Breath of the Wild takes this to its extreme: you can fight Ganon immediately after the tutorial. The world is fully open. Difficulty is determined entirely by where you choose to go and what you choose to acquire first.

The design challenge: the game must be beatable regardless of ordering, which means either:
- Every encounter is tuned for the weakest possible player state (boring if you're powered up)
- Encounters scale to the player (removes the feeling of getting stronger)
- Power-ups make things easier but aren't required (Mega Man's approach - the elegant middle ground)

## 4. Self-Imposed Challenge (Player-Authored Difficulty)

A challenge run is when the player intentionally refuses power-ups to increase difficulty. This is entirely emergent - the designers didn't plan for it, but the game's structure permits it.

This works because of **orthogonal game mechanics**: the sword is for damage, but the player's skill (dodging, positioning, timing) is an independent axis. When you remove the equipment upgrades, the skill axis still provides a path to victory.

Examples:
- Three-heart runs in Zelda (never collect heart containers)
- Nuzlocke runs in Pokemon (faint = permanently dead, catch only first encounter per area)
- Soul Level 1 runs in Dark Souls (never level up)
- Starting equipment only in Zelda (original sword and armor throughout)

The design implication: if your game has multiple axes of player power (equipment, levels, skill, knowledge), players will naturally experiment with removing some axes to create challenge. Games that support this accidentally tend to have devoted communities.

## 5. How These Concepts Relate

They form a spectrum of how much the designer controls the experience vs how much the player controls it:

```
                    PROGRESSION GATING
                    /                \
              Hard Gates          Soft Gates
            (impossible          (difficult
            without item)        without item)
                |                     |
          Key-Lock Graph      Vulnerability Graph
          (Zelda, Metroid)    (Mega Man)
                |                     |
                 \                   /
                  \                 /
              Player Chooses Order
              (when multiple paths exist)
                       |
              Emergent Difficulty
              (order determines challenge)
                       |
              Player Goes Further:
              Self-Imposed Constraints
              (challenge runs)
```

| Designer Control | Player Control | Example |
|-----------------|---------------|---------|
| Total | None | Linear game, one path (Call of Duty campaign) |
| High | Low | Zelda dungeons - gated, but some choice within |
| Medium | Medium | Mega Man - optimal path exists, player can deviate |
| Low | High | Breath of the Wild - almost everything optional |
| None | Total | Challenge runs - player invents their own rules |

## 6. Difficulty as Error Budget (Discovery-Phase Difficulty)

A distinct approach from all of the above: difficulty controls how much the learning process hurts, not how hard the game is once learned.

When a game hides its rules and the player discovers them through play (see [discovered-contract.md](discovered-contract.md)), every early action is an uninformed decision with unknown cost. Difficulty doesn't adjust enemy stats, puzzle complexity, or resource scarcity. It adjusts how many costly uninformed decisions the player can survive before they've learned enough to make informed ones.

- **Cautious (easy):** wide error margin. Multiple expensive discoveries can be absorbed. The player has a long runway to learn.
- **Daring (hard):** narrow error margin. Every discovery matters immediately. Misreading a visual cue could end the game.

This is NOT the anti-pattern of difficulty settings at the start ("the player doesn't know what Normal means"). The player doesn't need to understand the game to answer "how cautious are you?" — the framing describes emotional posture toward uncertainty, not mechanical tuning. It works because the one thing every player knows at the start is that they don't know the rules.

By late game, cautious and daring players converge — both know the rules, both have power. Knowledge replaces the difficulty buffer as the player's protection against bad outcomes. Difficulty mattered most when knowledge was lowest.

| Difficulty type | What it adjusts | When it matters most | Example |
|----------------|----------------|---------------------|---------|
| Challenge scaling | Enemy stats, puzzle complexity | Throughout the game | Most action games |
| Soft gating | Cost of deviating from intended path | When exploring off the critical path | Mega Man |
| Self-imposed constraints | Player removes axes of power | Late game, after mastery | Challenge runs |
| Error budget | Tolerance for uninformed decisions | Early game, during discovery | Vantage |

## For Implementation

In Bevy terms, a key-lock system is:
- A `HashMap<LockId, KeyId>` defining what opens what
- An `Inventory` resource or component tracking acquired keys
- A system that checks `inventory.contains(required_key)` when the player interacts with a locked barrier
- For soft gating: instead of blocking passage, modify enemy stats or spawn harder variants based on what the player has acquired

The graph itself can be data-driven - loaded from a file, not hardcoded - which makes it editable without recompiling.
