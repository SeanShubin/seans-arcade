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

### Parallelism Dimensions

Bevy's parallelism is safe by construction, enforced by the type system rather than runtime locks. Every system declares its access needs in its function signature — `Res<T>` / `ResMut<T>` for resources, `Query<&T>` / `Query<&mut T>` for components. The scheduler inspects these declarations at app startup and builds an execution plan. By the time systems actually run, all conflicts are already resolved. There is no locking, no contention, no deadlocks.

The tradeoff: you give up the freedom to grab whatever data you want whenever you want (like a global singleton in Java). In return, you get parallelism that is correct by construction. **Failure modes are all performance, never correctness.** You write slow schedules, not broken ones.

The scheduler resolves conflicts along three dimensions:

**By type.** Systems accessing different types never conflict. A system writing `Transform` and a system writing `Velocity` run in parallel, no questions asked. This applies equally to resources and components.

**By read vs write.** Multiple readers of the same type don't conflict. `Res<Score>` and `Res<Score>` can run in parallel. `Query<&Transform>` and `Query<&Transform>` can run in parallel. Only writers (`ResMut`, `Query<&mut T>`) create exclusivity.

**By entity set.** Two systems can both write the same component type if they provably touch different entities. Within a single system, you prove disjointness with `Without<>` filters:

```rust
fn my_system(
    players: Query<&mut Transform, With<Player>>,
    enemies: Query<&mut Transform, (With<Enemy>, Without<Player>)>,
)
```

This dimension only exists for components. Resources are global singletons — there is no "which resource instance" to partition on.

| Dimension | Resources | Components |
|---|---|---|
| By type | Yes | Yes |
| By read vs write | Yes | Yes |
| By entity set | No (singletons) | Within a system only |

**Cross-system caveat.** The entity-set dimension only works within a single function signature — where Bevy can statically verify that queries are disjoint. Between separate systems, the scheduler is conservative: it looks at component-level access, not filter-level access.

This works — one system, two queries, disjointness proven by `Without<>`:

```rust
fn move_all(
    players: Query<&mut Transform, With<Player>>,
    enemies: Query<&mut Transform, (With<Enemy>, Without<Player>)>,
) { ... }
```

This serializes — two systems, same component, scheduler can't see the filters:

```rust
fn move_players(players: Query<&mut Transform, With<Player>>) { ... }
fn move_enemies(enemies: Query<&mut Transform, With<Enemy>>) { ... }
```

Both systems write `Transform`, so the scheduler treats them as conflicting even though they touch different entities. For cross-system parallelism, only the type and read-vs-write dimensions apply.

### Drawing Abstraction Boundaries Along Parallelism Dimensions

Each dimension answers a different design question:

**"Do these systems touch different types?"** (Type dimension) — This is the primary tool. Every distinct type is an independent parallelism lane. Group data that's always accessed together into one type. Separate data that's accessed by different systems into different types. This is the "split along access boundaries, not data boundaries" principle — it applies to both resources and components. Components are the more flexible primitive: resources give you two axes of parallelism (type + read vs write), components give you all three. When you hit contention on a resource, moving the data to components opens up the entity dimension as an escape hatch. Resources are for truly global state — input, score, time, configuration — where there is exactly one instance and the concept of "which entity" doesn't apply. If you find yourself wanting two instances of a resource, it should be a component.

**"Does this system actually need to write?"** (Read vs write dimension) — Every unnecessary `mut` is parallelism left on the table. A system that reads `Score` can run alongside every other `Score` reader. A system that writes `Score` forces all of them to wait. `Res` instead of `ResMut`. `Query<&T>` instead of `Query<&mut T>`. This is the cheapest optimization — just change the signature.

**"Should these be one system or two?"** (Entity set dimension) — When two logical operations write the same component to different entities (move players, move enemies), two separate systems is cleaner architecturally but they serialize. One combined system with disjoint queries (`Without<>`) parallelizes internally but is less focused. Default to separate systems for clarity. Combine only if profiling shows the serialization actually matters.

**When each dimension matters.** Type splitting is a design-time decision that's hard to change later — get it roughly right up front. Read vs write is trivial to fix anytime. System consolidation for entity-set parallelism is a performance optimization you defer until measurement tells you it matters.

### System-Level vs Iteration-Level Parallelism

Bevy's automatic parallelism operates at the **system level**: the scheduler runs non-conflicting systems on different threads simultaneously. It does NOT parallelize iteration within a single system. A `Query` loop processes entities sequentially on one thread by default.

```rust
fn move_enemies(mut q: Query<&mut Transform, With<Enemy>>) {
    for mut transform in &mut q {
        // sequential — one enemy at a time, single thread
        transform.translation.x += 1.0;
    }
}
```

To parallelize iteration within a system, opt in explicitly with `par_iter`:

```rust
fn move_enemies(q: Query<&mut Transform, With<Enemy>>) {
    q.par_iter_mut().for_each(|mut transform| {
        // parallel — enemies distributed across threads via Bevy's task pool
        transform.translation.x += 1.0;
    });
}
```

`par_iter` is worth it when per-entity work is non-trivial (pathfinding, spatial queries, complex AI). For simple operations like applying velocity to position, the overhead of task distribution usually exceeds the savings. Profile before opting in.

### Commands Are Deferred

`Commands` is a special system parameter. Unlike `Query` or `ResMut`, it doesn't access any data immediately — it queues structural changes (spawn, despawn, insert/remove components) that are applied later at a **sync point** between system sets. Because `Commands` doesn't read or write anything during system execution, it doesn't conflict with any other system parameter. A system using `Commands` can run in parallel with any other system, regardless of what that other system accesses.

This is why startup systems that only call `commands.spawn(...)` never cause scheduling conflicts — they produce deferred work, not immediate data access.

### Resource Granularity: Three Failure Modes

**Capturing too broadly.** If a system declares `ResMut<Score>` but only reads it, the scheduler treats it as a writer and serializes it against every other system that touches `Score`. Use `Res<Score>` instead and it runs in parallel with other readers.

**Not splitting enough (god resources).** If too much state lives in one resource (e.g., a single `GameState` struct with everything), then every system that touches any part of it conflicts with every other. Splitting into focused resources (`Score`, `PaddleInput`, `BallVelocity`) gives the scheduler more room to parallelize.

**Splitting too much.** Over-splitting has costs:
- *Boilerplate* — If a system genuinely needs many resources, the function signature becomes unwieldy.
- *No scheduling benefit* — If you split `GameState` into `BallPosition`, `BallVelocity`, `BallSpin`, but your physics system needs `ResMut` on all three, you haven't gained parallelism. The scheduler still serializes anything that conflicts with any of them.
- *Logical fragmentation* — State that always changes together probably belongs together. If two resources are never accessed independently, splitting them adds two lookups instead of one with no scheduling benefit.

**The design principle: split along access boundaries, not data boundaries.** If different systems access different subsets of the state, split there. If everything that reads part of it also reads the rest, keep it together.

### Overlapping Access Patterns

When two systems need overlapping but not identical subsets of state, there are three strategies:

1. **Venn diagram split.** Shared fields become their own resource, unique fields stay separate. The systems still serialize on the shared resource, but other systems that only need the unique parts can run freely. This works but gets awkward if access patterns shift.

2. **Accept the serialization.** Keep it as one resource. If the two systems can't run in parallel anyway because of the overlap, splitting just adds indirection for no scheduling benefit. Simpler code wins.

3. **Move the data to components on entities.** Components give you a dimension that resources don't: per-entity access. If two systems touch the same component types but on different entities, the scheduler can still parallelize them. Resources are global singletons with no such escape hatch — any writer blocks all other accessors.

Overlapping resource access problems are often a signal that the data isn't truly global — it belongs on entities, where the query system handles partial overlap naturally. Resources work well for truly global state (input, score, time).

### How Parallelism Gets Defeated

The type system draws a hard line between two categories of failure. Staying within it can only cost performance. Going around it can break correctness.

**Within the type system — performance degrades, correctness preserved:**

- *Unnecessary `mut`* — `ResMut<T>` when `Res<T>` would do, `Query<&mut T>` when `Query<&T>` would work. Every writer excludes all other accessors of that type.
- *God resources / god components* — One big struct everything touches. Every system conflicts with every other.
- *Exclusive systems* — A system taking `&mut World` gets exclusive access to everything. Nothing else runs alongside it. Necessary sometimes (scene loading, structural changes) but serializes the entire schedule while it runs.
- *Non-Send resources* — Resources that aren't `Send` (window handles, GPU contexts) force their systems onto the main thread. Other systems can still run on other threads, but work is pinned to one core.

The worst case: your "parallel" game loop is effectively single-threaded. But it's still correct. The scheduler never lets two conflicting accesses happen simultaneously.

**Outside the type system — correctness breaks:**

- *`unsafe` access* — Raw pointers, `UnsafeCell`, or `UnsafeWorldCell` to read/write data the function signature doesn't declare. The scheduler doesn't know about these accesses, so it may schedule conflicting systems in parallel. Result: data races, torn reads, corrupted state, undefined behavior.
- *Global mutable state outside ECS* — `static mut`, lazy statics with interior mutability, thread-local state. The scheduler can't see it, can't protect it.
- *`Mutex`/`RwLock` inside components or resources* — Technically safe Rust (no UB), but reintroduces runtime contention that the ECS was designed to eliminate. Lock contention, potential deadlocks, and non-deterministic ordering — the exact problems Bevy's type-driven scheduling exists to avoid.

| Approach | What breaks | Symptom |
|---|---|---|
| Unnecessary `mut` | Performance | Systems serialize, underused cores |
| God resources/components | Performance | Everything waits in line |
| Exclusive systems | Performance | Full schedule stall |
| `unsafe` / global state | **Correctness** | Data races, UB, heisenbugs |
| Interior mutability (`Mutex`) | **Determinism** | Lock contention, non-deterministic ordering |

### Parallelism, Ordering, and Determinism

Bevy's scheduler guarantees **safety** (no conflicting access), not **ordering**. When two systems run in parallel, their completion order is non-deterministic. Whether this matters depends on the operation.

**Order doesn't matter when:**

- *Operations are independent per entity.* Each entity's update only reads/writes its own components and the computation is self-contained. Moving 1000 enemies by their individual velocities produces the same result regardless of which enemy gets processed first: `transform.translation += velocity.0 * time.delta_secs();`
- *Operations are commutative.* Adding score from two kills: `10 + 20 = 20 + 10`. Doesn't matter which system runs first.
- *Integer or fixed-point math.* Integer addition is truly associative: `(a + b) + c == a + (b + c)` always. No rounding surprises.

**Order matters when:**

- *Floating-point accumulation across entities.* Summing floats from multiple entities gives different results depending on the order, because float addition isn't associative. `(0.1 + 0.2) + 0.3 != 0.1 + (0.2 + 0.3)` in IEEE 754. This is the `par_iter` trap — thread scheduling changes the accumulation order.
- *Sequential dependencies.* Collision resolution where entity A pushes B, then B pushes C. Final positions depend on processing order.
- *State machines with shared transitions.* Two systems both check "is the game tied?" and one triggers overtime while the other triggers sudden death. Who runs first determines the outcome.
- *Read-then-write on shared state.* System A reads score, adds 10. System B reads score, adds 20. If both read before either writes, you get a lost update. Bevy prevents this via scheduling — but the order of the serialized execution still determines intermediate states that other systems might observe.

**The practical rule for lockstep:** you don't need to constrain everything — just the operations where order changes the result. Independent per-entity updates with integer/fixed-point math are safe to parallelize freely. Cross-entity accumulations and sequential dependencies need deterministic ordering via `.before()` / `.after()`.

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
