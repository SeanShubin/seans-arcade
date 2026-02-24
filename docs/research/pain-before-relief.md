# Pain Before Relief

Elaborates on [design-philosophy.md](design-philosophy.md) principle #4: Pain Before Relief.

The player must feel a problem viscerally before receiving the solution. The solution's value is proportional to how much the problem hurt.

## The Cycle (Factorio as Primary Example)

The core cycle — build, outgrow, feel the pain, unlock the solution, experience relief — is summarized in [design-philosophy.md](design-philosophy.md) principle #4. Factorio is the defining example.

Factorio could give you the solution earlier. The tech tree is deliberately paced so that you always feel the limitation before you unlock the fix. If logistics robots were available from the start, they'd be just another mechanic. Because you've spent hours manually managing logistics first, they feel like a miracle.

## Why It Works Psychologically

**Contrast effect.** Humans evaluate relative to recent experience, not in absolute terms. A warm room feels luxurious after being outside in the cold. A faster belt feels transformative after watching a slow one bottleneck your factory.

**Effort justification.** When you've struggled with a problem, the solution feels more valuable because your struggle "paid for" it. This is the fairness contract at work.

**Competence validation.** The moment you deploy the solution, you feel mastery. You understand exactly why it's good because you understand exactly what was bad. You don't need a tutorial for logistics robots because you've been doing logistics by hand.

## Other Games That Do This

**Dark Souls** - bonfires and shortcuts. Fight through a brutal area with no checkpoint for 15 minutes. Then open a shortcut back to the last bonfire. The shortcut is just a door. After 15 minutes of dread, that door is the best thing in the game.

**Zelda: Breath of the Wild** - cooking. Early game, you're scrounging for food and eating raw ingredients that barely heal. You discover cooking and suddenly raw apples become hearty meals. The system was always there, but you had to feel hunger first.

**Minecraft** - material tiers. You spend time mining with stone tools, feeling how slow they are, watching iron ore taunt you. When you get iron tools, the speed increase is visceral because you have a body memory of how slow stone was.

**Hollow Knight** - the map system. You enter a new area with no map. You wander, get lost, feel vulnerable. You find the cartographer, buy the map, and the area makes sense. If the map were given at the entrance, it would be a convenience. Because you wandered lost first, it's a relief.

**Celeste** - movement abilities. Early chapters constrain you to basic jumps and dashes. You feel the limitation when you see platforms you can't reach. Later chapters give you additional dashes, and suddenly those impossible-looking gaps are achievable.

**Terraria** - hardmode transition. You've mastered normal mode, you feel powerful, then hardmode resets your competence. Everything is dangerous again. Then you start finding hardmode ores and the cycle begins again at a higher tier.

## The Anti-Pattern: Solution Before Pain

The anti-pattern is summarized in [design-philosophy.md](design-philosophy.md) principle #4. One nuance worth expanding: difficulty settings at the start are a version of this anti-pattern (the player hasn't experienced the game yet, so they don't know what "Normal" means), with one exception — difficulty framed as emotional posture toward uncertainty ("how cautious are you?") works because it describes the player's tolerance for not-yet-knowing, which is the one thing they DO understand at that point. See [discovered-contract.md](discovered-contract.md).

## The Pattern Formalized

The five-step cycle and timing guidance are in [design-philosophy.md](design-philosophy.md) principle #4. The key nuance for implementation: the player's own success creates the pressure (step 2). The bottleneck comes from the player building something that works and then outgrowing it — the frustration is personal because they built the thing that's now failing. This is why the pain feels fair rather than imposed.
