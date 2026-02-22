# 9 Keys — Logical Design

This specification covers the logical design of the game only. Visuals will be in a separate specification.

## Maze Structure

- The player navigates through a maze.
- Each space is a square on a grid.
- The player always occupies a space.
- Each square has four transitions: up, down, left, right.
- Each transition connects two squares.
- Each transition between a grid square is either passable, gated, or impassable.
- Impassable transitions are arranged to make the maze finite.
- Maze size should favor smaller, but not so constrained that the ways the maze can be generated are predictable to a human.

## Keys and Gates

- There are 9 types of gates, and 9 types of keys.
- The 9 keys are scattered about the maze.
- Each key has a unique shape.
- Each key has a unique color.
- Each key has a unique name.
- Each key corresponds to a type of gate.
- There is only one instance of each key.
- There are one or more instances of each type of gate.
- Gates are passable when the player has the corresponding key, impassable otherwise.
- Individual players are prevented from moving through a gate they don't have a key for.
- Keys are never lost once acquired.

## Items

- The two types of items are keys and Ariadne's Thread.
- No two items are in the same space.
- There are no items in the starting space.
- When the player enters a space with a key, the key is automatically picked up for that player.
- When the player enters a space with Ariadne's Thread, Ariadne's Thread is automatically picked up for that player.
- Each player gets their own instance of an item.

## Ariadne's Thread

- Ariadne's Thread will indicate if there are accessible unexplored spaces down that path.
- What is "accessible" is determined by what keys the player currently has.
- Picking up Ariadne's Thread is optional.
- Ariadne's Thread can be acquired before any keys.

## Exploration

- A square becomes "explored" by the player entering it.
- Gates, keys, passable, impassable, and where keys were picked up from, all have obvious visual indicators.

## Start and End

- The player starts in a space adjacent to the gate that transitions to the end of the maze.
- The end of the maze has 1 gated transition, the remaining 3 transitions are impassable.
- The player wins by entering the end of the maze.
- Players may remain in the maze after they have won, they will have access to all of it.

## Key Progression

- The player cannot obtain the final gate key without obtaining all other keys first.
- The first key is the only key that can be reached without any other keys.
- The second key cannot be reached without the first key.
- The third key cannot be reached without the second key.
- The next 3 keys can be gathered in any order, but the maze topology will require the first 3 keys be gathered first.
- The final 3 keys will require backtracking to previously visited areas, meaning the maze topology will have required the player to pass gates these keys were locked behind at the time.

## Maze Generation

- The maze will be randomly generated (using deterministic randomness).
- The generated maze design will be such that none of the statements in this design document are untrue.
- The maze is always solvable.
- Players can see the maze seed.

## Scoring

- Score is by a timer, lower timer is better score.
- Each player has their own timer
- Timer starts upon the player first moving
- Timer ends when the player reaches the end of the maze
- An active timer pauses when leaving the game, resumes upon re-entering the game
- There is no loss condition.

## Multiplayer

- No limit to the number of players, players can come and go as they please.
- All players are in the same instance of the maze.
- This is real-time, simultaneous movement.
- No exploration sharing.
- No player interaction, other than being able to see each other.
- Players can see each other.
- Everyone gets an independent score.
