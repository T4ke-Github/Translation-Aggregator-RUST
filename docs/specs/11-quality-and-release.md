# 11 Quality and release (`NFR`)

## 1. Purpose and scope

This spec covers the qualities that apply to the whole app rather than one area: builds for both
bitnesses, CI quality gates, performance, limits, security and privacy, reliability,
accessibility, localisation of the app's own UI, licensing and the clean-room rule, packaging and
release, documentation, testing strategy and supported platforms.

It **owns the shared numbers**. Other specs reference them by ID (for example "within the budget
in NFR-03.1") and do not restate them. Changing a number here changes it everywhere.
Defaults that only one area uses (for example the LLM context limits in LLM-05 and LLM-06) stay in
that area's spec.

Not covered here: the behaviour of single features (see the area specs). The current CI setup is
described in `.github/CI.md`; this spec states what it must guarantee.

## 2. Goals

1. Every commit builds and passes the same gates on `i686-pc-windows-msvc` and
   `x86_64-pc-windows-msvc` with Rust 1.90 (D1, D2).
2. The app feels instant for local work and never freezes because of the network, disk or a plugin.
3. Secrets and user text stay private: nothing is sent or stored that the user did not ask for.
4. The rewrite stays `MIT OR Apache-2.0` and free of legacy code (D3, D13).
5. A release is a repeatable process with per-bitness packages, licences, docs and a list of
   behaviour changes (D20).

## 3. Features

### NFR-01 Builds for both bitnesses with MSRV 1.90

**Priority:** Must · **Milestone:** M0 · **Decisions:** D1, D2 · **Legacy:** — · **Depends on:** —

As the owner, I want every commit built for 32-bit and 64-bit Windows with the agreed MSRV, so
that both builds stay shippable at all times.

**Acceptance criteria**

- **NFR-01.1** Given any push to `main` or any pull request, when CI runs, then the whole workspace
  builds with `--locked` for `i686-pc-windows-msvc` and `x86_64-pc-windows-msvc`, and the tests of
  both builds run (i686 under WoW64 on the x64 runner). *(Verify: CI)*
- **NFR-01.2** Given the root `Cargo.toml`, when CI runs, then it sets `rust-version = "1.90"` and
  the MSRV job checks the workspace with that toolchain. Raising the MSRV needs a new decision in
  `product-direction.md`. *(Verify: CI)*
- **NFR-01.3** Given the 32-bit and the 64-bit build, when their features are compared, then they
  are the same. The only bitness differences are those in `rust-mapping.md` §0.1 (an optional
  `libmecab` must match the app's bitness; hook DLLs match the target game). *(Verify: manual)*
- **NFR-01.4** Given the repository, when the weekly job runs, then it builds and tests both
  targets on the beta toolchain from the committed `Cargo.lock`, to catch breakage before the next
  stable Rust. *(Verify: CI)*

### NFR-02 CI quality gates

**Priority:** Must · **Milestone:** M0 · **Decisions:** D1, D3 · **Legacy:** — · **Depends on:** NFR-01

As a contributor, I want one clear pass/fail result for every change, so that `main` stays green.

**Acceptance criteria**

- **NFR-02.1** Given a change, when CI runs, then `cargo fmt --all --check` passes.
  *(Verify: CI)*
- **NFR-02.2** Given a change, when CI runs, then
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes on both Windows
  targets. *(Verify: CI)*
- **NFR-02.3** Given a change, when CI runs, then all tests (nextest) and doctests pass on both
  Windows targets. *(Verify: CI)*
- **NFR-02.4** Given a change, when CI runs, then `cargo doc --no-deps --document-private-items`
  passes with `RUSTDOCFLAGS=-D warnings`. *(Verify: CI)*
- **NFR-02.5** Given a change, when `cargo deny check bans licenses sources` fails, then the change
  is blocked. `advisories` warns in `ci.yml` and blocks in the weekly `scheduled.yml` run.
  *(Verify: CI)*
- **NFR-02.6** Given branch protection, when a change is checked, then the `ci-result` job is the
  only required status check; it fails when any job failed or was cancelled, and skipped jobs
  count as passed. Third-party actions are pinned to commit SHAs. *(Verify: CI)*

### NFR-03 Performance

**Priority:** Must · **Milestone:** M1 (NFR-03.3 to NFR-03.5: M2) · **Decisions:** D5, D11 ·
**Legacy:** — · **Depends on:** PIP-01, PIP-04

As a user, I want text and local results to appear at once and the UI to never freeze, so that
reading a game feels immediate.

A **typical PC** for these numbers is: a 4-core x86_64 CPU from 2018 or later, 8 GB RAM, an SSD,
Windows 10 22H2 or Windows 11, one 1920×1080 screen at 100 % or 150 % scale, integrated graphics.
Both the 32-bit and the 64-bit build must meet the numbers on it. Times are measured warm (not the
first start after boot) unless stated otherwise.

**Acceptance criteria**

- **NFR-03.1** Given clipboard or local API text arrives, when it is shown, then it appears in the
  Original Text pane (in M1: the developer console harness) within **100 ms**. "Arrives" is the
  moment the app gets the clipboard notification or the complete API request; a user-configured
  idle delay or merge wait is not counted. *(Verify: integration)*
- **NFR-03.2** Given text of at most **500 characters** and loaded dictionaries, when it is
  processed, then local-only results (MeCab, JParser, substitutions) are shown within **300 ms**.
  *(M1 for substitutions, M4 for MeCab and JParser.)* *(Verify: integration)*
- **NFR-03.3** Given any load, including 10 parallel slow requests and dictionaries loading, when
  the UI runs, then it is never blocked by I/O: no network, disk or plugin call runs on the UI
  thread and no frame stall is longer than **50 ms**. *(M2)* *(Verify: integration)*
- **NFR-03.4** Given a typical PC, when the app is launched, then a usable window (layout restored,
  Original Text pane accepts input) appears within **2 s**. Dictionaries and plugins load in the
  background after that. *(M2)* *(Verify: manual)*
- **NFR-03.5** Given one frame, all official presets enabled and 1 000 history entries, when the
  app has been idle for 1 minute, then its private working set is at most **150 MB** without
  dictionaries loaded. *(M2)* *(Verify: manual)*

### NFR-04 Limits

**Priority:** Must · **Milestone:** M1 (local API limits M5, plugin limits M6) · **Decisions:** D5,
D10, D15, D18 · **Legacy:** `features.md` §3 "500-character cap for automatic web requests" ·
**Depends on:** PIP-04

As a user, I want sensible limits on automatic requests, input and plugins, so that a long
clipboard copy, a flooding tool, a slow provider or a stuck plugin doesn't cost money or block
results.

**Acceptance criteria**

- **NFR-04.1** Given default settings, when an automatic entry is longer than the auto-translate
  length cap of **500 characters** (configurable globally and per provider), then it is not sent
  automatically; the pane says why, and a manual translate still works (PIP-07). *(Verify: unit)*
- **NFR-04.2** Given default settings, when a provider does not answer, then the network timeout
  is **15 s** per request (configurable per provider). For streaming replies it applies to the
  first chunk and to each gap between chunks. *(Verify: contract)*
- **NFR-04.3** Given a request fails with a transient error (timeout, connection error, HTTP 429
  or HTTP 5xx), when it is handled, then it is retried **2 times** with exponential backoff (1 s,
  then 2 s, with jitter; a `Retry-After` value is honoured up to the timeout). Other 4xx replies
  (authentication, validation) are never retried, and a retry is dropped when newer text replaces
  the request. *(Verify: contract)*
- **NFR-04.4** Given default settings, when automatic input (clipboard, local API, plugin) arrives,
  then the maximum input length is **20,000 characters** (setting), counted as Unicode scalar
  values; longer input is not accepted (IN-05.6, API-07.1). *(Verify: unit)*
- **NFR-04.5** Given the local API is on, when a request body is larger than **64 KiB**, then it is
  refused without reading the rest (API-07.1). *(M5)* *(Verify: security)*
- **NFR-04.6** Given default settings, when local API clients send more than **20 requests per
  second** (burst **40**), then the extra requests are refused (API-07.2). The same limit applies
  to `text/push` from input-source plugins (PLG-07.3). *(M5; plugins M6)* *(Verify: security)*
- **NFR-04.7** Given a running plugin, when the app waits for it, then these times apply: the
  `initialize` answer within **10 s** (PLG-04.4); a text-transform answer within **2 s**
  (PLG-06.3); a `ping` every **30 s**, answered within **5 s** (PLG-08.2); a cancelled request
  stopped within **1 s** (PLG-05.4); after `shutdown`, at most **3 s** before the process is
  killed (PLG-08.5). A translator plugin's `translate` call uses the provider timeout of NFR-04.2.
  *(M6)* *(Verify: contract)*
- **NFR-04.8** Given a plugin process exits unexpectedly or is killed as hung, when the app
  restarts it, then the waits between restarts are 1 s, 2 s, 4 s, … up to **60 s**; after **5
  crashes within 10 minutes** it stays stopped until the user restarts it (PLG-08.3, NFR-06.2).
  *(M6)* *(Verify: integration)*
- **NFR-04.9** Given default settings, when results and entries are stored, then the result cache
  holds at most **10,000 results** (PIP-08.4) and the history at most **10,000 entries**
  (PIP-11.5); both sizes are settings. *(Verify: unit)*

### NFR-05 Security and privacy

**Priority:** Must · **Milestone:** M1 (UI marking M2, local API M5) · **Decisions:** D3, D18, D17,
D21 · **Legacy:** — · **Depends on:** CFG-05, NFR-02

As a user, I want my keys and my text to stay private, so that I can use the app with paid
providers and personal clipboard content.

**Acceptance criteria**

- **NFR-05.1** Given a canary key is configured, when the app runs and writes settings files,
  history, exports, logs, crash reports and the provider "Test" view, then no secret (API key,
  token) appears in any of them: a search of every file the app writes and every log line finds
  no match. *(Verify: security)*
- **NFR-05.2** Given a stored secret, when the UI shows it, then input fields are masked, the
  stored value is never shown again, and request details mask secret values. The one exception is
  the local API token, which the app generates and the user can copy to configure a client; on
  screen it shows only its last 4 characters (API-10.1). *(Verify: security)*
- **NFR-05.3** Given a provider whose endpoint is not a loopback address (`localhost`,
  `127.0.0.0/8`, `::1`), when its pane or settings are shown, then it is marked "text leaves this
  machine"; hosts on the local network count as remote. Remote providers start with auto translate
  off (PIP-05.1, *assumes O10 recommendation*). *(M2 for the UI marking.)* *(Verify: integration)*
- **NFR-05.4** Given a fresh install, when the local API is checked, then it is off, binds to
  `127.0.0.1` only, has an optional token, checks `Host` and `Origin`, and enforces the size and
  rate limits NFR-04.5 and NFR-04.6 (details in API-01, API-05, API-06, API-07). *(M5)*
  *(Verify: security)*
- **NFR-05.5** Given a fresh start with no provider enabled, when the app runs, then it makes no
  outbound connection (no telemetry). Logs stay on the machine in the data folder, are capped at
  20 MB in total with rotation, and do not contain source text or translations at the default log
  level. *(Verify: security)*
- **NFR-05.6** Given a release is prepared, when dependencies are audited, then `cargo deny` has
  checked licences, sources, bans and RustSec advisories (NFR-02.5), and every advisory is fixed or
  ignored in `deny.toml` with a written reason. *(Verify: CI)*

### NFR-06 Reliability

**Priority:** Must · **Milestone:** M2 (plugins M6) · **Decisions:** D15, D16, D8 · **Legacy:** — ·
**Depends on:** PIP-09, CFG-03, CFG-04

As a user, I want one broken provider or plugin not to take down the app or lose my settings, so
that I can keep reading.

**Acceptance criteria**

- **NFR-06.1** Given a provider fails or panics while handling a request, when the result is
  shown, then only its pane shows an error; the app and the other panes keep working.
  *(Verify: integration)*
- **NFR-06.2** Given a plugin process crashes or stops answering within its timeout (NFR-04.7),
  when the app notices, then its pane shows "plugin stopped" and the app restarts it with the
  backoff and crash limit of NFR-04.8 (details in PLG-08). *(M6)* *(Verify: integration)*
- **NFR-06.3** Given a settings save, when the app is killed during it, then the settings file is
  never left unreadable: saves are atomic and the last good copy is kept (test and details in
  CFG-03.1 and CFG-03.2). *(Verify: integration)*
- **NFR-06.4** Given the settings file is unreadable at start, when the app starts, then it starts
  from the last good copy (or defaults), tells the user, and keeps the broken file for inspection
  (details in CFG-04). *(Verify: integration)*
- **NFR-06.5** Given the network is offline, when requests fail, then remote panes show "Offline",
  local panes keep working, no dialog is shown per failed request, and the next request after the
  network returns works without a restart. Startup without network meets NFR-03.4.
  *(Verify: integration)*
- **NFR-06.6** Given the app panics, when it ends, then it writes a local crash report without
  secrets or user text, and the next start restores the auto layout (UI-05.3) and offers to open
  the report. *(Verify: integration)*

### NFR-07 Accessibility

**Priority:** Must · **Milestone:** M2 · **Decisions:** D11, D14 · **Legacy:** `ui-reference.md` §6
(no focus indicator, no keyboard pane navigation), §10 item 6 · **Depends on:** UI-01, UI-14

As a user who works with the keyboard or needs larger, clearer text, I want every feature usable
without a mouse and readable in both themes, so that the app works for me.

**Acceptance criteria**

- **NFR-07.1** Given only a keyboard, when the user works through the written walkthrough of every
  menu command, then every feature can be used, including moving panes between frames, resetting
  splitters, pane actions, settings and dictionary tooltips. *(Verify: manual)*
- **NFR-07.2** Given either theme, when an element has focus, then it has a visible focus indicator
  with a contrast of at least 3:1 against its surroundings. *(Verify: manual)*
- **NFR-07.3** Given either theme, when text is shown, then it has a contrast of at least 4.5:1
  (3:1 for large text and control borders), including badges, health indicators and furigana
  highlights, and state is never shown by colour alone. *(Verify: manual)*
- **NFR-07.4** Given 200 % scale on a 1920×1080 screen, when any window or settings page is shown,
  then no control is clipped or overlapped and every settings page is usable (scrolling allowed).
  *(Verify: manual)*
- **NFR-07.5** *(Should)* Given a screen reader, when it reads the app, then controls expose names
  and roles as far as iced allows. Before M8 the support is checked again; any gap is listed in the
  release notes. *(M8)* *(Verify: manual)*
- **NFR-07.6** Given Windows "Show animations" is off, when the app is busy or changes view, then
  busy indicators and transitions do not animate. *(Verify: manual)*

### NFR-08 Localisation of the app's own UI

**Priority:** Should · **Milestone:** M2 · **Decisions:** D11 · **Legacy:** — · **Depends on:** UI-13

As a future translator of the app, I want all UI strings in one place, so that the app can be
offered in more languages later.

**Acceptance criteria**

- **NFR-08.1** Given the UI ships in English, when CI checks the code, then all user-visible
  strings live in resource files, not in UI code, and no string literal shown to the user is found
  outside them. *(Verify: CI)*
- **NFR-08.2** Given the user changes the source or target translation language, when the UI is
  shown, then the UI language does not change. *(Verify: unit)*
- **NFR-08.3** Given a Windows user locale, when numbers, dates and durations are shown, then they
  are formatted with that locale. *(Verify: unit)*
- **NFR-08.4** Given a pseudo-locale that makes every string 40 % longer, when the UI is shown at
  100 % scale, then no label is cut off. *(Verify: manual)*
- **NFR-08.5** *(Could)* Given a new resource file for a further UI language (Japanese first,
  decided after M8), when it is added, then nothing else needs to change. *(Verify: manual)*

### NFR-09 Licensing and clean reimplementation

**Priority:** Must · **Milestone:** M0 (licence bundle M8, dictionaries M4) · **Decisions:** D3, D13 ·
**Legacy:** `risks-and-open-questions.md` §1 · **Depends on:** NFR-02

As the owner, I want the rewrite to be provably free of legacy code and properly licensed, so that
it can be shipped under `MIT OR Apache-2.0`.

**Acceptance criteria**

- **NFR-09.1** Given the workspace, when CI runs, then every workspace crate declares
  `license = "MIT OR Apache-2.0"`, `LICENSE-MIT` and `LICENSE-APACHE` exist at the repository root,
  and `cargo deny` checks the workspace crates too. *(Verify: CI)*
- **NFR-09.2** Given a release package, when it is built, then it contains a third-party licence
  file generated from the resolved dependency tree of that bitness. *(M8)* *(Verify: CI)*
- **NFR-09.3** Given dictionary data (EDICT/JMdict under the EDRDG licence, IPADIC, UniDic), when
  the user downloads or uses it, then its licence is shown before download, the About page and docs
  carry the required attribution, and a bundled dictionary ships with its licence file (JP-02).
  *(M4)* *(Verify: manual)*
- **NFR-09.4** Given the contributor guide, when a contributor reads it, then it states the
  clean-room rule of D13: implementers work from behaviour and format descriptions and black-box
  tests, not from legacy source. *(Verify: manual)*
- **NFR-09.5** Given a pull request that touches a reimplemented feature, when it is reviewed,
  then it passes a checklist: no copied identifiers beyond public format names, no copied tables,
  no copied comments, no legacy data files. *(Verify: manual)*
- **NFR-09.6** Given a shipped file matches the hash of a known legacy asset (`Conjugations.txt`,
  rule sets, toolbar bitmap, icon, sample INI, legacy user docs), when CI runs, then the check
  fails. *(Verify: CI)*

### NFR-10 Packaging and release

**Priority:** Must · **Milestone:** M8 · **Decisions:** D1, D3, D6, D19, D20, D21 · **Legacy:** — ·
**Depends on:** NFR-01, NFR-09, NFR-11

As a user, I want a download for my Windows bitness that just runs, so that I can install the app
without building it.

No release workflow exists yet (`ci.yml` and `scheduled.yml` only check). It has to be written
before M8.

**Acceptance criteria**

- **NFR-10.1** Given a release, when its files are listed, then there is one archive per bitness
  (`…-i686.zip`, `…-x86_64.zip`) with the app, official presets, the disabled unofficial examples
  (D21), user docs and licence files, and the app runs from the unpacked folder without an
  installer. *(Verify: manual)*
- **NFR-10.2** Given a release, when the hook plugin is offered, then it ships as a separate,
  optional package with the hosts and DLLs of both bitnesses (D6, D19). *(Verify: manual)*
- **NFR-10.3** Given a release, when its user docs and release notes are published, then both hold
  the behaviour-changes list (D20). *(Verify: manual)*
- **NFR-10.4** Given a release, when it is published, then a written release checklist is complete:
  all M8 Must criteria pass, CI green on both targets, `cargo deny` clean including advisories,
  MSRV build passes, licences and attribution bundled, SHA-256 checksums published.
  *(Verify: manual)*
- **NFR-10.5** Given a version tag `vX.Y.Z` is pushed, when the release workflow runs, then it
  builds both targets with `--locked` from that tag, runs the full test suite, and attaches the
  archives and checksums to a draft release. *(Verify: CI)*
- **NFR-10.6** *(Could)* Given release binaries, when they are published, then they are
  code-signed to reduce antivirus false positives, mainly for the hook plugin. *(Verify: manual)*

### NFR-11 Documentation

**Priority:** Must · **Milestone:** M8 (API M5, protocol M6, web guide M2) · **Decisions:** D18, D15,
D21, D20 · **Legacy:** — · **Depends on:** API-09, PLG-12, WEB-11

As a user or integrator, I want current docs for the app, its API and its extension points, so
that I can set it up and build on it without reading the code.

**Acceptance criteria**

- **NFR-11.1** Given a milestone changes user-facing behaviour, when it is done, then the user
  guide covering install, first run, panes and frames, layouts, shortcuts, settings, privacy ("text
  leaves this machine", keys) and the behaviour changes is updated. *(Verify: manual)*
- **NFR-11.2** Given a release, when it is published, then the user guide is published on the
  project wiki (or rendered docs site) for it. *(Verify: manual)*
- **NFR-11.3** Given the M5 release, when a reader looks for the local API reference, then it
  exists with a versioned schema and examples (API-09.1). *(M5)* *(Verify: manual)*
- **NFR-11.4** Given the M6 release, when a reader looks for the plugin protocol, then v1 is
  documented with a sample plugin (PLG-12.1, PLG-12.4). *(M6)* *(Verify: manual)*
- **NFR-11.5** Given a web connector feature is added or changed, when it is merged, then
  `docs/guides/custom-web-services.md` is updated and explains the unofficial examples and their
  risks. *(M2)* *(Verify: manual)*
- **NFR-11.6** Given a developer, when they read the developer docs (`.github/CI.md`), then these
  explain building both targets and running the CI checks locally. *(Verify: manual)*

### NFR-12 Testing strategy

**Priority:** Must · **Milestone:** M0 (NFR-12.2 and NFR-12.3: M1, when the first preset and the
first kept features exist; NFR-12.4: M6, when the last part it covers exists) · **Decisions:** D4,
D13, D20 · **Legacy:** `migration-plan.md` "Test summary" · **Depends on:** NFR-02

As the owner, I want each kind of behaviour tested with the right method, so that parity, contracts
and security are checked without depending on live services.

**Acceptance criteria**

- **NFR-12.1** Given a crate with logic, when CI runs, then its unit tests run on both Windows
  targets. *(Verify: CI)*
- **NFR-12.2** Given an official preset or LLM backend, when CI runs, then it has contract tests
  against recorded responses or a local mock server, including timeouts, errors and rate limits,
  and CI makes no call to a real provider. *(Verify: contract)*
- **NFR-12.3** Given a kept feature (substitutions, context filters, kana conversion, layout and
  `UserDict.txt` import, JParser, MeCab, hook codes), when its parity tests run against the Phase-0
  fixtures of the legacy app, then they pass; intended divergences (D20) are marked in the test and
  linked to the behaviour-changes list. *(Verify: parity)*
- **NFR-12.4** Given the security test suite, when it runs, then it covers the secret canary
  (NFR-05.1), local API binding, `Host`/`Origin` and token checks, size limits and floods, and
  plugin crash isolation. *(Verify: security)*
- **NFR-12.5** Given live tests against real providers or local model servers, when CI runs, then
  they are skipped; they are opt-in and never run in public CI with project secrets.
  *(Verify: CI)*
- **NFR-12.6** Given a milestone with UI criteria, when it is checked, then UI behaviour is covered
  by headless state tests (layouts, commands, pane states) and by written manual walkthroughs for
  the `manual` criteria. *(Verify: manual)*

### NFR-13 Supported platforms and rendering

**Priority:** Must · **Milestone:** M2 · **Decisions:** D1, D2, D14 · **Legacy:** — ·
**Depends on:** NFR-01

As a user with older or virtual hardware, I want the app to start and draw correctly, so that I can
use it on my machine.

**Acceptance criteria**

- **NFR-13.1** Given Windows 10 or Windows 11, 64-bit or 32-bit (the minimum of the Rust Windows
  targets), when the app is installed, then it is supported; given an older Windows version, then
  it is not supported, and the docs say so. *(Verify: manual)*
- **NFR-13.2** Given the 32-bit build, when it runs on 64-bit Windows or on 32-bit Windows 10, then
  it works. *(Verify: manual)*
- **NFR-13.3** Given no usable GPU (e.g. a VM or remote desktop), when the app starts, then it falls
  back to the software renderer by itself and still meets NFR-03.4 and NFR-03.3 on a typical PC.
  *(Verify: manual)*
- **NFR-13.4** Given the app is started from any working directory, when it loads its settings,
  then it uses the fixed data folder (or the folder next to the exe in portable mode), never a path
  relative to the working directory (D20, CFG-01.3). *(Verify: integration)*

## 4. Out of scope

| Item | Reason |
|---|---|
| Installer (MSI/MSIX) and auto-update | Not decided; release is per-bitness archives (NFR-10.1) |
| Telemetry or crash upload | No telemetry (NFR-05.5) |
| Linux or macOS builds of the app | Windows-only app (D1); the optional Linux CI job covers portable crates only |
| Shipping legacy data files or docs | Clean reimplementation (D13) |
| Support for legacy plugin DLLs | D15 |

## 5. Open questions

| # | Effect on this spec | Assumption used |
|---|---|---|
| O10 | Auto-translate default for remote providers (NFR-05.3) | Off |
| O11 | Bundled dictionaries and their licence files (NFR-09.3, NFR-10.1) | Download on first use; bundle only with licence and attribution |
| O12 | Token for the local API output stream (NFR-05.4) | Required when the stream is on |
| New | Code signing of release binaries (NFR-10.6) | Could; owner to decide before M8 |
| New | Installer and update mechanism | None for the first release (archives only) |

## 6. Traceability

| Feature | Priority | Milestone | Decisions |
|---|---|---|---|
| NFR-01 Builds for both bitnesses with MSRV 1.90 | Must | M0 | D1, D2 |
| NFR-02 CI quality gates | Must | M0 | D1, D3 |
| NFR-03 Performance | Must | M1 (NFR-03.2: M4 for MeCab/JParser; NFR-03.3 to NFR-03.5: M2) | D5, D11 |
| NFR-04 Limits | Must | M1 (local API M5, plugins M6) | D5, D10, D15, D18 |
| NFR-05 Security and privacy | Must | M1 (UI marking M2, local API M5) | D3, D18, D17, D21 |
| NFR-06 Reliability | Must | M2 (plugins M6) | D15, D16, D8 |
| NFR-07 Accessibility | Must (screen readers Should) | M2 (screen-reader check M8) | D11, D14 |
| NFR-08 Localisation of the app's own UI | Should | M2 | D11 |
| NFR-09 Licensing and clean reimplementation | Must | M0 (licence bundle M8, dictionaries M4) | D3, D13 |
| NFR-10 Packaging and release | Must | M8 | D1, D3, D6, D19, D20, D21 |
| NFR-11 Documentation | Must | M8 (API M5, protocol M6, web guide M2) | D18, D15, D21, D20 |
| NFR-12 Testing strategy | Must | M0 | D4, D13, D20 |
| NFR-13 Supported platforms and rendering | Must | M2 | D1, D2, D14 |
