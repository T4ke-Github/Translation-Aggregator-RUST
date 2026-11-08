# Continuous integration

Workflows live in `.github/workflows/`. While the repository has no root
`Cargo.toml`, a small `detect` job finds no workspace and every Rust job shows
as **skipped**, not failed. Once the workspace exists, no workflow changes are
needed.

## `ci.yml`: every push to `main`, every PR, manual runs

| Job | Runner | What it checks |
| --- | --- | --- |
| `detect` | Linux | Whether `Cargo.toml` exists; reads `rust-version` (MSRV) from it |
| `fmt` | Linux | `cargo fmt --all --check` |
| `clippy` | Windows, x86_64 + i686 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| `test` | Windows, x86_64 + i686 | `cargo build --all-targets`, `cargo nextest run`, doctests (`cargo test --doc`) |
| `test-linux` | Linux | Off by default. Clippy + nextest for platform-independent crates (see below) |
| `docs` | Windows | `cargo doc --no-deps --document-private-items` with `RUSTDOCFLAGS=-D warnings` |
| `msrv` | Windows | `cargo +<MSRV> check --workspace` |
| `cargo-deny` | Linux | `bans licenses sources` (blocking) and `advisories` (warning only) using `deny.toml` |
| `ci-result` | Linux | One aggregate status. Make this the only required check in branch protection |

Notes:
- **Both Windows targets** are required: the app ships as a 32-bit and a 64-bit
  build. i686 test binaries run natively on the x64 runner.
- **`--locked`** is used everywhere, so `Cargo.lock` must be committed (this is
  the right call for an application).
- **Caching**: `Swatinem/rust-cache`. Only runs on `main` save caches; PRs restore them.
- **MSRV** is **1.90**: set `rust-version = "1.90"` in the root `Cargo.toml`
  (`[workspace.package]`). CI picks it up automatically. `MSRV_FALLBACK` in
  `ci.yml` (currently `1.90`) is used only when that field is missing.
- **Linux job**: set the repository variables `CI_ENABLE_LINUX=true` and,
  optionally, `CI_LINUX_PACKAGES="-p crate-a -p crate-b"`
  (Settings -> Secrets and variables -> Actions -> Variables). The default
  `--workspace` only works if the Windows-only crates compile to nothing off
  Windows (`#![cfg(windows)]`).
- **Advisories** do not block PRs, because a newly published advisory has nothing to do with
  the PR. The weekly workflow fails on them instead.

## `scheduled.yml`: Mondays, plus manual runs

- `advisories`: `cargo deny check advisories`, blocking.
- `beta`: `cargo +beta test` on both Windows targets, to catch breakage
  before the next stable Rust release.

## Supply chain

- `deny.toml` (repo root): permissive license allow-list (the project is
  `MIT OR Apache-2.0`), crates.io-only sources, no `*` version requirements,
  RustSec advisories. **Copyleft crates (GPL, LGPL, AGPL) are rejected**;
  MPL-2.0 is allowed. See the comments in `deny.toml`.
- Every third-party action is pinned to a commit SHA, with the version in a
  comment. `.github/dependabot.yml` updates actions and cargo dependencies
  weekly, with a cooldown period. `dtolnay/rust-toolchain` has no releases, so
  its pin (a `master` commit) has to be bumped by hand.
- Tool versions (cargo-nextest, cargo-deny) are fixed by the pinned
  `taiki-e/install-action` release, and checksums are verified.

## Running the checks locally

```sh
rustup target add i686-pc-windows-msvc        # once
cargo install --locked cargo-nextest cargo-deny  # once (or: cargo binstall ...)

cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo clippy --workspace --all-targets --all-features --locked --target i686-pc-windows-msvc -- -D warnings
cargo nextest run --workspace --locked
cargo nextest run --workspace --locked --target i686-pc-windows-msvc
cargo test --workspace --doc --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items --all-features --locked
cargo +1.90 check --workspace --all-features --locked   # replace 1.90 with your MSRV
cargo deny --all-features check
```

PowerShell: set the doc flags with `$env:RUSTDOCFLAGS = "-D warnings"` first.
