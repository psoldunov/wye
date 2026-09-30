# Wye: agent and contributor guide

Wye is a native Linux browser picker written in Rust. It registers as the desktop's
default web browser and sends every link to the right browser, browser profile, private
window or desktop app, or asks with a small picker. The specification is in
[docs/spec/](docs/spec/README.md); the design is in [docs/architecture.md](docs/architecture.md).

## Layout

| Path | Contents |
|------|----------|
| `crates/wye-core` | Pure routing core: config, targets, rules, matchers, URL cleaning, redirect unwrapping, web app catalogue, pipeline. No IO. |
| `crates/wye-desktop` | Linux integration: desktop entries, browser discovery and profiles, Exec/launch, `mimeapps.list` default browser, source-app detection. |
| `crates/wye` | The `wye` binary and CLI. |
| `data/` | Shipped data (`services.toml`, `expansion.toml`, `tracking-parameters.toml`), the desktop entry and the icon. |
| `nix/`, `flake.nix` | Package, checks and dev shell. |
| `docs/spec/` | The specification. |

Every requirement in the spec has an ID such as `PIPE-06`. Cite the ID in code comments,
tests and commit bodies.

## Commands

Host has no Rust toolchain. Run everything through the dev shell:

```sh
nix develop -c cargo build
nix develop -c cargo test --workspace --locked
nix build -L
```

## Review gates

Run in this order. Every gate must pass before a change is ready.

1. `nix develop -c cargo fmt --all --check`
2. `nix develop -c cargo clippy --workspace --all-targets --locked -- --deny warnings`.
   `clippy::pedantic` is on workspace-wide. Each `#[allow]` needs a comment giving the reason.
3. `nix develop -c cargo test --workspace --locked`
4. `nix develop -c rust-doctor --yes --blocking warning`. A change must not introduce findings.
   Install once with `nix develop -c cargo install --locked rust-doctor`. CI installs the
   pinned version and runs it in `.github/workflows/rust-doctor.yml` (read-only token, no pull
   request comment); config, if needed, goes in `rust-doctor.toml`.
5. `nix develop -c cargo deny check` (advisories need the network).
6. `nix flake check -L --keep-going`

## Commits

Conventional Commits: `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`, `perf:`, `ci:`.
