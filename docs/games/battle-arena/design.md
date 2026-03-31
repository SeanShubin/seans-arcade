# Battle Arena — Design Specification

## Player
- simple geometric shapes where it is easy to tell what direction they are facing
- twin stick controls, left controls movement, right controls direction.
- abilities
  - passive attack, automatic when enemy is in range, melee
  - active attack, press button to initiate in current direction, medium range
  - dash, hold button to activate, 4x speed, drains dash meter, dash meter automatically recharges over time when not in use
  - block, hold button to activate, damage applied to block meter instead of player, block meter automatically recharges over time when not in use
  - charge, tap to target closest enemy in direction, hold to charge up meter over time, release to fire at targeted enemy, long range, powerful

## Arena
- 2 dimensional battle arena
- bounded by a rectangle
- many obstacles for cover

## Enemies
- start with about 10 enemies
- enemies detect the player when about 1/4 screen distance away
- enemies wander if player not detected
- enemies lose track of player if player is out of range
- some enemies are melee, some are ranged
- melee enemies try to close distance to player
- ranged enemies try to maintain optimal range distance from player

## Hud
- dash meter
- block meter
- charge meter
- targeted enemy (if applicable, for charge)