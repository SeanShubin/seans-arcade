# The Discovered Contract

Elaborates on [design-philosophy.md](design-philosophy.md) principle #3: Discovery Feels Like the Player's Achievement, and principle #6: The Fairness Contract.

The game's rules are themselves a reward to be earned through play. The fairness contract isn't fixed from the start — it expands as the player discovers more of the system. Early actions are exploratory gambles with bounded downside. Late actions are informed decisions with full reasoning.

## The Pattern (Vantage as Primary Example)

Vantage is a board game where:
- You don't know most victory conditions or rules until you commit to an action that reveals them
- Actions always succeed, but you don't know the cost before taking the action — and the cost could cause you to lose
- You explore the world using visual cues to estimate risk
- You start precarious but become vastly more powerful over time
- You choose difficulty at the start: cautious (easy) to daring (hard), which controls how many costly uninformed decisions you can survive

Each of these mechanics reinforces the others. Together they form a pattern where the player's understanding of the game IS the primary progression system.

## What Makes This Different From Hidden Information

Many games have hidden information — fog of war, unrevealed map tiles, unknown enemy stats. The discovered contract goes deeper: the **rules of the game** are hidden. Not just "what's behind this door" but "what doors are, what opening costs, and why you'd want to."

In a typical game, the player learns the rules during a tutorial, then applies them. In a discovered contract game, learning the rules IS the game. Every action is simultaneously gameplay and tutorial, because the action teaches you something that changes how you evaluate future actions.

## Why Actions Must Always Succeed

This is the design choice that makes hidden costs tolerable. Consider the alternatives:

| Actions can fail | Actions always succeed |
|-----------------|----------------------|
| Hidden cost + failure = feels capricious | Hidden cost + success = feels exploratory |
| "I gambled, I lost, I learned nothing" | "I paid a price, I got something, I learned the price" |
| Player becomes risk-averse (stops exploring) | Player stays curious (every action teaches) |
| Fairness contract feels broken | Fairness contract feels different but valid |

If actions could fail AND have hidden costs, the game punishes exploration. Because actions always succeed, every action has three outputs: the success (always), the cost (discovered), and the rule (learned). The player always comes away with more than they started, even if the cost was high — because the knowledge of the cost is itself a permanent gain.

## The Evolving Fairness Contract

The standard fairness contract (see [design philosophy #6](design-philosophy.md)) says: "The player must be able to reason about outcomes before committing." The discovered contract deliberately violates this early on, but does so fairly:

| Phase | What you know | What you risk | Fairness mechanism |
|-------|--------------|---------------|-------------------|
| Early game | Almost nothing | Unknown costs | Difficulty setting = error budget; visual cues = partial reasoning; every action teaches |
| Mid game | Some rules, some costs | Calculated risks | Growing knowledge enables better decisions; discovered rules are consistent |
| Late game | Most rules, most costs | Informed decisions | Full reasoning is now possible; knowledge replaces the difficulty buffer |

The contract tightens as the stakes rise. Early, when the player can't reason fully, the game doesn't ask for precise decisions — it asks for exploration. Late, when a bad decision could be catastrophic, the player has learned enough to reason effectively. The game is hardest to reason about when reasoning matters least, and easiest to reason about when reasoning matters most.

## Difficulty as Error Budget

Traditional difficulty settings ask "how hard do you want the game to be?" — which requires understanding the game. Vantage's difficulty asks "how much do you want not knowing the rules to cost you?" — which requires understanding only one thing: that you don't know the rules. See also [progression-and-difficulty-design.md](progression-and-difficulty-design.md) — Difficulty as Error Budget, which places this concept in the broader spectrum of difficulty approaches.

"Cautious" and "daring" describe the player's relationship to uncertainty, not the game's challenge level:

- **Cautious:** wide margin. Multiple costly discoveries can be absorbed. The knowledge axis has a long runway to develop.
- **Daring:** narrow margin. Every discovery matters immediately. Visual cues must be read accurately from the start.

What difficulty does NOT change:
- The rules themselves
- The costs themselves
- The visual cues
- The victory conditions
- The late-game experience

What difficulty changes:
- How many uninformed mistakes you can survive before you've learned enough to make informed ones

By late game, a cautious player and a daring player are in similar positions — both know the rules, both have power. The difficulty setting mattered most when knowledge was lowest. As knowledge replaces ignorance, it replaces the difficulty buffer as your protection against bad outcomes. The training wheels become irrelevant because you learned to ride.

This is not the anti-pattern of "difficulty settings at the start" (which the [pain-before-relief](pain-before-relief.md) analysis flags). That anti-pattern is about asking players to choose a challenge level before understanding what challenge means in this game. Vantage's framing communicates emotional posture ("how cautious are you?"), not mechanical tuning ("how much damage should enemies do?"). It works without game knowledge because it's describing the player's tolerance for the universal experience of not-yet-knowing.

## Knowledge of Rules vs Knowledge of World

The [emergent gameplay](emergent-gameplay-and-progression.md) doc defines knowledge as one of three progression axes: secret passages, enemy weaknesses, hidden interactions. The discovered contract pushes this one level deeper. The player isn't learning "fire beats ice" within a known ruleset — they're learning that fire exists. The rules themselves are the content.

This creates a distinct form of progression:

| Knowledge type | Example | Effect on gameplay |
|---------------|---------|-------------------|
| Knowledge of world | "There's a secret passage behind the waterfall" | Player can now access that passage |
| Knowledge of mechanics | "Fire beats ice" | Player can now exploit that interaction |
| Knowledge of rules | "Actions have costs, and visual cues predict cost character" | Player can now evaluate ALL future actions differently |

Knowledge of rules is the most powerful because it's transferable. Learning one specific cost teaches you something about ALL costs. Learning one visual cue teaches you how to read ALL visual cues. Each discovery has compound returns because it calibrates your judgment, not just your map.

## Trust Building Through Visual Cue Calibration

The [meaningful choice analysis](meaningful-choice-analysis.md) identifies trust as cumulative — early choices that matter build trust that future choices matter. The discovered contract builds trust through a different mechanism: visual cue reliability.

Each time the player:
1. Reads a visual cue ("this looks dangerous")
2. Takes the action anyway
3. Discovers the cost matched the cue's character

...they trust the visual language more. By late game, the player is making confident decisions based on visual reads — not because the game taught them the rules, but because they've calibrated their own ability to read the game. This is a powerful form of earned mastery. The player doesn't feel taught; they feel perceptive.

The BG3 analysis describes "predictable in tone, surprising in specifics" — the player can predict the direction of a choice without foreseeing every detail. The discovered contract takes this further: visual cues let the player predict the *character* of a cost without knowing the specifics or even the category. Less information than BG3, but still enough to support reasoning and build trust.

## The Pattern Formalized

1. **Hide the rules, not arbitrarily, but as content to discover.** The rules are consistent — once discovered, they always apply. The hiding is the game's way of making discovery the reward.
2. **Actions always succeed.** The player always gets something. Hidden costs are tolerable because the action's success and the cost's lesson are both real gains.
3. **Visual cues provide partial reasoning.** The player can estimate risk character without knowing specifics. This preserves agency — actions are informed gambles, not blind guesses.
4. **Difficulty controls the error budget, not the challenge.** The player chooses how much the discovery phase can hurt, not how hard the game is once they've learned.
5. **Knowledge of rules replaces the difficulty buffer.** As the player learns, their understanding provides the protection that the difficulty setting provided early on. The game becomes self-balancing.
6. **Start precarious, end powerful.** The power arc tracks the knowledge arc. Early vulnerability is real because ignorance is real. Late power is earned because understanding is earned.

## Connection to Other Principles

| Principle | How the discovered contract relates |
|-----------|-------------------------------------|
| [Pain before relief](pain-before-relief.md) | Early ignorance IS the pain. Rule discovery IS the relief. Each rule learned transforms the game the way a Factorio upgrade transforms the factory. |
| [Discovery as achievement](design-philosophy.md) | Taken to its deepest form — the player doesn't just discover secrets within the rules, they discover the rules themselves. |
| [Fairness contract](design-philosophy.md) | The contract evolves rather than being fixed. Fairness is maintained by matching information to stakes, not by providing full information upfront. |
| [Meaningful choice](meaningful-choice-analysis.md) | Choices are irrevocable and consequential from the start. The twist: the player evaluates fairness retrospectively as they learn what their choices actually cost. |
| [Multiple progression axes](emergent-gameplay-and-progression.md) | Knowledge becomes the PRIMARY axis for early game, not just one of three parallel options. Power and skill become relevant once the player knows enough to use them strategically. |
| [Difficulty spectrum](progression-and-difficulty-design.md) | Adds a new category: difficulty as error budget during discovery, not as challenge scaling. |
