# Migration plan

Phased plan for the rewrite, following `../product-direction.md`. Order: core + clipboard + one web preset → GUI shell → LLM plugin → MeCab/JParser → local API → plugin protocol → hook plugin → packaging. Each phase ends with something usable and names what can be tested in isolation.

Ground rules:
- **Clean reimplementation (D13).** Every component is reimplemented from behaviour and file formats (`architecture.md`, `features.md`, `formats-and-interfaces.md`, `ui-reference.md`). No legacy source is copied or translated, and no legacy-authored data file is shipped. Developers working on a component use the behaviour/format descriptions and black-box observations, not the C++ files.
- **Black-box parity, only where behaviour is kept.** Parity tests feed the same input to the legacy app and the rewrite and compare outputs. They exist for kept features (substitutions, context filters, kana conversion, JParser, MeCab readings, layout and `UserDict.txt` import, hook codes and captured text). Replaced or new features (web connector, LLM, local API, plugin protocol, modernized UI) get contract and fixture tests instead.
- **Quirks are fixed (D20).** Where the legacy output is a bug (`risks-and-open-questions.md` §6), the parity test records the intended divergence, and the change goes into a "behaviour changes" list in the user docs.
- Both bitnesses are built and tested from Phase 0 on (D1); MSRV 1.90 (D2); `cargo deny` gates every phase (D3).

## Phase 0 — Scaffolding and guardrails
- Workspace skeleton (`workspace-layout.md`), `rust-version = "1.90"`, CI matrix for i686 and x86_64, `cargo deny check` (license policy from D3).
- Clean-room working rule written down (who may read legacy code for documentation vs. who writes Rust code; see risks §1).
- **Capture black-box fixtures from the running legacy app** while it still works: input text and resulting output for substitutions (with sample `UserDict.txt`), context filters (repeat/phrase/line-break modes), kana/romaji conversions (Ctrl-H/K/R), JParser output for a corpus (with a user-supplied EDICT), MeCab pane output (with a known dictionary), legacy INI files with the resulting window layout (screenshots + parsed values), hook codes accepted/rejected by "Check codes" and text captured from a test program.
- Exit: CI green on both targets with empty crates; fixture set stored with a note of how each was captured.

## Phase 1 — Core, clipboard and one web preset
Crates: `ta-text`, `ta-core` (pipeline, history, config), `ta-substitute`, `ta-translate`, `ta-provider-web`, `ta-platform` (clipboard listener, credential store), `ta-ingest` (clipboard part).
- Pipeline: contexts, idle flush, filters, dedupe, length limits, substitutions.
- Router with cache, rate limit, retries, cancel/replace.
- Web connector with the TOML definition format and **one preset** (LibreTranslate is easiest to test against a self-hosted instance; DeepL API next). Keys from the OS credential store.
- A developer-only console harness (not shipped) to watch clipboard → translation end to end.
- Testable in isolation: everything except the clipboard listener; connector tests replay recorded HTTP responses from a local mock server.
- Black-box parity: substitutions, context filters, kana conversions against Phase-0 fixtures. For substitutions, the fixture `UserDict.txt` files are read by a test-only loader in `ta-substitute`; the user-facing import (with report and UI) comes in Phase 2.
- Exit: copying Japanese text produces a translation from the preset on both bitnesses.

## Phase 2 — GUI shell
Crates: `ta-gui` (iced, D14), more presets.
- Precondition: the UI design step has agreed the concrete look, using `ui-reference.md` as input.
- Original Text pane + one pane per translator, multiple frames, one/two columns with splitters, layouts (save/restore, Alt-0..9), transparency, always on top, hide frame/toolbars, history back/forward, keyboard shortcuts, settings UI (providers, keys, languages, fonts, themes).
- One-time import of legacy INI and `UserDict.txt`, then TOML (D16).
- Remaining first presets: Google Cloud Translation, Microsoft Translator, DeepL API (D17).
- Connector features needed by D21 (session step, cookie reuse, token refresh, form bodies, JSONPath with joining), the provider "Test" view, and the disabled keyless Google/Bing example definitions with their guide.
- Testable in isolation: layout/config load-save round trips, import mapping; UI state logic headless.
- Black-box parity: a legacy INI with `[Layout N]` lines imports to the same frames, pane order, columns and splitter positions (within rounding); `UserDict.txt` import gives the same substitution results.
- Exit: daily use with clipboard input and the four official web presets.

## Phase 3 — LLM translator
Crate: `ta-provider-llm` (compiled in as a feature at this stage; moves out of process in Phase 6).
- OpenAI-compatible Chat Completions (incl. local servers) and Anthropic Messages API; prompt template, glossary from substitutions, previous-lines context, streaming into the pane, token usage and optional user-entered cost.
- Testable in isolation: request building and SSE parsing against recorded streams; a local mock server for timeouts, errors and rate limits.
- No parity tests (new feature). Live tests are opt-in and need user keys or a local model server.
- Exit: streaming LLM translations next to the web panes, with clear "text leaves this machine" indication for remote backends.

## Phase 4 — MeCab and JParser
Crates: `ta-morph` (`vibrato` default, optional `lindera`/libmecab), `ta-dict`; furigana and tooltip views in `ta-gui`.
- Dictionary download/selection in settings, with licence display and attribution.
- Self-authored conjugation rules (D13); EDICT loading and cache.
- Testable in isolation: tokenizer adapter, de-inflection, matching, cache invalidation.
- Black-box parity: MeCab readings and segmentation vs. the legacy MeCab pane **with the same dictionary**; JParser segmentation and chosen entries vs. the Phase-0 corpus. Divergences caused by the new conjugation data are reviewed and listed, not forced to match.
- Exit: MeCab and JParser panes with furigana and hover definitions.

## Phase 5 — Local input/output API
Crate: `ta-ingest` (server part).
- `POST /v1/text`, `GET /v1/status`, WebSocket/SSE stream of incoming text and translations; off by default, loopback only, optional token (D18); versioned schema document.
- Testable in isolation: entirely (in-process client tests).
- Security tests: non-loopback bind refused, wrong `Host`/`Origin` rejected, token checks, size limits, behaviour under floods.
- Exit: an external script or overlay can send text and receive translations.

## Phase 6 — Plugin protocol
Crate: `ta-plugin-protocol`.
- JSON-RPC over stdio (and optionally named pipe), handshake with protocol version and capabilities, manifests, supervision (start, health, restart, stop).
- Move the LLM translator into a plugin process (keep the in-process feature as an option, using only protocol-level functionality).
- A small sample text-transform plugin (replaces the legacy `__stdcall` sample, D15).
- Testable in isolation: protocol conformance tests with a fake plugin; version-mismatch handling; crash and restart.
- Exit: LLM plugin runs out of process on both bitnesses; protocol v1 documented.

## Phase 7 — Hook plugin
Location: `plugins/hook/` (D19). Crates: `ta-hookcode`, `ta-hook-engine`, `ta-hook-dll`, `ta-hook-host`.
- Hook-code parser/evaluator (32-bit syntax from `docs/Hook Codes.txt`), then the x86 engine and DLL, then x86_64 and the 64-bit hook-code extension; launch profiles; AGTH-code converter; text sent to the app via the local API with a context id.
- Testable in isolation: hook-code parsing/evaluation; instruction relocation against crafted byte sequences; capture against small purpose-built 32- and 64-bit test programs.
- Black-box parity: the same hook codes on the same test program (or game) capture the same text as the legacy DLL (32-bit). The 64-bit side has no legacy oracle.
- Test matrix: {32-bit app, 64-bit app} × {32-bit target, 64-bit target}.
- Exit: hook plugin package for both bitnesses, optional install.

## Phase 8 — Packaging and release
- Per-bitness app archives, hook plugin package, presets, docs, third-party licence file, behaviour-changes list (D20).
- Release checklist: `cargo deny` clean, MSRV build on both targets, licence/attribution of any bundled data, decision on the rewrite's own release licence files (`MIT OR Apache-2.0`, D3).

## Test summary
| Area | Kept behaviour? | Oracle | Method |
|---|---|---|---|
| Substitutions, context filters, kana conversion | yes | legacy app (Phase-0 fixtures) | black-box golden I/O |
| INI layout / `UserDict.txt` import | yes (import) | legacy files + observed layout | import → compare state |
| JParser | yes, with new conjugation data | legacy app corpus | black-box, reviewed divergences |
| MeCab | yes | legacy MeCab pane, same dictionary | black-box readings/segmentation |
| Hook codes and captured text (32-bit) | yes | legacy DLL on a test program | same codes → same text |
| Web connector | replaced | provider docs + recorded responses | contract/fixture tests, opt-in live |
| LLM plugin | new | recorded streams, mock server | contract tests, opt-in live |
| Local API, plugin protocol | new | spec | conformance and security tests |
| UI | modernized | agreed UI design | manual walkthrough, headless state tests |

## Sequencing rationale
- Phase 1 delivers the main use case (D5) with the least risk: pure logic, one official API, no GUI toolkit risk.
- The GUI follows as soon as the core works, after the UI design step, so D11 is settled before screens are built.
- LLM and MeCab/JParser add the most user value next and are independent of the plugin machinery.
- The local API comes before the plugin protocol because the hook plugin uses it for input; the protocol then hosts the LLM plugin out of process.
- The hook plugin is last: it is the only heavily `unsafe`, per-architecture part, it is optional for the main use case (external hookers already work through the clipboard), and it depends on the local API.
