# Architectural Rules in ECS: What Applies, What Doesn't

## Why ECS Is a Different Paradigm

ECS exists because game loops process thousands of entities per frame. Data-oriented design (structs of arrays, cache-friendly iteration) is a hard performance requirement, not a style preference. OOP dependency injection optimizes for isolation and testability at the cost of indirection. ECS optimizes for throughput at the cost of the patterns familiar from business applications.

This is analogous to how functional programming doesn't need constructor injection because it uses different composition mechanisms. ECS has different maintainability problems (system ordering, implicit data coupling, resource contention) that need different solutions.

## Rule-by-Rule Assessment

### Rules That Apply Directly

**Coupling and Cohesion** - Fully applies. Organize systems into Bevy Plugins by domain. A "combat" plugin contains combat systems, components, and events. A "movement" plugin contains movement systems. Changes to combat logic should be localized to the combat plugin.

**Abstraction Levels** - Fully applies. Separate high-level orchestration systems from low-level mechanical systems. A system that coordinates game state transitions is higher level than a system that calculates physics.

**Package Hierarchy** - Fully applies. Rust modules enforce this naturally. No cycles between modules. Organize by game domain (combat/, inventory/, ui/) not by ECS role (systems/, components/, resources/).

**Event Systems** - Applies with adaptation. Bevy has `EventWriter<T>` / `EventReader<T>` which are typed and explicit. Events are the primary decoupling mechanism in Bevy. The difference: event handlers are registered in the App builder, not constructor-injected.

**Anonymous Code** - Fully applies. Name your system parameters meaningfully. Extract complex queries into named helper functions.

### Rules That Need Adaptation

**Dependency Injection** - The concept applies but the mechanism is fundamentally different.

| OOP DI Concept | Bevy ECS Equivalent |
|----------------|-------------------|
| Constructor injection | System function parameters (Query, Res, EventReader, etc.) |
| Interface / trait object | The component/resource types themselves (concrete, not abstracted) |
| Composition root | `App::new().add_plugins().add_systems()` builder |
| Staged DI | Bevy States + startup systems + run conditions |
| Integrations boundary | Bevy owns the boundary (window, input, rendering); you configure, not inject |
| Faking for tests | `World::new()` lets you construct a minimal world with only the components/resources a system needs |

Key insight: Bevy's system parameter declaration IS the dependency declaration. When you write `fn move_player(query: Query<&mut Transform, With<Player>>, time: Res<Time>)`, you've declared that this system depends on Transform components of Player entities and the Time resource. The scheduler provides them. This is inversion of control, just not via constructors.

**Free-Floating Functions** - Does not apply to Bevy systems. Systems MUST be free functions (or closures) for the scheduler to work. This is intentional and idiomatic. Wrapping them in structs would defeat Bevy's design. However, non-system utility functions should still be organized in modules with clear names.

### Rules That Don't Apply

**Language Separation** - Not relevant in a pure Rust + Bevy context. No HTML/CSS/SQL embedding.

**Staged Dependency Injection (the specific pattern)** - The Integrations -> Bootstrap -> ApplicationDependencies staging pattern doesn't map to Bevy. Instead:
- Bevy's plugin system provides modularity
- Startup systems handle initialization
- States handle phase transitions
- Resources hold shared configuration
- The App builder is the single composition root

## What to Watch For Instead (ECS-Specific Maintainability)

ECS has its own set of maintainability problems:

| Problem | Symptom | Solution |
|---------|---------|----------|
| **System ordering bugs** | Behavior depends on which system runs first | Use explicit system ordering (.before/.after) or system sets |
| **Implicit data coupling** | Two systems modify the same component without knowing about each other | Use events for cross-domain communication; document shared components |
| **God resources** | A single Resource struct that everything reads/writes | Split into focused resources per domain |
| **Component soup** | Entity has 20 components with no logical grouping | Use marker components and bundles to group related components |
| **Plugin bloat** | One plugin with 30 systems | Split plugins by subdomain, same as splitting large packages |

## Testing in Bevy

Bevy supports isolated testing without running the full engine:

```rust
// Create a minimal world with only what the system needs
let mut world = World::new();
world.insert_resource(Time::default());
world.spawn((Transform::default(), Player));

// Run the system
let mut schedule = Schedule::default();
schedule.add_systems(move_player);
schedule.run(&mut world);

// Assert on world state
let transform = world.query::<&Transform>().single(&world);
assert_eq!(transform.translation.x, expected_x);
```

This is analogous to the Test Orchestrator pattern: hide infrastructure, assert on domain state. The "tester" is the World setup.
