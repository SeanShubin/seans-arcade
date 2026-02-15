# Classic Game Candidates

Games from the early era of video games, ordered roughly by implementation complexity. Each is small enough to implement completely.

| Game | Year | Key Concepts | Complexity |
|------|------|-------------|------------|
| Pong | 1972 | Two paddles, one ball, score. ~3-4 systems. | Lowest |
| Breakout | 1976 | Paddle, ball, destructible bricks. Adds spawning/despawning entities. | Low |
| Space Invaders | 1978 | Grid of enemies, player ship, projectiles. Adds enemy AI (simple march pattern), shooting in both directions. | Low-Medium |
| Asteroids | 1979 | Ship with rotation, wrapping screen edges, splitting asteroids. Adds rotation physics and entity subdivision. | Medium |
| Snake | 1976 | Grid movement, growing body, food spawning. Tile-based, no physics. | Medium |
| Pac-Man | 1980 | Tile-based movement, ghost AI with distinct personalities, state changes (power pellets). | Medium-High |
| Frogger | 1981 | Lane-based obstacles, timing-based gameplay, multiple hazard types. | Medium-High |
| Tetris | 1984 | Grid state management, piece rotation, line clearing. No physics, pure logic puzzle. | Medium-High |

## Recommended Starting Point

**Pong** or **Breakout**. They're completable in a single session, cover core Bevy concepts (spawning entities, input, movement systems, collision, score state), and the "done" state is unambiguous.
