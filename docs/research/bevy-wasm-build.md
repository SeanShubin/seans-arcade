# Building Bevy Examples for WebAssembly

How to compile Bevy examples (e.g., `bouncing_balls`) to WASM and run them in the browser.

## Tool Options

| Tool | Use case | Maturity |
|------|----------|----------|
| **`wasm-server-runner`** | Local dev — `cargo run` opens a browser tab automatically | Stable, widely used |
| **`bevy_cli`** | Bevy-specific build/serve tool | Alpha, not recommended yet |
| **`wasm-bindgen-cli`** | Production builds — generates JS glue + optimized `.wasm` | Stable, manual setup |

For day-to-day iteration, `wasm-server-runner` is the simplest path. Use `wasm-bindgen-cli` when you need a deployable artifact.

## One-Time Setup

```sh
# Add the WASM compile target
rustup target add wasm32-unknown-unknown

# Install the dev runner
cargo install wasm-server-runner
```

## Configuration

### `.cargo/config.toml`

Create this file so `cargo run --target wasm32-unknown-unknown` automatically uses the dev runner:

```toml
[target.wasm32-unknown-unknown]
runner = "wasm-server-runner"
```

### `Cargo.toml` Changes

Two changes are required:

**1. `getrandom` WASM support** — `rand` depends on `getrandom`, which has no default WASM backend. Add the `wasm_js` feature:

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
```

Without this, compilation fails with an error about `getrandom` having no backend for `wasm32-unknown-unknown`.

**2. Target-gate filesystem crates** — `glob` and `zip` use filesystem APIs that don't exist in WASM. Gate them behind a native-only cfg:

```toml
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
glob = "0.3"
zip = "2"
```

And remove `glob` and `zip` from the main `[dependencies]` section. Any code that uses these crates must also be gated:

```rust
#[cfg(not(target_arch = "wasm32"))]
use glob::glob;
```

## Running (Dev)

```sh
cargo run --example bouncing_balls --target wasm32-unknown-unknown
```

With `wasm-server-runner` configured, this compiles to WASM, starts a local HTTP server, and opens your browser.

## Production Build

For a deployable build, use `wasm-bindgen-cli` to generate JS bindings and optionally `wasm-opt` to shrink the binary.

```sh
# Install tools
cargo install wasm-bindgen-cli
# wasm-opt comes from the binaryen package (install via your system package manager)

# Build in release mode
cargo build --example bouncing_balls --target wasm32-unknown-unknown --release

# Generate JS glue code
wasm-bindgen \
    --out-dir out \
    --target web \
    target/wasm32-unknown-unknown/release/examples/bouncing_balls.wasm

# (Optional) Optimize the WASM binary
wasm-opt -Oz -o out/bouncing_balls_bg.wasm out/bouncing_balls_bg.wasm
```

Then serve with a minimal `index.html`:

```html
<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body>
<script type="module">
    import init from './bouncing_balls.js';
    init();
</script>
</body>
</html>
```

Serve the `out/` directory with any static HTTP server (e.g., `python -m http.server`).

## Known Issues

- **Filesystem crates** — `glob` and `zip` cannot compile for WASM. Must be target-gated (see above).
- **Single-threaded** — WASM runs single-threaded by default. Bevy's multi-threaded scheduler falls back to single-threaded mode automatically, but performance differs from native.
- **Asset loading** — Assets are fetched via HTTP in WASM, not read from disk. Bevy handles this transparently, but assets must be served alongside the `.wasm` file.
- **Binary size** — Unoptimized WASM builds can be very large. Use `--release` and `wasm-opt` for reasonable sizes.
- **Audio** — Browser autoplay policies may block audio until the user interacts with the page.
