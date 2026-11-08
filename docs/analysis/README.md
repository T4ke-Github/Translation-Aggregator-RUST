# Translation Aggregator — Rust rewrite preparation

**Authoritative source for scope and design: [`../product-direction.md`](../product-direction.md)** (decisions D1–D21, proposals, open questions). Where anything in this folder disagrees with it, the decision record wins.

This folder has two kinds of documents:
- **Legacy reference** — describes the C++ app in `../../Old-Codebase/` as it is, with file:line citations. Each has short "Rewrite disposition" notes. Under D13 these are references for features, behaviour and formats only; nothing is translated from the code.
- **Rewrite plan** — follows the decision record.

## Index

Legacy reference:
- **[architecture.md](architecture.md)** — components, process boundaries, threading, the exe↔hook-DLL protocol, the capture → translation → display flow.
- **[module-inventory.md](module-inventory.md)** — every directory/module: purpose, size, dependencies, dead code; disposition table per module.
- **[features.md](features.md)** — user-facing features (panes, Japanese tools, input/capture, text processing, UI) with a **Rewrite** column (Keep / Modernize / Plugin / Replace / Cut).
- **[formats-and-interfaces.md](formats-and-interfaces.md)** — INI, `UserDict.txt`, dictionaries, plugin ABI, hook-code language, engine DLLs, IPC, web endpoints; disposition per format.
- **[ui-reference.md](ui-reference.md)** — legacy look and behaviour (frames, panes, menus, toolbars, shortcuts, dialogs, colours, fonts), modernization opportunities, and the toolkit comparison / iced notes. Input for the UI design step.

Rewrite plan:
- **[rust-mapping.md](rust-mapping.md)** — per component: Rust approach and crates; bitness, MSRV and license rules; **verified-crates table** (§14).
- **[workspace-layout.md](workspace-layout.md)** — crates, dependency diagram (Mermaid), packaging for both bitnesses, hook-plugin artifacts.
- **[migration-plan.md](migration-plan.md)** — phases: core + clipboard + one preset → GUI → LLM → MeCab/JParser → local API → plugin protocol → hook plugin → packaging; black-box parity only for kept behaviour.
- **[risks-and-open-questions.md](risks-and-open-questions.md)** — clean-room discipline and data licences, LLM cost/latency/privacy, API-key storage, local API security, provider terms, plugin protocol versioning, iced gaps, open questions.

Related: [`../guides/custom-web-services.md`](../guides/custom-web-services.md) (web connector definition format and setup guide, D21).

## Executive summary

**The legacy app.** Translation Aggregator is a GPLv2 Win32 desktop app (32-bit only) that shows many machine translators side by side in a multi-pane window: two offline engines (ATLAS, LEC), about fourteen unofficial web scrapers, a built-in Japanese parser (JParser) and MeCab. It takes text from typing, the clipboard and an injected 32-bit hook DLL. The CI build does not produce the hook DLL (`architecture.md` §1).

**The rewrite** (`../product-direction.md`):
- **Focus:** translate text that arrives via the **clipboard** or a **local API** with every configured translator, side by side (D5). Built for **i686 and x86_64** (D1), **MSRV 1.90** (D2), licensed **`MIT OR Apache-2.0`** with permissive dependencies only (D3).
- **Clean reimplementation** (D13): the legacy app is a reference for features, behaviour and formats; no code is translated, no legacy-authored data (e.g. `Conjugations.txt`) is shipped, parity is tested black-box.
- **Translators:** a declarative **web connector** (D10) with official-API presets — DeepL, Google Cloud Translation, Microsoft Translator, LibreTranslate (D17) — plus disabled "unofficial" example definitions for the keyless Google/Bing web versions (D21); an **LLM plugin** for OpenAI-compatible APIs (incl. local servers) and Anthropic (D8). ATLAS and LEC are **cut** (D7), so there is no 32-bit engine host any more.
- **Japanese tools kept:** MeCab via the pure-Rust **`vibrato`** analyzer by default (D9, verified), JParser with user-supplied EDICT and self-authored conjugation rules, furigana, substitutions, history (D12).
- **Hooking** becomes an out-of-process **hook plugin** under `plugins/hook/` (D6, D19) with per-bitness hosts and DLLs, feeding the app through the local API.
- **Plugins:** only the new out-of-process JSON-RPC protocol (D15). **Settings:** one-time INI/`UserDict.txt` import, then TOML (D16).
- **UI:** the familiar multi-pane layout, modernized (D11), built with **`iced`** in a classic Win32 style (D14).

**Key findings for planning.**
1. With ATLAS/LEC cut and MeCab in pure Rust, the core is identical for both bitnesses; only the hook plugin and the optional `libmecab` backend are bitness-specific (`rust-mapping.md` §0.1).
2. All recommended crate stacks resolve under MSRV 1.90 with permissive licenses for both Windows targets. Watch-outs: `lindera` `embed-*` features (build-time download, `webpki-roots` CDLA license), newest `egui`/`windows-*` 0.100 crates need Rust 1.95, `native-windows-gui` rejected (dependency without license). `reqwest` should use `native-tls` (SChannel) instead of its default rustls/aws-lc (`rust-mapping.md` §14).
3. Clean-room discipline is the main legal risk; legacy-authored assets (conjugation data, icons, sample INI, user docs) need replacements, and dictionary data needs licence/attribution handling (`risks-and-open-questions.md` §1).
4. The local API is reachable from browsers on the same machine; it needs `Host`/`Origin` checks and JSON-only requests in addition to loopback binding (`risks-and-open-questions.md` §4).
5. iced gaps to spike before the GUI phase: native menu bar, pane drag between windows, whole-window alpha, Japanese IME (`ui-reference.md` §11).
6. **Correction to earlier analysis:** Ctrl-H/K/R (to hiragana/katakana/romaji) are active in the legacy app, not dead (`features.md` §2).
7. Much legacy code is dead or inactive and is cut (`features.md` §6, `module-inventory.md`).

Scope note: analysis and planning only. No Rust code or Cargo files exist; crate choices are recommendations.
