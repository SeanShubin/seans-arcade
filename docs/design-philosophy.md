# Game Design Philosophy

These principles guide any game I build. This document is self-contained - every design decision should be evaluable against it without referencing anything else.

---

## 1. The Player Is In Control

Every outcome - success, failure, discovery, struggle - should feel like the result of the player's choices. "I made a mistake" not "that was unfair." "I earned that" not "the game let me win."

**Three axes of player investment** - each is valid, and they're exchangeable:

- **Persistence** (time) - grinding, farming. Rewarded with character power.
- **Mastery** (skill) - practice, pattern recognition. Rewarded with the ability to bypass power requirements.
- **Curiosity** (attention) - exploration, experimentation, NPC hints. Rewarded with knowledge that transforms the game.

No axis invalidates the others. The grinder and the shortcut-finder both earned their progress.

**Evaluation test:** For any challenge - can it be solved through power, skill, or knowledge independently? Does each path feel earned? Does any path make the others feel wrong?

| Situation | Power | Skill | Knowledge |
|-----------|-------|-------|-----------|
| Hard boss | Out-stat it | Learn the pattern | Find the hidden weakness item |
| Locked area | Main progression key | Sequence break | Hidden back entrance |
| Resource scarcity | Farm drops | Play flawlessly | Find the hidden stash |
| Dangerous path | Level up to survive | Navigate without getting hit | Find the shortcut |

---

## 2. The Avatar Does What You Meant

Translate player **intention** into action, not raw **input**. Human inputs are imprecise. The game silently corrects. Responsive, not realistic.

**Implementation priorities:**

1. **Instant visual response** - every input acknowledged on the frame it's pressed
2. **Input buffering** (5-10 frames) - remember presses across state transitions
3. **Coyote time** (5-10 frames) - allow jumping briefly after leaving a ledge
4. **Variable jump height** - hold longer to jump higher
5. **Generous hitboxes** - player hurtbox smaller than sprite, enemy hitbox smaller than sprite
6. **Sound on input frame** - audio confirms input before animation plays
7. **Animation cancel windows** - consistent, learnable rules for when actions can be interrupted
8. **Target magnetism** - infer the intended target from input direction

**Movement:** near-instant acceleration, instant ground direction change, immediate stop. Momentum-based movement only as a deliberate hazard, never as default.

**Camera:** look ahead of movement, smooth follow with slight lag, never obscure the player, never fight the player.

---

## 3. Discovery Feels Like the Player's Achievement

The player should feel clever, observant, or creative - genuinely, not as a manufactured moment.

**Foreshadowing.** Place rewards before the player knows they need them. The connection is discovered by the player, never announced by the game. "You'll need this later" is a checklist. Realizing it yourself is an insight.

**Interconnected systems.** Mechanics interact in ways the game doesn't teach. Fire weapon melts ice (obvious), lights dark rooms (less obvious), scares certain enemies (surprising), works on the blacksmith's forge (emergent). Each layer rewards deeper experimentation.

**Hidden depth.** A dash crosses gaps. It also has invincibility frames. It also passes through thin walls. Each discovery transforms a simple mechanic into a secret the player uncovered.

**The reasoning test.** Target reaction: "I should have seen that sooner." Design failure: "How was I supposed to know that?" Every discoverable thing needs a hint, even a subtle one.

**The deepest form: rules as discovery.** Some games hide not just secrets within the rules, but the rules themselves. The player discovers how the game works through play, and each discovered rule transforms how they evaluate all future actions. When the rules are the content, the knowledge progression axis dominates early game. See [discovered-contract.md](discovered-contract.md).

---

## 4. Pain Before Relief

The player must feel a problem before receiving the solution. The solution's value is proportional to how much the problem hurt.

**The cycle:**
1. The player builds/achieves something that works
2. Their needs outgrow it - the thing that was fine is now a bottleneck
3. They feel the limitation - frustration is real and personal
4. They unlock the solution through natural progression
5. The relief is euphoric because they remember the pain

**Timing is everything.** Too early: a feature the player doesn't appreciate. Too late: the player quits in frustration. The window: long enough to feel the pain, short enough that pain hasn't become resentment.

**The anti-pattern: solution before pain.** Tutorials that teach mechanics before you need them. Skill tree abilities for situations you haven't encountered. The solution feels like nothing when delivered before the pain. It feels like everything when delivered after.

---

## 5. Systems Have Rules, Not Scripts

Build systems that follow consistent rules. Emergent interactions arise as consequences of those rules, not as individually authored features.

**Rules vs scripts:**
- **Scripted:** "If the player uses fire on the ice door, the door melts." One anticipated interaction, one piece of code.
- **Rules-based:** "Fire is hot. Ice melts when heated. Doors can be destroyed. Wood is flammable." The ice door melting is a *consequence* that was never specifically written. So is the wooden bridge burning, the food cooking, and the enemy fleeing from the torch - all from the same property: fire is hot.

**Design methodology:** when adding a system, define its properties and let those properties interact with existing systems. Don't ask "what interactions should I script?" Ask "what are the rules, and what do they imply?"

**Scope constraint:** a small number of systems with real rules produces more emergent interactions than many systems with scripted behaviors. Three deeply consistent systems (fire/ice/wind, or physics/destruction/sound) create a combinatorial space the player can explore. Ten shallow systems with one behavior each create a list the player memorizes.

**Why this matters for player trust:** when the world follows consistent rules, the player learns to reason about it. They predict "fire should melt this" and it does. They try "maybe wind affects projectiles" and it does. Each confirmed prediction builds confidence that experimentation will be rewarded. Inconsistency - fire melts this ice but not that ice - breaks trust the same way the fairness contract does.

**Evaluation test:** When adding a new element to the world, does it have properties that interact with existing systems? Or does it only do one predetermined thing? Can the player predict how it behaves based on rules they've already learned?

---

## 6. The Fairness Contract

The player maintains an internal ledger of effort vs reward. When it balances, they trust the game. When it doesn't, trust breaks and may not recover.

**Proportional rewards.** Hard effort yields powerful rewards. Easy finds yield modest ones. If the player did something hard and got something weak, the contract is broken.

**Consistent rules.** If fire beats ice once, fire always beats ice. If a visual pattern means "breakable wall," it always means breakable wall. The player must be able to reason about outcomes before committing.

**Readable difficulty.** Visual language communicates threat: enemy size, color, environmental hostility. "That looks too hard right now, I'll come back stronger" is a valid and satisfying decision.

**Parallel solutions don't invalidate each other.** The grinder shouldn't feel stupid for not finding the shortcut. The explorer shouldn't feel they missed the grind rewards. Different paths give laterally equivalent rewards, not the same reward at different cost.

**The contract can evolve.** Some designs intentionally start with an incomplete fairness contract — the player can't fully reason about outcomes because the rules are still being discovered. This is fair when: the stakes match the information (low knowledge = bounded downside), visual cues provide partial reasoning, and the contract tightens as the player learns. See [discovered-contract.md](discovered-contract.md).

---

## 7. The World Reacts

The player exists in the world, not executing a script.

- Defeated enemies stay defeated (or their absence is acknowledged)
- Acquired items visibly change what's possible
- NPCs acknowledge what the player has done
- The environment changes in response to progression
- Returning to old areas with new abilities reveals new possibilities

**Two types of gates:**
- **Hard gates** - impossible without a specific item. Create directed progression. Critical constraint: never require an item that's behind the lock it opens.
- **Soft gates** - significantly harder without the intended tool, but possible for skilled players. Respects mastery.

Combine both: hard gates for major milestones, soft gates for everything in between.

---

## 8. Pacing Is Authored

Rhythm: tension and release, challenge and reward, density and breathing room.

- After a hard fight, a quiet space
- After acquiring a new ability, a space designed to enjoy using it
- Before a boss, anticipation. After a boss, the world opens up.
- Glimpsing an unreachable area creates curiosity. Reaching it later pays off the anticipation.

**Teachable moments.** The room after acquiring an ability is designed to teach it. The designer anticipated the player's mental state and constructed a lesson. This cannot be procedurally generated.

---

## 9. Choices Must Be Felt

A choice is meaningful only when the world visibly changes, the decision can't be taken back, and later content remembers what happened.

**Visible consequence.** The player must see the impact. If a choice enters a void - nothing changes, no one reacts - the player learns to stop caring about choices.

**Irrevocability.** Weight comes from permanence. If the player knows they can reload and try the other option, the decision costs nothing.

**The world remembers.** A choice in hour 1 should echo in hour 10. NPCs reference it. Doors are open or closed because of it. The world is different because of what the player decided.

**Predictable in tone, surprising in specifics.** The player should be able to reason about the direction of a choice ("siding with them will probably mean these people suffer") without foreseeing every detail. Unpredictable-but-consistent consequences feel like a living world. Random consequences feel unfair.

**Trust is cumulative.** Early choices that matter build trust that future choices matter. Early choices that don't matter train the player to stop engaging. Once trust in the choice system erodes, even a genuinely meaningful late-game choice won't land.

---

## 10. Respect the Player's Time

- Backtracking reveals something new, not repetition
- Grinding is a choice, never a requirement
- No artificial delays for common actions
- Checkpoints respect real-world time constraints
- Death penalties teach, not punish
- Shortcuts open as areas are mastered

---

## 11. Trust Between Players

Games may have adversarial mechanics (competition, PvP, territory control), but the architecture assumes all players are acting in good faith. Distrust between players is a game mechanic, not a technical reality.

**No cost for distrust.** The game should never spend processing power, bandwidth, or architectural complexity on the assumption that players are trying to cheat or corrupt each other's experience. Server-authoritative architectures that exist primarily to prevent cheating are paying a permanent tax for a problem that doesn't exist in a trusted group.

**Prefer P2P deterministic simulation.** When all players are trusted, every client can run the full simulation independently. Only inputs need to travel over the network. This minimizes bandwidth and eliminates the need for a powerful central server. A lightweight coordinator orders inputs; every client computes the world state from those inputs.

**Determinism is an engineering discipline, not a trust mechanism.** Ensuring all clients arrive at the same game state is a correctness problem, not a security problem. Checksums detect accidental drift from hardware or floating-point differences, not malicious manipulation.

**Implications:**
- No input validation on a central server — clients are trusted to send legitimate inputs
- No server-side simulation to "check" client results
- No bandwidth spent on state snapshots that exist only to override client predictions
- Anti-cheat is not a design consideration
- If competitive integrity matters, it's solved socially (playing with friends, community norms), not technically

**Technical implementation:** The specific architectural decisions that follow from this principle — lockstep relay, latency hiding, cloud storage over always-on servers, and others — are documented in [architecture-decisions.md](architecture-decisions.md). The networking model is detailed in [network-architecture.md](network-architecture.md) and [network-operations.md](network-operations.md).

---

## Design Decision Checklist

| Question | Violated principle |
|----------|--------------------|
| Does the player have meaningful choice? | Player control |
| Can this be solved through power, skill, or knowledge? | Multiple axes |
| Does the control scheme translate intention? | Avatar responsiveness |
| Will the player feel clever figuring this out? | Discovery |
| Has the player felt the problem before getting the solution? | Pain before relief |
| Does this element have rules that interact with existing systems? | Systems have rules |
| Is the reward proportional to the effort? | Fairness |
| Does the world acknowledge what the player did? | World reactivity |
| Does this moment complement what came before? | Pacing |
| Does this choice have visible, irrevocable, remembered consequences? | Meaningful choice |
| Is every second spent here worthwhile? | Time respect |
| Are we paying any technical cost to prevent cheating? | Player trust |
| If rules are hidden, do stakes match the player's current knowledge? | Discovered contract |

---

## The Player Should Feel

- **Capable** - the controls do what I want
- **Respected** - my time and effort are valued
- **Clever** - I figured something out on my own
- **Free** - I chose how to play and it worked
- **Fair** - I got what I deserved, good or bad
- **Curious** - I wonder what's over there
- **Consequential** - what I decided actually mattered

If any is missing, find the violated principle above.
