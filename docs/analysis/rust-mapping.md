# Rust mapping

How each component of the rewrite is built in Rust, and which crates it uses. This document follows `../product-direction.md` (authoritative). Crate choices are recommendations; nothing here is a committed dependency, and no Cargo files exist yet.

**Clean reimplementation (D13).** The legacy C++ code is GPLv2 and the rewrite is `MIT OR Apache-2.0`, so nothing below means "port this file". Every component is **reimplemented from behaviour and file formats** as described in `architecture.md`, `features.md` and `formats-and-interfaces.md`. Parity is checked with black-box input/output tests (`migration-plan.md`), never by comparing code.

Verification of the crates named here (license, MSRV, how it was checked) is in §14.

## 0. Cross-cutting decisions

### 0.1 Bitness (D1)
The whole app ships as `i686-pc-windows-msvc` and `x86_64-pc-windows-msvc`. With ATLAS and LEC cut (D7) and MeCab handled by a pure-Rust analyzer (D9), **the core has no 32-bit-only part any more** and no engine-host helper. Both builds are the same code.

Bitness still matters in two places:

| Piece | Rule |
|---|---|
| Optional `libmecab.dll` backend (FFI) | Only a DLL of the **same bitness as the app** can be loaded. The 64-bit app needs a 64-bit MeCab install; otherwise the pure-Rust backend is used. |
| Hook plugin (`plugins/hook/`) | A hook DLL must match the **target game's** bitness. The plugin builds one hook DLL and one host per bitness. It talks to the app through the local input API, so a 32-bit host can feed a 64-bit app and the other way round. |

| Binary | i686 package | x86_64 package |
|---|---|---|
| `ta-gui` (the app) | i686 | x86_64 |
| First-party LLM plugin (if not compiled in) | i686 | x86_64 |
| Hook plugin hosts + DLLs | both bitnesses (optional component) | both bitnesses (optional component) |

### 0.2 MSRV 1.90 (D2)
- Set `rust-version = "1.90"` on every crate and use the MSRV-aware resolver (edition 2024 default). In the checks below this resolver picked MSRV-compatible versions on its own (e.g. `eframe` fell back to 0.33.3).
- Crates whose **newest** release needs more than 1.90 (checked 2026-11-08): `egui`/`eframe` 0.36+ (1.95), `egui_dock`/`egui_tiles` (1.95), the `windows-*` 0.100 sub-crates (`windows-core`, `windows-strings`, `windows-registry`, `windows-version`: 1.95), `rfd` 0.18 (1.92). Stay on `windows` 0.62 / `windows-sys` 0.61 until the MSRV moves. `muda` 0.21 and `tray-icon` 0.26 need exactly 1.90.
- No recommended crate needs nightly. `retour`'s `static-detour` and `thiscall-abi` features need nightly and are not used.

### 0.3 License policy (D3)
- The rewrite is `MIT OR Apache-2.0`. Dependencies must be permissive (MIT, Apache-2.0, BSD, ISC, Zlib, BSL-1.0, MPL-2.0, …); GPL/LGPL/AGPL crates are not allowed. Enforcement is `deny.toml` (maintained separately; not changed by this document).
- The resolved trees of every recommended stack (core, iced GUI, hook plugin) were checked for both Windows targets: **no GPL/LGPL crate was found**.
- Things that would need attention under `deny.toml`:
  - `lindera` with an `embed-*` feature pulls `webpki-roots` (`CDLA-Permissive-2.0`) through a build-time downloader. Use `lindera` without `embed-*`, or prefer `vibrato` (§5).
  - `egui`'s default fonts add `OFL-1.1` and `Ubuntu-font-1.0` (`epaint_default_fonts`). Not relevant now that `iced` is chosen (D14).
  - `native-windows-gui` depends on `stretch` 0.3.2, which has **no license metadata**. Rejected.
- `slint` stays excluded (GPLv3/commercial).
- Data is separate from crates: dictionaries (EDICT, IPADIC, UniDic) have their own licences and attribution duties (§5, §6).

### 0.4 Win32 bindings
- `windows` 0.62 for the GUI-side platform code (clipboard listener window, hotkeys, credential store if not using `keyring`, known folders, layered-window alpha).
- `windows-sys` 0.61 where code is handle-and-pointer shaped (hook plugin, optional MeCab FFI). Features follow header-group module names; enable only what is used.

### 0.5 Strings
Domain logic uses UTF-8 `String`/`&str`; UTF-16 conversion only at Win32 boundaries (`windows` string helpers or `widestring`). The legacy locale-invariant comparison and kana/width/case folding (`Shared/Shrink.h unify()`) are behaviour to re-specify and test, not to copy (§7).

## 1. Inputs (`ta-ingest`)

### 1.1 Clipboard (D5)
- A message-only window on its own thread registers `AddClipboardFormatListener` and reacts to `WM_CLIPBOARDUPDATE` (`windows`, features `Win32_System_DataExchange`, `Win32_UI_WindowsAndMessaging`). This replaces the legacy viewer chain (`architecture.md` §2.2).
- Read text with `arboard` (or `clipboard-win`); neither offers a change notification on Windows in the checked docs, so the listener stays our own code.
- Ignore changes the app made itself (the legacy app re-posts its own copies as plain text, `TranslationAggregator.cpp:1358-1377`); compare the clipboard owner or the sequence number.
- Feeds the pipeline as context `clipboard` with the per-context options (auto translate, Japanese only, merge, repeat filters, idle delay).

### 1.2 Local input/output API (D5, D18)
- `axum` 0.8 on `tokio`. Bound to `127.0.0.1` only; **off by default**; optional token (D18).
- Surface (Proposal): `POST /v1/text`, `GET /v1/status`, a WebSocket (`axum` `ws`, built on `tokio-tungstenite`) and/or SSE stream of incoming text and translations.
- Hardening even on loopback: check the `Host` header against loopback names (DNS rebinding), reject cross-origin browser requests unless allowed (`Origin`), constant-time token compare, request size and rate limits, no CORS by default. See `risks-and-open-questions.md` §4.
- The schema is a hand-written, versioned document under `/v1`; `schemars` (MIT) can generate JSON Schema from the request types if wanted.

### 1.3 Pipeline (part of `ta-core`/`ta-ingest`)
Context buffering with idle flush, text-loop detection, repeat filters, merge, Japanese-only, length caps, dedupe; then substitutions. Reimplemented from the behaviour in `architecture.md` §2.2 and the Context Manager options (`features.md` §3). The 500-character auto cap becomes a per-provider limit in the router.

## 2. Translator core (`ta-translate`)
- `Translator` trait (Proposal): `translate(text, src, dst, context) -> stream of result`, plus capabilities (languages, max length, streaming, cost hint).
- Router: fan-out to enabled translators; result cache keyed by provider, settings, languages and text; per-provider rate limit (token bucket; `governor` MIT is an option); retries with backoff; cancel/replace an in-flight request when newer text arrives (`tokio-util` `CancellationToken`).
- History stores results per entry and provider (`ta-core`), searchable (D11).
- HTTP client: **`reqwest` 0.13 with `default-features = false` and `native-tls`** (Windows SChannel via `schannel`), plus `json`, `stream`, `gzip`, `http2`, `system-proxy`, and `cookies`, `form`, `query` for the web connector (§3). Reason: reqwest 0.13's default TLS is rustls with `aws-lc-rs`, whose `aws-lc-sys` needs a native C build for both targets; SChannel uses the OS certificate store and needs no extra toolchain. The rustls/aws-lc stack passes the new license policy too, so it remains a fallback.

## 3. Web connector (`ta-provider-web`, D10, D21)
The definition format is specified in `../guides/custom-web-services.md` (owner draft); this section only covers how to build it.

- A provider is a **TOML definition** (`serde` + `toml`): `[provider]` (id, name, enabled, unofficial badge, auto-translate, max chars, rate limit, timeout, website), `[languages]` (auto code, code map, supported list), optional `[session]`, `[request]`, `[response]`. Users add, import or edit definitions in the settings UI; no rebuild. A **Test** action shows the raw request and reply, with secrets masked.
- **Placeholders** `{{text}}`, `{{src}}`, `{{dst}}`, `{{counter}}`, `{{session.name}}`, `{{secret.name}}`: a small own substitution engine, not a general template language. Escaping depends on where the placeholder sits: URL-encoded in `url`, a JSON string value in `body.json` (the body is built as a `serde_json::Value`, never by string concatenation), form-encoded in `body.form` (`reqwest` `form`/`query` features, `serde_urlencoded`). This rules out injection through translated text. (`minijinja`/`upon` are not needed for this format.)
- **Secrets:** `{{secret.name}}` is resolved from the OS credential store (§9) at send time; never stored in the TOML file, never logged, never shown in Test.
- **Response extraction:** **JSONPath** (RFC 9535) via `serde_json_path` (MIT) with optional `join` of several matches (sentence-split replies); `regex` first capture group for HTML/text replies; optional `error` extraction shown in the pane; optional `html_decode` (`html-escape`). `jsonptr` is not needed.
- **Session step (D21):** optional pre-request that fetches a page, extracts values with regexes into `{{session.*}}`, keeps cookies (`reqwest` `cookies` feature → `cookie_store`), and is re-run after N uses, after a number of seconds, on given HTTP statuses, or when the reply matches `refresh_session_if` (then the request is retried once). Session state lives in memory per provider; nothing from it is logged.
- **Rate limits:** per-definition `rate_limit` enforced by the router (§2), plus the global per-provider budgets (risks §2).
- **Presets (D17):** DeepL API, Google Cloud Translation, Microsoft Translator (the service behind Bing), LibreTranslate (self-hosted or hosted URL). Each preset is checked against the provider's current official documentation when it is written. Auth modes that need real logic (OAuth service accounts, request signing, computed tokens) are translator plugins, not definitions.
- **Examples (D21):** keyless Google and Bing web versions as `examples/providers/google-web-unofficial.toml` and `examples/providers/bing-web-unofficial.toml`, disabled by default and marked unofficial in the UI, documented in `docs/guides/custom-web-services.md`. Written fresh against the current endpoints (D13), expected to break from time to time; they are also the acceptance test that the session step, cookies, form bodies and JSONPath joining are expressive enough.
- Errors are mapped to clear pane states: auth missing/invalid, quota, rate limited, session refresh failed, network, bad definition.

## 4. LLM translator plugin (`ta-provider-llm`, D8)
- First-party plugin over the plugin protocol (§10); may also be compiled into the app as an optional feature, using only what the protocol offers.
- Backends: **OpenAI-compatible Chat Completions** (covers many hosted services and local servers such as Ollama, LM Studio, llama.cpp server) and the **Anthropic Messages API**. Requests and responses are small `serde` types written for this crate; streaming via SSE (`eventsource-stream` or a small parser over `reqwest`'s byte stream).
- Settings (all user settings, no hard-coded model names or defaults): endpoint, model, temperature, max output tokens, system/user prompt template with source/target language and tone, optional glossary from the substitution/user dictionary, optional previous N lines of the same context, timeout, streaming on/off.
- Shows token usage from the provider's usage fields; cost only if the user enters prices (prices change, so none are built in).
- Privacy: every remote LLM backend is opt-in per provider, and the UI makes clear that text leaves the machine; local servers are the private option (see risks).

## 5. MeCab (`ta-morph`, D9)

**Proposal refined after verification** (also updated in `product-direction.md`): the default backend is **`vibrato`**; `lindera` is the fallback and only with external dictionary files.

- **`vibrato`** 0.5.3 (`MIT OR Apache-2.0`, MSRV 1.88): pure-Rust MeCab-compatible tokenizer. It reads a compiled dictionary file; prebuilt dictionaries (e.g. from mecab-ipadic 2.7.0) are published on its releases page, and it can compile system dictionaries from MeCab-format sources. Output is MeCab-style feature strings, so readings for furigana and POS for colouring map directly. Clean dependency tree.
- **`lindera`** 6.2.0 (MIT, MSRV 1.88): use `load_dictionary(path)` with a downloaded dictionary and **no `embed-*` features**. The `embed-*` features download dictionary archives at build time (network access in CI, reproducibility) and pull `webpki-roots` (`CDLA-Permissive-2.0`).
- **Optional libmecab backend:** `libloading` 0.9 (ISC) loads an installed `libmecab.dll` of the app's bitness; discovery via registry (`winreg` 0.56, MIT) and known folders. MeCab is not thread-safe: serialize calls.
- **Dictionaries** (data licences, not crate licences): IPADIC (NAIST/ICOT licence: permissive, keep the copyright and licence notice; not an SPDX id), UniDic 2.1.2 (offered under GPL, LGPL **or BSD-3-Clause**; use it under BSD-3-Clause). Newer UniDic releases: licence not verified here. Default: the user downloads a dictionary from the settings UI; if one is bundled, its licence file and attribution ship with it.

## 6. JParser and dictionaries (`ta-dict`, D12)
- Reimplement EDICT/EDICT2/ENAMDICT reading from the format description (`formats-and-interfaces.md` §7): `encoding_rs` for EUC-JP/Shift_JIS/UTF-16 with BOM sniffing, `flate2` for `.gz`.
- Matching, de-inflection and scoring are re-specified from observed behaviour (golden I/O cases), not from `Dictionary.cpp`.
- **Conjugation rules are self-authored** (D13) from grammar references, stored in the crate's own format; the legacy `Conjugations.txt` is not shipped.
- Cache: own versioned, memory-mapped format (`memmap2`) or `postcard`/`rkyv`; rebuilt when the source changes.
- EDICT/JMdict data comes from the EDRDG under its own licence (CC BY-SA per the EDRDG licence statement; not re-verified in this pass). The user downloads it, or it ships with licence text and attribution.

## 7. Text utilities (`ta-text`)
- Encodings: `encoding_rs` (Shift_JIS, EUC-JP, UTF-16LE/BE).
- Kana/romaji conversion, half-width→full-width katakana, the folding used for matching: own tables, specified by tests. `wana_kana` (MIT) is an option for romaji; `unicode-normalization` NFKC is broader than the legacy half-width conversion, so it is not a drop-in.
- HTML entities: `html-escape`.

## 8. Substitutions (`ta-substitute`)
- Profiles (global `*` plus per-context/per-game), longest match, escapes; `aho-corasick` (`Unlicense OR MIT`) with leftmost-longest matching is a good fit.
- Storage TOML; one-time `UserDict.txt` import (UTF-16LE, format in `formats-and-interfaces.md` §3) (D16).

## 9. Config, credentials, paths (`ta-core`, `ta-platform`)
- Settings: `serde` + `toml`, versioned. One-time import of the legacy INI (UTF-16 aware; `rust-ini` MIT or a small parser) including `[Layout N]` lines and `WritePrivateProfileStruct` hex blobs where useful (D16).
- Data directory: a defined location (e.g. `%APPDATA%` via `SHGetKnownFolderPath` in `windows`, or `directories`), plus an optional portable mode next to the exe. Never CWD-relative (legacy bug, `architecture.md` §6).
- **Credential store:** `keyring` 4.2 with `default-features = false` and `windows-native-keyring-store` → Windows Credential Manager (checked in the keyring docs via Context7). Alternative with fewer deps: `CredWriteW`/`CredReadW` through `windows` (`Win32_Security_Credentials`). Keep secrets out of logs and crash reports; `zeroize`/`secrecy` for in-memory handling.

## 10. Plugin protocol (`ta-plugin-protocol`, D15)
- Out-of-process only. JSON-RPC 2.0 messages (`serde_json`) over **stdio** of a child process; optionally a **named pipe** (`tokio::net::windows::named_pipe`, built into tokio). `jsonrpsee` (MIT) or `interprocess` (`0BSD OR Apache-2.0`) are options, not required.
- Handshake: `initialize` exchanges protocol version (major.minor), plugin kind (input source / translator / text transform), capabilities and plugin version; the host rejects an unsupported major version with a clear message.
- Plugin manifest (TOML) next to the executable: id, name, kind, executable per bitness, supported protocol range.
- Lifecycle: start on demand, health check, restart with backoff, kill on shutdown; crashes stay in the plugin process.
- Input-source plugins may use the local input API instead of stdio.

## 11. GUI (`ta-gui`, D11, D14)
- **`iced` 0.14** (MIT), styled like classic Win32. Look and behaviour of the legacy UI, gaps in iced, and their mitigations: `ui-reference.md`.
- Building blocks: `iced::daemon` for multiple top-level frames; `pane_grid` for splits, resize and drag inside a frame; a custom `Theme`/palette and per-widget style functions for the classic look plus light/dark; `window::Level::AlwaysOnTop`; custom widgets for the furigana view and dictionary tooltips.
- Platform glue via the raw window handle and `windows`: whole-window alpha (`SetLayeredWindowAttributes`), global hotkeys (`RegisterHotKey`, or `global-hotkey`), optional native menu bar (`muda`, MSRV 1.90). How well each attaches to iced's windows is **unverified** and is a spike in the UI design step.
- Renderer: wgpu by default; the `tiny-skia` software renderer as a fallback for machines without a usable GPU (both resolve for i686; a real i686 run is unverified).

## 12. Hook plugin (`plugins/hook/`, D6, D19) — architecture level
- **`ta-hookcode`:** parser and expression evaluator for the hook-code language, reimplemented from `docs/Hook Codes.txt` (D13); bitness-generic AST; the 64-bit extension is designed here. Pure Rust, testable anywhere.
- **`ta-hook-engine`:** per-architecture inline-hook backends (x86 and x86_64). `iced-x86` decodes and re-encodes relocated instructions; `retour` (stable `GenericDetour`/`RawDetour` only) covers detours of exported text APIs. A small architecture-specific stub captures registers for user hook codes. This is the most `unsafe` code in the project and stays isolated in this crate.
- **`ta-hook-dll`** (cdylib, built per bitness) and **`ta-hook-host`** (per-bitness executable: target selection, launch profiles, injection, AGTH-code converter). The host talks to its DLL over a private channel inside the plugin and forwards text to the app via the local input API with a context id.
- `windows-sys` for process, memory and module APIs. No step-by-step injection design here; that is the plugin's own design doc.

## 13. Misc
- Errors and logging: `thiserror`, `anyhow` (binaries), `tracing`.
- Regex where definitions need it: `regex`.
- Async glue: `futures`.

## 14. Verified crates

Method (2026-11-08): versions, licenses and declared MSRV from the crates.io API and `cargo info`; transitive licenses by resolving throwaway manifests with `rust-version = "1.90"` for `i686-pc-windows-msvc` and `x86_64-pc-windows-msvc` (`cargo metadata --filter-platform`) and evaluating each license expression against D3; API behaviour from Context7 docs (iced, reqwest, keyring, axum, lindera, retour, arboard, windows-rs) or the GitHub README via `gh api` (vibrato, which Context7 does not index). Nothing was compiled; "MSRV" is the crate's declared `rust-version` ("—" = not declared; the crate still resolved under 1.90).

| Crate | Version | License | MSRV | Verdict |
|---|---|---|---|---|
| `vibrato` | 0.5.3 | MIT OR Apache-2.0 | 1.88 | **Use** (default MeCab backend) |
| `lindera` | 6.2.0 | MIT | 1.88 | Use only without `embed-*` (fallback) |
| `lindera-ipadic` / `lindera-unidic` | 6.2.0 | MIT (crate code) | 1.88 | **Avoid** (build-time download; `webpki-roots` CDLA-Permissive-2.0) |
| IPADIC data (mecab-ipadic 2.7.0) | — | NAIST/ICOT licence (permissive, notice required) | — | OK with notice (data) |
| UniDic data 2.1.2 | — | BSD-3-Clause option of GPL/LGPL/BSD triple | — | OK under BSD-3-Clause (data); newer UniDic unverified |
| EDICT/JMdict data | — | EDRDG licence (CC BY-SA) | — | unverified in this pass; user download + attribution |
| `reqwest` | 0.13.5 | MIT OR Apache-2.0 | 1.85 | **Use** with `default-features = false` + `native-tls` |
| `native-tls` / `schannel` | 0.2.18 / 0.1.30 | MIT OR Apache-2.0 / MIT | 1.80 / 1.71 | Use (via reqwest) |
| `aws-lc-rs` / `aws-lc-sys` | 1.19.0 / 0.47.0 | ISC/Apache-2.0/MIT/BSD-3 mix | 1.71 | Allowed, not needed (native C build) |
| `tokio` | 1.54.1 | MIT | 1.85 | **Use** |
| `tokio-util` | 0.7.20 | MIT | 1.85 | Use (`CancellationToken`) |
| `axum` | 0.8.9 | MIT | 1.80 | **Use** (local API) |
| `tokio-tungstenite` | 0.30.0 | MIT | 1.85 | Use (via axum `ws`) |
| `hyper` | 1.12.0 | MIT | 1.63 | Alternative (lower level) |
| `tiny_http` | 0.12.0 | MIT OR Apache-2.0 | — | Not chosen (no WebSocket, old) |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | 1.56 | **Use** |
| `serde_json` | 1.0.154 | MIT OR Apache-2.0 | 1.71 | **Use** (incl. JSON Pointer) |
| `toml` | 1.2.1 | MIT OR Apache-2.0 | 1.85 | **Use** |
| `keyring` | 4.2.0 | MIT OR Apache-2.0 | 1.88 | **Use** (`windows-native-keyring-store` only) |
| `arboard` | 3.7.0 | MIT OR Apache-2.0 | 1.71 | Use (read/write; no change listener) |
| `clipboard-win` | 5.4.3 | BSL-1.0 | — | Alternative |
| `encoding_rs` | 0.8.44 | (Apache-2.0 OR MIT) AND BSD-3-Clause | 1.88 | **Use** |
| `iced` | 0.14.0 | MIT | 1.88 | **Use** (D14); highest transitive MSRV 1.90; tree includes Apache-2.0-only crates (e.g. `winit`, `glutin_wgl_sys`), fine under D3 |
| `iced_aw` | 0.14.1 | MIT | 1.88 | Optional (drawn menus etc.) |
| `egui` / `eframe` | 0.37.0 (newest ≤1.90: 0.33.3) | MIT OR Apache-2.0 | 1.95 (0.33.3: 1.88) | Not chosen (D14); default fonts add OFL-1.1/Ubuntu-font-1.0 |
| `windows` | 0.62.2 | MIT OR Apache-2.0 | 1.82 | **Use**; don't adopt 0.100-series sub-crates (1.95) |
| `windows-sys` | 0.61.2 | MIT OR Apache-2.0 | 1.71 | **Use** |
| `retour` | 0.3.1 | BSD-2-Clause | — | Use, stable APIs only (no `static-detour`/`thiscall-abi`); 0.4 is alpha |
| `iced-x86` | 1.22.0 | MIT | 1.64 | **Use** (hook plugin) |
| `jsonptr` | 0.8.2 | MIT OR Apache-2.0 | 1.79 | Not needed (`serde_json::pointer`) |
| `serde_json_path` | 0.7.2 | MIT | — | **Use** (JSONPath extraction with joining, D21) |
| `cookie_store` (via `reqwest` `cookies`) | 0.22.1 | MIT OR Apache-2.0 | 1.88 | Use (session step cookies, D21) |
| `jsonpath-rust` | 1.0.12 | MIT | — | Alternative, not chosen |
| `minijinja` | 3.1.0 | Apache-2.0 | 1.70 | Allowed (D3) but not needed (own `{{…}}` placeholders) |
| `upon` | 0.11.0 | MIT OR Apache-2.0 | 1.66 | Not needed |
| `eventsource-stream` | 0.2.3 | MIT OR Apache-2.0 | — | Use (LLM streaming) |
| `libloading` | 0.9.0 | ISC | 1.88 | Use (optional libmecab) |
| `winreg` | 0.56.0 | MIT | 1.60 | Use (`windows-registry` 0.100 needs 1.95) |
| `flate2`, `memmap2`, `regex`, `aho-corasick` | 1.1.11, 0.9.12, 1.13.1, 1.1.5 | MIT OR Apache-2.0 (aho-corasick: Unlicense OR MIT) | 1.67, 1.65, 1.65, 1.60 | Use |
| `html-escape`, `widestring`, `thiserror`, `anyhow`, `tracing` | 0.3.1, 1.2.1, 2.0.21, 1.0.104, 0.1.44 | MIT / MIT OR Apache-2.0 | ≤1.77 | Use |
| `interprocess` | 2.4.8 | 0BSD OR Apache-2.0 | 1.75 | Optional |
| `jsonrpsee` | 0.26.1 | MIT | 1.87 | Optional |
| `muda` | 0.21.2 | Apache-2.0 OR MIT | 1.90 | Optional (native menu bar; iced integration unverified) |
| `global-hotkey` | 0.9.0 | Apache-2.0 OR MIT | 1.77 | Optional |
| `native-windows-gui` | 1.0.13 | MIT | — | **Reject** (dep `stretch` has no license; unmaintained since 2022) |
| `slint` | — | GPL-3.0 / commercial | — | **Excluded** |

Transitive check results (both Windows targets): core stack (reqwest+native-tls, tokio, axum, serde/toml, keyring, arboard, clipboard-win, encoding_rs, windows, libloading, …): all permissive. Web-connector stack (reqwest with `cookies`/`form`/`query`, `serde_json_path`, `regex`): all permissive. iced 0.14 stack: all permissive. Hook stack (retour + iced-x86): all permissive. lindera with `embed-*`: one non-allow-listed license (`webpki-roots`).
