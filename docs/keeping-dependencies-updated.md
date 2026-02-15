# Keeping Dependencies Updated

## Rust Toolchain

Rust releases a new stable version every 6 weeks. Update with:

```bash
rustup update stable
```

Check your current version:

```bash
rustc --version
```

Bevy releases specify a minimum supported Rust version (MSRV). If `cargo check` fails after updating Bevy, run `rustup update stable` first.

## Checking for Outdated Crates

Install `cargo-outdated` (one-time):

```bash
cargo install cargo-outdated
```

Then check what's behind:

```bash
cargo outdated
```

This shows a table of current vs latest versions for all dependencies.

## Updating Dependencies

### Patch updates (safe, do regularly)

```bash
cargo update
```

This updates dependencies within the version constraints in `Cargo.toml`. For example, if `Cargo.toml` says `bevy = "0.18.0"`, `cargo update` will pull in `0.18.1`, `0.18.2`, etc., but not `0.19.0`. These updates are backwards-compatible by semver convention.

### Major/minor updates (review changelog first)

Edit `Cargo.toml` to change the version number, then:

```bash
cargo check
```

For Bevy specifically, major version upgrades (e.g., 0.18 to 0.19) include breaking API changes. Consult the [Bevy migration guides](https://bevyengine.org/learn/migration-guides/) before upgrading.

## Automated Updates with GitHub

If this repo is hosted on GitHub, enable **Dependabot** by adding `.github/dependabot.yml`:

```yaml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
```

This creates pull requests automatically when new dependency versions are available. You review and merge at your own pace.

**Renovate** is an alternative that tends to handle Rust projects well. Enable it through the [Renovate GitHub App](https://github.com/apps/renovate).

## Suggested Routine

1. Before starting a new experiment, run `cargo outdated` to see if anything is behind.
2. Run `cargo update` to pick up patch releases.
3. If a new Bevy minor version is out, read its changelog, update `Cargo.toml`, and fix any breaking changes.
4. Run `rustup update stable` occasionally to stay current on the compiler.
