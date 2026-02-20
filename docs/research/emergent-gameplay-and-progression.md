# Emergent Gameplay and Satisfying Progression

Elaborates on [design-philosophy.md](design-philosophy.md) principles #1: The Player Is In Control, #5: Systems Have Rules Not Scripts, and #6: The Fairness Contract.

Maximizing player agency through multiple progression systems that allow the player to have whatever experience they choose, while always feeling they got what they deserved.

## The Core Principle: Effort Must Equal Reward, But Effort Takes Many Forms

The player should always feel "I earned this." But "earning" can mean:
- I figured out the right order (intelligence)
- I practiced until I could do it (skill)
- I invested time to get stronger (persistence)
- I explored and found something hidden (curiosity)
- I combined two things nobody told me to combine (creativity)

The design goal: all of these are valid currencies, and they're all exchangeable. A player who is rich in skill but poor in patience should progress. A player who is rich in patience but poor in skill should also progress. Neither should feel the other cheated.

## Multiple Progression Axes

Design at least three independent axes of player power:

### 1. Character Power (Grindable)

Stats, equipment, levels, upgrades. The player gets stronger by repeating content. This is the safety net - anyone can progress given enough time.

- Health upgrades - more mistakes allowed
- Damage upgrades - fights end faster
- Defensive items - specific hazards become trivial
- Consumables - temporary power spikes

This axis respects the player's time. "I spent two hours farming, I deserve to be stronger." They're right.

### 2. Player Skill (Practiced)

The player's actual ability: dodging, timing, pattern recognition, resource management. This cannot be taken away and doesn't appear in any inventory.

- A skilled player with starting equipment can beat content designed for max-level characters
- An unskilled player with max equipment can brute-force content designed for skilled players
- The game never tells the player which path they're on - both feel like "playing the game correctly"

This axis respects mastery. "I died 40 times and learned the pattern." They earned it differently but equally.

### 3. Knowledge (Discovered)

What the player knows about the world: secret passages, enemy weaknesses, optimal routes, hidden interactions, NPC hints. This is the axis that rewards exploration and curiosity.

- A player who explores thoroughly finds the fire weapon before the ice dungeon
- A player who talks to every NPC learns the boss's weakness
- A player who experiments discovers that combining two items creates something powerful
- A player on their second playthrough is "overpowered" by knowledge alone

This axis respects attention. "I noticed the cracked wall, I read the old man's hint, I tried using the lantern on the dark room." They were paying attention, and the game rewarded it.

**Deeper form: knowledge of rules.** Most games assume the player knows the rules and discovers secrets within them (hidden passages, enemy weaknesses). Some designs hide the rules themselves — the player discovers how the game works through play. Knowledge of rules is the most powerful form because it's transferable: learning one rule calibrates your judgment for ALL future decisions, not just the specific situation. When rules are the discovery content, the knowledge axis dominates early game and the other axes become relevant once the player knows enough to use them strategically. See [discovered-contract.md](discovered-contract.md).

## How The Axes Interact

The magic happens when the axes substitute for each other:

| Situation | Power Solution | Skill Solution | Knowledge Solution |
|-----------|---------------|----------------|-------------------|
| Hard boss | Grind until you out-stat it | Learn the pattern, beat it at base level | Find the weakness item hidden in the world |
| Locked area | Find the key through main progression | Sequence break with precise movement | Discover the hidden back entrance |
| Resource scarcity | Farm enemies for drops | Play flawlessly, need fewer healing items | Find the hidden stash the NPCs hinted at |
| Long dangerous path | Level up to survive the gauntlet | Navigate it without getting hit | Find the shortcut/warp |

Every cell in this table should be a valid way to play. The player picks their column unconsciously based on their personality, and the game respects all three equally.

## Designing For Unexpected Discovery

The key principle: the player should find things they weren't looking for that solve problems they haven't encountered yet.

### Environmental Foreshadowing

Place rewards before the player knows they need them:
- A room has a conspicuous item behind a simple puzzle. The player grabs it. Three hours later, they hit a boss that's nearly impossible - unless you have that item, which makes it trivial. The player feels like a genius for exploring earlier.
- The critical detail: the player must not know the item is important when they find it. If the game says "you'll need this later," it's a checklist. If the player discovers the connection themselves, it's an insight.

### Interconnected Systems

When systems interact in ways the game doesn't explicitly teach:
- Fire weapon melts ice blocks (obvious after a moment's thought)
- But fire weapon also lights dark rooms (less obvious)
- And fire weapon scares certain enemies who flee instead of fight (surprising)
- And fire weapon can be used on the blacksmith's forge to upgrade equipment (emergent)

Each discovery layer rewards deeper experimentation. The player who only uses the fire weapon for combat has a fine experience. The player who tries it on everything discovers a richer game.

### Hidden Depth In Simple Mechanics

- A dash ability's primary purpose is crossing gaps
- But it also has invincibility frames during the dash
- A skilled player realizes they can dash through attacks
- An exploratory player realizes they can dash through thin walls
- Each discovery transforms a simple mechanic into something that feels like a secret the player personally uncovered

### The "Wait, What?" Moment

Design specific interactions that surprise even attentive players:
- An enemy that's been harassing you for hours turns out to be tameable if you approach it without weapons equipped
- A piece of environmental decoration turns out to be interactive if you have the right item
- Returning to the starting area late in the game reveals it's changed, with new secrets accessible with late-game abilities

## The Fairness Contract

For all of this to feel earned rather than arbitrary, there's an implicit contract:

### The Player Must Be Able To Reason About It

- If fire beats ice, the player should suspect it before confirming it
- If a wall is breakable, there should be a visual tell (even subtle)
- If an NPC gives a hint, it should be actionable, not just flavor text
- No "how was I supposed to know that?" moments - every discovery should feel like "I should have seen that sooner"

### Power Must Be Proportional To Investment

- A hard-to-reach secret should contain a powerful reward
- An easy-to-find item should be modestly useful
- A long grind should produce meaningful improvement
- A trivial task should produce trivial reward
- The player's internal accounting must balance. If they did something hard and got something weak, trust is broken.

### Multiple Solutions Must Not Invalidate Each Other

- The player who grinded for 2 hours should not feel stupid when they learn there was a shortcut
- The player who found the shortcut should not feel they missed out on the grinding rewards
- Frame it as: "I chose my path and it worked" not "I chose wrong"
- Solution: the grind path and the clever path should give different rewards that are laterally equivalent, not the same reward with different effort

### Difficulty Must Be Readable

- The player should be able to look at a challenge and estimate its difficulty before committing
- Visual language: enemy size, color intensity, environmental hostility
- This lets the player make informed decisions about which axis to lean on
- "That looks too hard for me right now, I'll come back stronger" is a valid and satisfying decision

## Implementation Architecture

In Bevy terms, this translates to:

```
Progression Systems (Resources/Components):
├── CharacterStats      # health, damage, defense - modified by equipment
├── Equipment           # items that modify stats
├── Inventory           # keys, consumables, special items
├── PlayerKnowledge     # flags for discovered secrets, NPC conversations
├── WorldState          # what's been changed, opened, defeated
└── SkillExpression     # (implicit - lives in the player's hands, not the code)

Cross-Cutting Systems:
├── DifficultyResolver  # reads all axes, determines actual encounter difficulty
├── RewardScaler        # adjusts rewards based on player state
├── HintSystem          # surfaces contextual hints based on PlayerKnowledge gaps
└── WorldReactivity     # changes world based on player actions and state
```

The `DifficultyResolver` is the critical system. It doesn't set a "difficulty level" - it reads the player's current state across all axes and determines what the player is likely capable of. Not to scale the game to match (that removes agency), but to ensure the game's challenges are distributed so that every combination of axes has a viable path forward.

## The Design Test

For every piece of content, ask:

1. Can a high-power, low-skill player complete this? (grinding path)
2. Can a low-power, high-skill player complete this? (mastery path)
3. Can a medium-power, medium-skill player with knowledge complete this? (explorer path)
4. Does each path feel like the player earned the outcome?
5. Does any path feel like it invalidates the others?

If all five answers are satisfactory, the content respects player agency.
