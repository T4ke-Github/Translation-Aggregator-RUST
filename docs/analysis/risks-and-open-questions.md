# Risks and open questions

Follows `../product-direction.md`. Items settled by decisions D1–D21 are listed at the end and no longer tracked here.

## 1. Licensing and clean-room discipline (highest priority)
- **Why it matters.** The legacy code is GPLv2 (no "or later" notice); the rewrite is `MIT OR Apache-2.0` (D3). That only holds if the rewrite is not a derivative work (D13). Translating C++ functions into Rust, or copying tables and comments, would put the rewrite under GPLv2.
- **Discipline.**
  - Implementers work from behaviour and format descriptions (`features.md`, `formats-and-interfaces.md`, `ui-reference.md`) and black-box observation of the running app, not from the C++ files. The analysis docs cite file:line so facts can be checked; they are descriptions, not code to transcribe.
  - Where possible, the person who reads legacy code to answer a question is not the one who writes the Rust for it; answers are passed on as behaviour ("input X gives output Y").
  - Review checklist for every PR touching a reimplemented feature: no copied identifiers beyond public format names, no copied tables, no copied comments.
  - Parity tests are black-box input/output only.
- **Legacy-authored assets that need permissive replacements** (not to be shipped): `dictionaries/Conjugations.txt` (self-authored conjugation rules instead), `Rule Sets\*.trs` (cut anyway, D7), `exe/toolbar.bmp` and `exe/icon1.ico` (new icons), `exe/TranslationAggregator.ini` (sample INI), `docs/Hook Codes.txt` and other user docs (new docs written from the behaviour; the hook-code *syntax* itself is a format and stays compatible), `Sample Dll/`.
- **Third-party data licences and attribution.**
  - EDICT/ENAMDICT/JMdict: EDRDG licence (CC BY-SA per the EDRDG statement; not re-verified in this pass). Share-alike applies to the dictionary data, attribution is required. Default: the user downloads; if bundled, ship the licence and attribution.
  - MeCab dictionaries: IPADIC (NAIST/ICOT licence, keep the notice), UniDic 2.1.2 (use under its BSD-3-Clause option). Newer UniDic licence: unverified.
  - Third-party licence file for all crates in each package (generated at release).
- **Third-party code of unknown provenance:** the legacy "Z0mbie" length disassembler (`dll/Hooks/disasm.cpp`) is not used; the hook plugin uses `iced-x86` (MIT).

## 2. LLM translator: cost, latency, privacy, quality
- **Cost.** Auto-translating every clipboard change or every hooked line can send many paid requests, multiplied by the number of enabled providers. Mitigations: remote providers start with auto-translate off (the legacy default for remote panes, `TranslationWindow.cpp:22`), per-provider budgets (requests or characters per day/month) with a visible counter, debounce and dedupe in the pipeline, cache hits shown as such, and a length cap per provider.
- **Latency.** LLM responses are slower than MT APIs. Mitigations: streaming into the pane, per-provider timeouts, cancel/replace when newer text arrives, showing request time and status per pane.
- **Privacy.** Clipboard and game text go to a third party. The clipboard can hold passwords or personal data. Mitigations: explicit opt-in per remote provider with a clear "text leaves this machine" indicator, Japanese-only and length filters before sending, honour clipboard formats that password managers set to exclude content from monitoring, local model servers as the private option, no telemetry.
- **Quality and prompt injection.** Game or clipboard text can contain instructions the model follows. Impact is limited because output is only displayed; never let model output trigger actions, and label it as machine translation.
- **Model settings.** Model names, endpoints and prices change; all are user settings (no hard-coded defaults), and prices are entered by the user if cost display is wanted.

## 3. API-key storage
- Keys live in the Windows Credential Manager (`keyring` or `CredWriteW`), never in TOML, logs, history exports or crash reports.
- Credential Manager entries are readable by any process running as the same user; this is the platform's normal protection level and must be documented.
- Portable installs: keys stay on the machine/user where they were entered; moving the folder loses them (document; offer re-entry).
- Settings import/export must skip secrets; provider definitions reference a credential name only.

## 4. Local input/output API security
- Loopback-only is not authentication: every local process, and **web pages in the user's browser**, can reach `127.0.0.1`. Browsers can send simple cross-origin POSTs without a preflight, and DNS rebinding can defeat same-origin checks.
- Mitigations (beyond D18's off-by-default, loopback-only, optional token): require `Content-Type: application/json` (forces a CORS preflight), reject requests with a foreign `Origin` unless allow-listed, check `Host` against loopback names, no CORS headers by default, request size limits, rate limits so a local process cannot burn the user's LLM budget, constant-time token comparison.
- The output stream exposes every translation to any local reader. Recommend requiring the token whenever the output stream is enabled (even if text input stays tokenless), and saying so in the settings UI (O12).
- Port conflicts: configurable port with a clear error if taken.

## 5. Provider terms of service
- Official APIs (DeepL, Google Cloud Translation, Microsoft Translator, LibreTranslate hosted instances, LLM providers) each have terms on data use, caching, attribution, rate limits and free-tier limits. Each preset needs a short note linking the provider's current terms; caching translations may be restricted by some terms (check per provider when writing the preset; unverified here).
- The project ships no API keys; every user brings their own.
- The web connector lets users define arbitrary endpoints, including unofficial ones. Built-in presets are official APIs only. The keyless Google and Bing web versions ship as **disabled example definitions marked unofficial** (D21). Risks: they may violate the providers' terms, break without notice (support load), and get the user's IP rate-limited or blocked. Mitigations: disabled by default, an *unofficial* badge on the pane and a "use at your own risk" notice when enabling, conservative `rate_limit` values in the examples, the official presets offered as the recommended path for the same services, the setup guide (`../guides/custom-web-services.md`) explains the trade-off and how to repair a broken definition, examples are versioned separately from presets so they can be fixed without a release, and users are responsible for the terms of endpoints they enable or add.
- Hook plugin: some games' terms or anti-cheat forbid injection; the plugin is optional and its docs say so.

## 6. Behaviour changes (D20: fix, document)
The legacy bugs found during analysis are fixed, not reproduced, and each fix goes into the behaviour-changes list:
- partial-send bug in the socket layer (`exe/BufferedSocket.cpp:209`) — the code path is gone, the hook plugin uses its own channel;
- context-settings reset that clears only 4 bytes (`exe/Config.cpp:1192`);
- reference leak on the auto-translate fall-through (`exe/TranslationAggregator.cpp:1345-1358`);
- `SETSUMI_CHANEGS` typo (`exe/TranslationAggregator.res.rc:157`);
- hard-coded buffer sizes that truncate (`contextString[2024]`, `temp[6000]` layout strings);
- CWD-relative INI and data paths (`architecture.md` §6);
- DeepL space quirk (`DeepLWindow.cpp:188-189`) — moot with the DeepL API preset.
Parity tests mark these as intended divergences.

## 7. GUI toolkit (iced) gaps
Details and mitigations in `ui-reference.md` §11. Main risks:
- no native Win32 menu bar in iced (drawn menus, or `muda` attached via the window handle — unverified);
- pane drag between top-level frames is not built in (`pane_grid` works inside one window);
- whole-window transparency needs the raw window handle (`SetLayeredWindowAttributes`) — unverified with iced's renderer;
- Japanese IME behaviour in iced text inputs: unverified, must be tested early because the Original Text pane is an editor;
- GPU renderer on old 32-bit machines/VMs: keep the `tiny-skia` software fallback;
- screen-reader support: unverified; keyboard accessibility is required by D11 regardless.
Spike these in the UI design step before Phase 2.

## 8. Plugin protocol versioning and plugin trust
- Third-party plugins in any language depend on the protocol. Policy (Proposal): `major.minor` version in the handshake; minor versions only add optional methods/fields; a major bump keeps the previous major supported for at least one release; unknown fields are ignored; capability negotiation instead of version checks where possible.
- Plugins are executables running with the user's rights. The app never downloads plugins by itself; the first start of a new plugin asks the user; manifests show what kind of plugin it is.
- Hangs and crashes: timeouts per call, restart with backoff, clear status in the UI.

## 9. Hook plugin (optional, highest technical effort)
- The x86_64 hook backend and the 64-bit hook-code extension are net-new; there is no legacy oracle for them.
- Injection triggers antivirus and anti-cheat; expect false positives and support questions. Signing the binaries helps a little.
- Test matrix {32-bit app, 64-bit app} × {32-bit target, 64-bit target} needs purpose-built test programs and a few real games.
- Japanese-locale launch: AppLocale is obsolete and a no-op on modern Windows (`formats-and-interfaces.md` §11); see O13.

## 10. Migration of user data
- The legacy INI lives in the current working directory (`exe/Config.cpp:850-864`), so the importer must ask for or search the old location.
- `WritePrivateProfileStruct` hex blobs and versioned `ContextSettings` need explicit decoding or are skipped.
- Layouts referencing cut panes (ATLAS, LEC, scraper panes) import without them.
- The `.bin` dictionary cache is not imported; it is rebuilt.

## 11. Testing dependencies
- Black-box fixtures need the legacy app running on Windows, with MeCab and an EDICT file installed, and a test program for hook capture. Capture them early (Phase 0).
- Live provider and LLM tests need user keys or a local model server; they are opt-in and never run with project-owned secrets in public CI.
- Real i686 runs of the iced GUI (GPU path) are not verified yet.

## 12. Open questions

Product-level open questions are tracked in `../product-direction.md`. Current list:

| # | Question | Recommendation |
|---|---|---|
| O10 | Default auto-translate for remote (paid/LLM) providers? | Off, with per-provider budgets. |
| O11 | Bundle a MeCab dictionary and/or EDICT, or download on first use? | Download on first use, show licence; bundling only with licence + attribution. |
| O12 | Require the token for the local API output stream even if input stays tokenless? | Yes when exposing the stream; warn in settings. |
| O13 | Japanese-locale launch in the hook plugin: reimplement something, or point to external tools? | Document external tools; no AppLocale reimplementation. |
| O14 | Native Win32 menu bar (via window handle) or iced-drawn menus? | Decide in the UI design step after a spike. |

## Resolved (no longer tracked)
| Former item | Resolved by |
|---|---|
| GPL-2.0-only vs -or-later; Apache-2.0-only deps | D3 (`MIT OR Apache-2.0`, permissive deps), D13 |
| UI fidelity / toolkit choice | D11, D14 (`iced`, classic Win32 look) |
| Hooking both bitnesses inside the core | D6 (hook plugin), D19 |
| 32-bit engine host for ATLAS/LEC/MeCab | D7 (cut), D9 (pure-Rust MeCab) |
| Which web providers; scrapers vs official APIs; keyless Google/Bing (O9) | D10, D17, D21 |
| Legacy plugin ABI | D15 |
| INI forever vs import | D16 |
| Keep vs fix legacy bugs | D20 |
| Local API defaults | D18 |
