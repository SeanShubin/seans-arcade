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

The three axes of player investment — Persistence, Mastery, and Curiosity — are defined in [design-philosophy.md](design-philosophy.md) principle #1, along with the substitution table showing how each axis can independently solve every challenge. The rest of this section elaborates on the deeper form of the knowledge axis and on designing content that respects all three axes equally.

**Deeper form: knowledge of rules.** Most games assume the player knows the rules and discovers secrets within them (hidden passages, enemy weaknesses). Some designs hide the rules themselves — the player discovers how the game works through play. Knowledge of rules is the most powerful form because it's transferable: learning one rule calibrates your judgment for ALL future decisions, not just the specific situation. When rules are the discovery content, the knowledge axis dominates early game and the other axes become relevant once the player knows enough to use them strategically. See [discovered-contract.md](discovered-contract.md).

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

The fairness contract — proportional rewards, consistent rules, readable difficulty, and parallel solutions that don't invalidate each other — is defined in [design-philosophy.md](design-philosophy.md) principle #6. For how the contract can evolve through discovery, see [discovered-contract.md](discovered-contract.md).

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
