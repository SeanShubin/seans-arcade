# Shape Description Language

How to describe 3D shapes as pure data while retaining the compression power of a programming language.

## Problem

We need a way to describe robot bodies (and eventually all game entities) that is:
- **Pure data** — no arbitrary code, diffable, serializable, inspectable
- **Compressed** — no literal, structural, or contextual duplication
- **Visual** — can be rendered for preview without compilation
- **Parameterized** — the same description produces different variants

A raw voxel list has none of these properties. A full programming language has all of them but loses the "pure data" quality. The goal is the sweet spot between the two.

## Three Kinds of Duplication

### Literal Duplication (two identical things)

Programming analog: extract method.

Shape analog: **named templates**. Define a subtree once, reference it by name.

```toml
[templates.arm]
shape = "box"
size = [0.12, 0.4, 0.12]

[parts.left_arm]
template = "arm"
at = [-0.45, 0.7, 0.0]

[parts.right_arm]
template = "arm"
at = [0.45, 0.7, 0.0]
```

### Structural Duplication (same structure, different values)

Programming analog: polymorphism.

Shape analog: **parameterized templates**. Templates accept values that vary their output.

```toml
[templates.limb]
params = ["length", "width", "segments", "end_effector"]

[[templates.limb.parts]]
shape = "box"
size = ["$width", "$length / $segments", "$width"]
repeat = { count = "$segments", along = "y" }

[[templates.limb.parts]]
template = "$end_effector"

[parts.arm]
template = "limb"
length = 0.4
width = 0.12
segments = 2
end_effector = "claw"

[parts.leg]
template = "limb"
length = 0.5
width = 0.14
segments = 2
end_effector = "wheel"
```

### Contextual Duplication (same intent, different context)

Programming analog: monad / flatMap / reader environment.

Shape analog: **context inheritance**. Values flow down the tree. Each node can read or override them. Children inherit without explicitly passing.

```toml
[context]
material = "armor"
damage = 0.0

[parts.left_half]
context = { side = "left" }
children = ["arm", "leg", "shoulder_pad"]

[parts.right_half]
context = { side = "right", mirror = "x" }
children = ["arm", "leg", "shoulder_pad"]
# Same children, mirrored by context — no per-part repetition
```

## The Three Mechanisms

The data format needs exactly three mechanisms to achieve the same compression as code:

1. **Templates** — name a subtree, reference it by name. Eliminates literal duplication.
2. **Parameters** — templates accept values that vary their output. Eliminates structural duplication.
3. **Context inheritance** — values flow down the tree, each node can read or override, children inherit implicitly. Eliminates contextual duplication.

These are not programming constructs. There are no conditionals, no loops, no arbitrary computation. But they cover the same compression cases:

| Data mechanism | Programming analog | What it compresses |
|---|---|---|
| Template | Function definition | Identical subtrees |
| Parameter | Function argument | Same structure, different values |
| Context | Reader monad / environment | Values that vary by position in the tree |

## Fixed Combinators

The data format does not need general computation. Instead, a small fixed set of **combinators** handled by the Rust interpreter covers the shape operations:

- **mirror(axis)** — reflect children across X, Y, or Z
- **repeat(count, spacing, along)** — duplicate children along an axis
- **chain(along)** — attach children end-to-end
- **smooth_union(k)** — blend children together with smoothness k
- **taper(start_scale, end_scale, along)** — scale children progressively along an axis
- **jitter(seed, amount)** — deterministic random offset per child (for organic/damaged look)

The data says *which* combinator to use and with what values. The Rust code knows *how* to execute each one. This keeps the data format finite and predictable.

## Expressions, Not Statements

When a parameter needs simple math (e.g., "width is 1.0 divided by leg count"), the data format allows **expressions**:

```toml
width = "1.0 / $leg_count"
offset_y = "$length * -0.5"
```

Expressions can reference parameters and use arithmetic (+, -, *, /, min, max). They cannot branch, loop, or have side effects. This is the spreadsheet model — cells contain formulas, not programs. Powerful enough for shape description without becoming Turing-complete.

## What the Rust Interpreter Does

The data format is inert. The Rust code:

1. **Parses** the data file (TOML or RON)
2. **Resolves** templates — expands references, substitutes parameters, inherits context
3. **Evaluates** expressions — arithmetic on parameter values
4. **Executes** combinators — mirror, repeat, smooth_union, etc.
5. **Produces** a tree of positioned, sized, colored primitives (or voxels, or meshes)
6. **Renders** via Bevy's 3D pipeline

Steps 1-5 are pure functions (data in, shape tree out). Step 6 is the Bevy integration. This separation means the shape system can be tested without rendering.

## Example: Complete Robot Description

```toml
[meta]
name = "Scout Bot"
description = "Fast wheeled scout with ranged weapon"

[context]
material = "metal"
color = [0.45, 0.45, 0.50]
accent = [0.8, 0.5, 0.1]

# --- Templates ---

[templates.wheel]
params = ["radius"]
shape = "cylinder"
size = ["$radius", 0.1]
color = [0.25, 0.25, 0.28]
orient = "z"  # lay on side

[templates.sensor_eye]
shape = "sphere"
size = [0.06]
color = [0.9, 0.2, 0.1]
emissive = true

# --- Root ---

[root]
children = ["chassis", "head", "wheels", "weapon"]

[parts.chassis]
shape = "box"
size = [0.7, 0.35, 0.5]
at = [0.0, 0.55, 0.0]

[parts.head]
shape = "sphere"
size = [0.18]
at = [0.0, 0.9, 0.0]
children = ["eye"]

[parts.eye]
template = "sensor_eye"
at = [0.0, 0.0, 0.15]

[parts.wheels]
context = { mirror = "x" }
children = ["wheel_pair"]

[parts.wheel_pair]
template = "wheel"
radius = 0.18
repeat = { count = 2, spacing = 0.3, along = "z", center = true }
at = [0.42, 0.18, 0.0]

[parts.weapon]
shape = "cylinder"
size = [0.04, 0.5]
at = [0.0, 1.05, 0.0]
orient = "x"  # barrel points forward
color = [0.3, 0.3, 0.35]
joint = { type = "turret", axis = "y", speed = 2.0 }
```

This describes a complete robot in ~50 lines of data. The same robot in raw voxel coordinates would be thousands of entries. The compression comes from templates (wheel used 4 times, defined once), context (mirror produces left+right from one definition), and the interpreter knowing what "cylinder with orient = z" means.

## Where This Breaks Down

**Conditional logic** — "if this robot has more than 4 legs, shrink them to fit." Two options:

1. Keep the data dumb. Put conditional logic in the Rust interpreter. The data says "6 legs of size X" and the interpreter ensures they fit. Preferred approach.
2. Allow `min(0.3, 1.0 / $leg_count)` in expressions. Covers most cases without branching.

**Truly procedural shapes** — "a tree with random branching." The data format cannot do this. Use the noise/SDF code libraries directly for organic shapes, and the data format for mechanical/constructed things. This maps to the game's own distinction: robots are data-described, terrain is code-generated.

## Relationship to Existing Decisions

- **Hybrid rasterizer + SDF rendering** — the shape description produces geometry that feeds into the existing rendering pipeline
- **Robots, not humans** — mechanical shapes are well-suited to this compositional approach
- **Two-tier rendering** — shape descriptions are pre-processed to meshes/voxels (pre-computed tier), animation transforms happen at runtime
- **Visual language over realism** — the data format includes joint types and animation hints, ensuring that shape and behavior are described together

## Prior Art

| System | What it does | Lesson |
|---|---|---|
| CSS | Context inheritance (cascade), templates (classes), parameters (custom properties) | Proves inheritance + templates compress UI description without being a language |
| OpenSCAD | Declarative 3D modeling with pure functions | Close to what we want, but chose to be a full language |
| URDF/SDF (robotics) | XML trees of links and joints with parameters, xacro macros for templates | Solves the exact same problem for real robots |
| glTF | Scene graph with node hierarchy, mesh references, material parameters | Pure data, no logic — but no templates or parameters |
