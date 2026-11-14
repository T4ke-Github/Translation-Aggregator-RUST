# Contributing

Thanks for helping with the Rust rewrite of Translation Aggregator. Read this page before your
first pull request: the clean-room rule below is what lets the project be licensed
`MIT OR Apache-2.0`.

- What to build: [`docs/specs/`](docs/specs/README.md) (features and acceptance criteria) and
  [`docs/product-direction.md`](docs/product-direction.md) (decisions).
- How CI checks a change: [`.github/CI.md`](.github/CI.md).

## The clean-room rule (D13)

The legacy Translation Aggregator is licensed GPLv2. A Rust translation of its code would be a
GPLv2 derivative, so the rewrite is a **clean reimplementation**:

1. **Work from behaviour and formats, not from legacy code.** Implement a feature from its spec,
   from `docs/analysis/` (which describes behaviour and file formats), from the legacy app's
   observable behaviour and from black-box fixtures. Don't open the legacy C++ files while you
   write the Rust code for the same feature.
2. **Copy nothing from the legacy codebase**: no code, no comments, no identifiers beyond public
   format names (INI keys, file names, hook-code syntax), no tables, no data files
   (`Conjugations.txt`, rule sets, sample INI files), no images or icons, no documentation text.
   CI fails if a file matches a legacy file by hash (`tools/legacy-check/`).
3. **Reading legacy code is for documentation only.** Someone who reads legacy code to answer a
   question about behaviour writes the answer as a description in `docs/analysis/` (behaviour,
   format, inputs and outputs), not as code. If you have read the legacy code for a feature,
   say so in your pull request and let someone else write that feature's implementation.
4. **Third-party data keeps its own licence.** Dictionaries (EDICT/JMdict, IPADIC, UniDic) are
   downloaded by the user or shipped with their licence and attribution (NFR-09.3).
5. **New dependencies must be permissive.** `cargo deny` blocks GPL, LGPL and AGPL crates
   (`deny.toml`).

Every pull request answers the checklist in the pull request template.

## Building

- Windows 10 or 11 with the Visual Studio Build Tools (the "Desktop development with C++"
  workload) for the MSVC linker.
- Rust stable, with both app targets:
  `rustup target add x86_64-pc-windows-msvc i686-pc-windows-msvc`.
- The minimum supported Rust version is **1.90** (`rust-version` in `Cargo.toml`, D2). Don't use
  newer language or library features; CI checks it.

```sh
cargo build --workspace                                   # 64-bit
cargo build --workspace --target i686-pc-windows-msvc     # 32-bit
```

Both builds must have the same features (D1, NFR-01.3). The commands CI runs are listed in
[`.github/CI.md`](.github/CI.md#running-the-checks-locally).

## Workspace

All crates live in `crates/`; the layout and the allowed dependencies between them are in
[`docs/analysis/workspace-layout.md`](docs/analysis/workspace-layout.md). Each crate's `lib.rs`
says whether it may contain `unsafe` code. Only `ta-platform` (Win32) is meant for FFI; `ta-dict`
and `ta-morph` deny `unsafe` except where an item allows it with a `// SAFETY:` comment.
The hook plugin crates will live in `plugins/hook/` (D19) once its spec is written.

## Tests

Use the method the acceptance criterion names (`docs/specs/README.md`, NFR-12):

| Kind | Where | Notes |
|---|---|---|
| `unit` | `#[cfg(test)]` modules in the crate | Pure logic. Must pass on both Windows targets. |
| `integration` | the crate's `tests/` folder | Several parts together, no network. |
| `contract` | the provider crate's `tests/` folder, with recorded responses in `tests/fixtures/` or a local mock server | Every official preset and LLM backend, including timeouts, errors and rate limits. CI never calls a real provider (NFR-12.2). |
| `parity` | tests that read `fixtures/legacy/` | Black-box comparison with output captured from the legacy app. An intended difference (D20) is marked in the test and listed in `docs/specs/README.md` under "Behaviour changes". |
| `security` | the crate's `tests/` folder, named `security_*` | Secret canary, local API binding, token checks, limits, plugin isolation (NFR-12.4). |
| `manual` | `docs/walkthroughs/` | A written walkthrough per milestone for the criteria marked `manual` (NFR-12.6). |

**Live tests** (real providers or local model servers) are opt-in. Mark them `#[ignore]` and make
them return early unless `TA_LIVE_TESTS=1` is set. Read keys from your own environment, never from
the repository. CI never runs them (NFR-12.5). Run them with:

```sh
TA_LIVE_TESTS=1 cargo test -p <crate> -- --ignored
```

## Pull requests

- One topic per pull request. Name the feature and criterion IDs it implements (e.g. `IN-01.1`).
- `CI result` must pass. It is the only required check.
- When a criterion passes, mark it in the [spec tracker](tools/spec-tracker/README.md).
- New user-visible differences from the legacy app go into the "Behaviour changes" list (D20).
