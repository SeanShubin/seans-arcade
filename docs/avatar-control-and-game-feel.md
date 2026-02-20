# Avatar Control and Game Feel

Elaborates on [design-philosophy.md](design-philosophy.md) principle #2: The Avatar Does What You Meant.

The best-controlling games lie to you constantly, and the worst-controlling games are often the most honest.

## The Fundamental Tension

Players don't want realistic control. They want **intentional control** - the avatar should do what the player meant, not what they input. These are different things because human inputs are imprecise - we press buttons too early, too late, at wrong angles. Great games silently correct for this. Bad games execute exactly what you input and it feels terrible.

**The game's job is to translate intention into action, not input into action.**

## The Techniques

### 1. Input Buffering

The player presses jump slightly before landing. A realistic game ignores it. A good game remembers the press for 3-10 frames and executes it the moment you land.

In practice: store the last N frames of input. When a state transition happens (landing, finishing an attack, exiting stun), check the buffer for pending actions.

### 2. Coyote Time

The player walks off a ledge and presses jump 3 frames later. Realistically, they're in the air. Coyote time gives a grace period (typically 5-10 frames) after leaving a platform where jumping still works.

The player never notices this. They just feel like the controls are "tight." Remove coyote time from any platformer and it immediately feels unfair.

### 3. Generous Collision/Hit Detection

Celeste does several things simultaneously:
- The player's hurtbox (where they take damage) is smaller than their sprite
- Enemy hitboxes are smaller than the enemy sprites
- Ledge grabs trigger even when the player barely misses the platform
- Spikes on the sides of blocks don't kill you if you're on top of the block

The player sees a near miss and thinks "I barely dodged that!" In reality, the game decided they dodged it. The player feels skillful.

The inverse: if hitboxes are larger than sprites, the player gets hit by things that visually missed them. This is where "that didn't hit me!" frustration comes from.

### 4. Aim Assist and Target Magnetism

Batman Arkham: you press attack, and Batman lunges 15 feet to the nearest enemy with a contextually appropriate move. Your input was "attack, vaguely that direction." Batman's output was a precise flying kick.

This works because:
- The intent interpretation is almost always correct (nearest enemy in the direction you're pushing)
- The result is more satisfying than what precise control would produce
- The rhythm of attack-attack-counter-attack creates a flow state where you feel like you're conducting an orchestra, not puppeteering a mannequin

Zelda: Breath of the Wild's bow has subtle aim assist - the reticle slightly pulls toward targets. The player thinks they're a good shot. The game is helping.

### 5. Animation Canceling and Priority Systems

When a player presses dodge during an attack animation, what should happen?

- **Realistic**: nothing, the attack animation must complete
- **Responsive**: the attack animation is interrupted immediately and the dodge starts
- **Designed**: the attack has specific windows where it can be canceled into a dodge, creating intentional rhythm

Dark Souls uses the third approach and it creates the entire combat feel. You commit to an attack and must live with the commitment unless you're in the right frame window. This feels fair because the rules are consistent and learnable.

### 6. Responsive Feedback Loops

The avatar must acknowledge input immediately, even if the action takes time:

- Press attack: the wind-up animation starts on the exact frame you pressed the button. The damage happens 10 frames later, but the visual response was instant.
- Press jump: the character leaves the ground immediately.
- Change direction: the character sprite flips instantly, even if the movement velocity takes a few frames to reverse.

When there's a gap between input and visual response - even 2-3 frames - the player perceives the game as "laggy" or "floaty."

### 7. Variable Jump Height

Mario's jump height depends on how long you hold the button. Short tap for a small hop, long press for full height.

Implementation: apply upward velocity on press. Continue applying it each frame the button is held, up to a maximum duration. When the player releases, stop adding upward force and let gravity take over.

### 8. Speed and Momentum Curves

A responsive game typically has:
- Near-instant acceleration to max speed (0-2 frames)
- Instant direction change on the ground
- Slightly delayed direction change in the air (but still possible)
- Immediate stop when input ceases (on the ground)

Ice physics (slow acceleration, slow deceleration) feel terrible for normal movement. It works as a deliberate hazard because the player understands they're in an abnormal environment.

### 9. Camera Cooperation

The camera must never fight the player:
- Look slightly ahead of the player's movement direction
- Smoothly follow, not rigidly track - small lag makes the player feel fast
- Never obscure the player during critical moments
- In 3D: never force the player to fight the camera and the enemies simultaneously

Bad cameras make good controls feel bad because the player can't see the results of their inputs.

### 10. Sound Design As Feedback

Every input should produce an immediate sound:
- Jump: sound on launch, not on landing
- Attack: sound on swing, not on hit (hit gets its own sound)
- Dash: whoosh on frame one
- Menu selection: click on press, not on release

Sound arrives faster than visual processing. A sound that confirms your input was received makes the game feel responsive even before the animation plays.

## Why Some Games Feel Bad

Common third-person action game failures:
- Animation priority too strict - inputs during animations ignored rather than buffered
- Camera fights the player in tight spaces
- No generous collision correction - fall off platforms that looked landable
- Movement has momentum curves designed for visual realism rather than player intent
- Attack animations commit the player for too long without cancel windows
- No aim correction on attacks - swing exactly where pointed, rarely exactly at the enemy

## The Unifying Principle

The player's intention is "jump across that gap." Their input is "press jump somewhere approximately near the edge." The game's systems (coyote time, input buffering, generous landing zones) bridge the gap between the imprecise input and the clear intention.

Games that feel bad are usually games that faithfully execute imprecise inputs. Games that feel good are usually games that infer what you meant and do that instead.

## Implementation Priority For a Bevy Project

If building a 2D action game, implement in this order:

1. **Instant visual response** to input (frame 1 sprite change)
2. **Input buffering** (5-10 frame window)
3. **Coyote time** (if platformer)
4. **Variable jump height** (if platformer)
5. **Generous hitboxes** (player hurtbox smaller than sprite)
6. **Sound on input frame**
7. **Animation canceling rules**
8. **Aim assist / target magnetism** (if combat game)

Each one is a small system in Bevy. None is more than 20-30 lines. Collectively they're the difference between "this feels great" and "this feels like a student project."

## Further Reading

- "Game Feel" by Steve Swink
- "Juice it or lose it" (YouTube talk)
