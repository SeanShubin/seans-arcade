# Static Analysis: Porting code-structure to Rust

## What code-structure Detects (Language-Agnostic)

Four metrics, all applicable to Rust's module hierarchy:

1. **IN_DIRECT_CYCLE** - Two or more code units that depend on each other directly (A -> B -> A)
2. **IN_GROUP_CYCLE** - Module-level cycles from cross-module class dependencies (login.Service -> auth.Verifier -> login.Hash creates login <-> auth cycle)
3. **ANCESTOR_DEPENDS_ON_DESCENDANT** - Parent module depends on child module
4. **DESCENDANT_DEPENDS_ON_ANCESTOR** - Child module depends on parent module

## What Rust Gives You for Free

- **Cross-crate cycles are impossible.** Cargo won't compile them. The most critical metric is enforced by the compiler at the crate boundary.
- **Visibility is explicit.** `pub`, `pub(crate)`, `pub(super)` give cleaner dependency signals than JVM's package-private.
- **Module hierarchy is strict.** The `mod` tree is a true hierarchy; no classpath ambiguity.

## The Hard Part: No Bytecode Equivalent

JVM bytecode provides actual runtime dependencies (method calls, field access) in a uniform binary format. Rust has no equivalent artifact.

### Options for Dependency Extraction

| Approach | Pros | Cons |
|----------|------|------|
| **Parse `use` statements + AST via `syn` crate** | Simple, Rust-native, covers majority of real dependencies | Misses some implicit dependencies (trait method calls without explicit import) |
| **Use `rust-analyzer` programmatically** | Full name resolution, closest to bytecode accuracy | Complex integration, slower |
| **Parse compiler metadata** (`cargo metadata`, `cargo check --message-format=json`) | Easy to invoke, crate-level structure | Too coarse for module-level analysis |

### Recommendation

Start with `syn` crate for name extraction and `use`-statement-based dependency analysis. Rust's explicit import system means `use` statements capture the vast majority of real dependencies. Refine with rust-analyzer integration later if needed.

## Mapping code-structure Concepts to Rust

| code-structure (JVM) | Rust Equivalent |
|-----------------------|-----------------|
| Package (e.g., `com.example.login`) | Module path (e.g., `crate::login`) |
| Class / top-level declaration | `struct`, `enum`, `fn`, `trait` within a module |
| `.class` file constant pool | `use` statements + AST type references |
| `$` truncation (inner classes fold to outer) | Not needed; Rust modules are explicit separate units |
| Maven module boundary | Cargo crate boundary |
| Package within module | Module within crate |

## Core Algorithm (Reusable As-Is)

- **Tarjan's strongly connected components** for cycle detection - language independent
- **Hierarchical name prefix matching** for ancestor/descendant checks - works with module paths
- **Scoped analysis at multiple levels** (global, per-module depth) - same concept

## Implementation Phases

1. **Phase 1**: Write a Rust source parser using `syn` to extract module structure and `use` dependencies
2. **Phase 2**: Implement cycle detection and ancestor/descendant checks (port Tarjan's from code-structure)
3. **Phase 3**: Generate reports (reuse report format concepts from code-structure)
4. **Phase 4**: Integrate with `cargo` as a custom subcommand or build script
5. **Optional Phase 5**: Integrate rust-analyzer for deeper dependency resolution
