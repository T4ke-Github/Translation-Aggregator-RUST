# Project handover

The Rust rewrite is in M1, not at the M1 exit gate. M0 scaffolding and several M1 slices are merged into `main`. CI builds and tests both Windows MSVC widths, but a passing CI run does not prove every acceptance criterion. Use the [project plan](project-plan.md), [spec index](specs/README.md), and [product direction](product-direction.md) as the source of work and decisions.

## M1 checkpoint on 2026-11-21

- The clipboard console loads configured web providers and runs one active request plus one queued
  request per provider. Fast results appear independently of slow providers. `translate-all`
  reaches enabled manual-only providers even while capture is paused. The console and retained
  history record provider ID, outcome, elapsed time, and cache status.
- The clipboard and typed-input paths load saved global, exact-context, and wildcard substitution
  profiles from `settings.toml`. The explicit keyless LibreTranslate override still uses its
  command-line rules. See the [M1 walkthrough](walkthroughs/M1-core.md) for setup and criterion-level evidence.
- Both Windows MSVC builds passed 170 workspace tests, strict Clippy, all seven native clipboard
  smoke cases, three-provider routing with a slow provider timing out, and live Japanese-to-English
  translation against LibreTranslate 1.9.6 on loopback. The Rust 1.90 workspace check passed.
- Both builds passed the disposable Credential Manager and history checks. The [legacy fixtures](../fixtures/legacy/README.md)
  contain 22 captured cases with automated replay and documented behavior differences.
- The owner approved the previous development PC for M1 performance sign-off. Clipboard latency
  upper bounds were 0.78 ms on 64-bit and 0.87 ms on 32-bit against the 100 ms budget. Warm
  substitutions took 13.71 ms and 19.16 ms against the 300 ms budget.

## Resume on another Windows PC

This checkpoint builds on `95f9a1f`. On the next PC, open the `RUST-Rewrite` repository and run
`git status --short --branch` to check for local edits. Fetch `origin/main` and fast-forward local
`main` once the M1 checkpoint PR is merged. The checkpoint includes the console source, parity
tests, and both legacy fixture folders.

Install Visual Studio Build Tools with **Desktop development with C++**, the MSVC x64/x86 tools,
and a Windows SDK. Run the following from the `RUST-Rewrite` root in PowerShell. The explicit
toolchain leaves the machine's Rust default alone.

```powershell
rustup toolchain install stable-x86_64-pc-windows-msvc
rustup target add --toolchain stable-x86_64-pc-windows-msvc i686-pc-windows-msvc
rustup component add --toolchain stable-x86_64-pc-windows-msvc rustfmt clippy

foreach ($target in 'x86_64-pc-windows-msvc', 'i686-pc-windows-msvc') {
    cargo +stable-x86_64-pc-windows-msvc build --locked -p ta-gui --target $target
    cargo +stable-x86_64-pc-windows-msvc test --workspace --locked --target $target
}
```

The previous builds, Python mocks, and LibreTranslate server used temporary folders on the other
PC. Recreate the loopback mock or server from the [M1 walkthrough](walkthroughs/M1-core.md) if
another native run is needed. Use a disposable portable profile for clipboard, Credential Manager,
and history checks.

## Next M1 work

1. Run the [off-device consent walkthrough](walkthroughs/M1-core.md#automatic-off-device-translation)
   on Windows for PIP-05.2. The console asks before the first automatic request to a fixed remote
   HTTPS origin. Approval is stored per provider and origin. Remote providers still default to
   manual translation under O10.
2. Continue from the [M1 acceptance audit](m1-acceptance-audit.md). Reconcile its evidence with the
   private [spec tracker](../tools/spec-tracker/README.md) once an editable tracker or export is
   available. Record passes only when the specified verification method has been met.

## Later gates

M2 covers the GUI and legacy import. The [UI design draft](ui-design.md) is ready for review and
lists the pending Windows spikes. Screen work requires owner approval under UI-01. M3 adds LLM
backends, M4 adds Japanese tools, M5 adds the local API, and M6 adds plugins. M7 cannot start hook
code until its placeholder spec gains approved criteria and safety rules. M8 needs release builds,
licences, docs, and final checks for both Windows widths.

## Work rules and repo state

Use one orchestrator and two subagents for each milestone, with exclusive file areas. Write a criterion-level failing test before code, then make it pass and rerun the linked checks. Keep the clean-room split in [CONTRIBUTING.md](../CONTRIBUTING.md); never implement a feature from legacy C++ source. A helper test is not proof that the app calls the helper.

Review the transferred changes and preserve any pre-existing local edits before branching.
