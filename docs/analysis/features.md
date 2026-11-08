# User-facing feature inventory

Legend for **Status** (legacy app): **Active** (compiled and reachable in the CMake build); **Inactive** (compiled but not instantiated or unreachable); **Dead** (not compiled, stub, or only behind undefined macros); **DLL-only** (needs `App Translator.dll`, which the CMake/CI build does not produce; see `architecture.md` §1).

Legend for **Rewrite** (target, per `../product-direction.md`): **Keep** (same feature, reimplemented from behaviour, D13); **Modernize** (kept, with better UI/UX or a modern mechanism); **Plugin** (moves out of the core into a plugin, mainly the hook plugin, D6); **Replace** (the need is met by a different mechanism, e.g. the declarative web connector, D10); **Cut** (dropped: D7, dead service, or dead code).

## 1. Translation panes (`exe/TranslationWindows/TranslationWindowFactory.cpp:27-54`)

Factory order matters: layouts and menus refer to panes by `windowType`, and `windows[0]` must be "Original Text".

| # | Pane (`windowType`) | Kind | Implementation | Languages | Status | Rewrite |
|---|---|---|---|---|---|---|
| 0 | Original Text | input | `LocalWindows/UntranslatedWindow.cpp` (toolbar toggle "Auto Translate Highlighted Text", INI `Trans Highlighted`) | — | Active | Modernize: also shows which input (clipboard, API, hook plugin, typed) delivered the text |
| 1 | ATLAS | local engine | `LocalWindows/AtlasWindow.cpp`, `Shared/Atlas.cpp` | ja↔en (direction from `langDst`, `AtlasWindow.cpp:83-85`) | Active, but needs Fujitsu ATLAS V13/V14 installed | Cut (D7) |
| 2 | LEC | local engine | `LocalWindows/LECWindow.cpp` (`Plugins\Nova\JaEn\EngineDll_je.dll` or the LEC Power Translator 15 install path from the registry) | ja→en (code page 932 hard-coded; ko→en variant behind `#if 1`) | Active, but needs LEC Nova | Cut (D7) |
| 3 | Excite | web | `HttpWindows/ExciteWindow.cpp` (`www.excite.co.jp` POST, scrape `name="after"` textarea) | ja/en (+ others in `GetLangIdString`) | Active | Replace: no preset (no official API); a user may write a connector definition |
| 4 | LEC Online | web | `HttpWindows/LECOnlineWindow.cpp` (`www.lec.com/translate-demos.asp`) | multiple | Active. ChangeLog r185 says "out of translations, no real way to fix" | Cut (service dead) |
| 5 | Google | web | `HttpWindows/GoogleWindow.cpp` (`translate.google.com/_/TranslateWebserverUi/data/batchexecute`, rpcid `MkEWBc`, hard-coded `bl=boq_translate-webserver_20211006.11_p0`, fixed `f.sid`) | about 60 | Active | Replace: Google Cloud Translation API preset (D17); the keyless web version ships only as a disabled, "unofficial" example definition (D21) |
| 6 | Bing | web | `HttpWindows/BingWindow.cpp` (`www.bing.com/ttranslatev3`, scrapes `IG`, `IID`, key and token from `/translator`, refreshes after 100 uses) | many | Active | Replace: Microsoft Translator API preset (D17); the keyless web version ships only as a disabled, "unofficial" example definition (D21) |
| 7 | DeepL | web | `HttpWindows/DeepLWindow.cpp` (`www2.deepl.com/jsonrpc`, `LMT_handle_jobs`, sentence splitting, deliberate space quirk at `:188-189`) | 24 | Active | Replace: DeepL API preset (D17); the space quirk is not reproduced (D20) |
| 8 | Yandex | web | `HttpWindows/YandexWindow.cpp` (`translate.yandex.net/api/v1/tr.json/translate`, SID scraped from `translate.yandex.com/`) | many | Active | Replace: no first preset; user definition against Yandex's official cloud API possible |
| — | Honyaku (Yahoo) | web | `HttpWindows/HonyakuWindow.cpp` | ja/en | **Inactive** (compiled, factory line commented "Service shutdown") | Cut (service dead) |
| 9 | SysTran | web | `HttpWindows/SysTranWindow.cpp` (`translate.systran.net/translate/html`, CSRF token from `/translationTools/text`, SHA-256 signature via PicoSHA2) | many | Active | Replace: no preset; official keyed API can be a user definition |
| 10 | SDL | web | `HttpWindows/SDLWindow.cpp` (`api.freetranslation.com/freetranslation/translations/text`, JSON) | many | Active | Replace: no preset (status unknown) |
| 11 | Infoseek | web | `HttpWindows/InfoseekWindow.cpp` (`translation.infoseek.ne.jp/clsoap/translate`, hard-coded `Cross-Licence` header) | ja/en/ko/zh… | Active | Cut: unofficial only; no preset |
| 12 | Baidu | web | `HttpWindows/BaiduWindow.cpp` (`fanyi.baidu.com/v2transapi`, token + JS-derived `sign`, cookie) | zh/en/ja/… | Active | Replace: Baidu's official API needs signed requests, so only as a translator plugin (not first-party) |
| 13 | Babylon | web | `HttpWindows/BabylonWindow.cpp` (`translation.babylon-software.com/translate/babylon.php`, JSONP) | many (numeric ids) | Active | Cut: unofficial only |
| 14 | Babel Fish | web | `HttpWindows/BabelfishWindow.cpp` (`www.babelfish.com` POST, then GET `/success`, scrape `<h4>Translation</h4>`) | many | Active | Cut: unofficial only |
| — | FreeTranslation | web | `HttpWindows/FreeTranslationWindow.cpp` (`tets9.freetranslation.com`) | — | **Inactive** (factory commented; ChangeLog r183 "server seems to be dead") | Cut (dead) |
| 15 | WWWJDIC | web dictionary | `HttpWindows/JdicWindow.cpp` (mirror configurable, INI `[WWWJDIC] Mirror`, default `http://wwwjdic.se/cgi-bin/wwwjdic.cgi`) | ja→en only | Active | Replace: optional user connector definition; local JParser covers dictionary lookup |
| 16 | Mecab | local morphological | `LocalWindows/MecabWindow.cpp`, `util/Mecab.cpp` | ja | Active, but needs MeCab (UTF-8/SJIS/EUC-JP dictionary) | Keep (D9): pure-Rust analyzer by default, optional libmecab |
| 17 | JParser | local dictionary parser | `LocalWindows/JParseWindow.cpp`, `util/Dictionary.cpp` | any EDICT-format dictionary (ChangeLog r189) | Active, but needs `dictionaries\edict2` (or `.gz`) and `Conjugations.txt` | Keep (D12): own conjugation data (D13), user-supplied EDICT |

New in the rewrite (no legacy pane): **LLM translator** (Plugin, D8: OpenAI-compatible incl. local servers, Anthropic) and **LibreTranslate** (web connector preset, D17).

The status of the web services' *network endpoints* is covered in `formats-and-interfaces.md` §10. All of them are unofficial web-scraping integrations.

Common pane behaviour (`TranslationWindow.cpp`, `HttpWindow.cpp`):

| Behaviour | Code | Rewrite |
|---|---|---|
| Translate button; the button label is the pane name | `TranslationWindow.cpp:116` | Modernize (pane header with name, status, actions) |
| "Automatic Clipboard Translation" toggle (INI `[<pane>] Auto Clipboard`, default on for local panes and off for remote ones) | `TranslationWindow.cpp:22,117` | Modernize: "auto translate" per pane, for every input source |
| Optional Configure button | `TranslationWindow.cpp:130-135` | Modernize (opens the pane's page in the settings UI) |
| Close button | `TranslationWindow.cpp:140-146` | Keep |
| Busy animation (7-frame icon on the translate button) | `TranslationWindow.cpp:641-658` | Modernize (spinner plus request time/status) |
| One request in flight; a newer request replaces the queued one | `README`, `HttpWindow.cpp` | Keep (router-level cancel/replace) |
| Results cached in History | `exe/History/History.cpp` | Keep |
| "Websites" menu opens each service's `srcUrl` in the browser | `TranslationAggregator.cpp:601-608,1703-1717` | Modernize (per-provider link to docs/console) |
| Pane context menu: Cut/Copy/Paste, To Hiragana/Katakana/Romaji, SJIS/UTF-16 ↔ hex, Configure, plus a copy of the View menu | `TranslationWindow.cpp:315-445` | Modernize (per-pane actions: copy, retry, pin) |

## 2. Japanese-language tools

| Feature | Code | Notes | Rewrite |
|---|---|---|---|
| JParser word segmentation with conjugation de-inflection, name detection (enamdict), MeCab-assisted mode (`JPARSER_USE_MECAB`) | `util/Dictionary.cpp` (`FindBestMatches`, `SortMatches`, `GetConjString`) | JParser config dialog (`IDD_JPARSER_CONFIG`) has: show conjugations, Japanese on its own line, single kanji, single hiragana, use MeCab, hide cross refs/usage/POS, definition lines, reformat numbers, no kana brackets | Keep (D12, reimplemented; conjugation rules self-authored, D13) |
| Furigana display (hiragana/katakana/romaji/none), colours, font sizes | `LocalWindows/FuriganaWindow.cpp` (`FuriganaCharacterType`, INI keys `BK Color`, `Furigana BK Color`, `Particle BK Color`, `Trans BK Color`, `Normal Font Size`, `Furigana Font Size`, `Character Set`) | shared by JParser and MeCab | Modernize (ruby layout, theme-aware colours, DPI scaling) |
| Hover tooltips with dictionary definitions | `Dialogs/MyToolTip.cpp` | colours `toolTipKanji/Hiragana/Paren/Conj`, separate tooltip font | Modernize (scrollable, pinnable, keyboard-reachable) |
| Auto half-width to full-width katakana | `TranslationAggregator.cpp:147-156` | menu Tools, INI `AutoHalfWidthToFull` | Keep |
| Auto romaji to hiragana when no Japanese is present | `:159-170` | `CONFIG_ENABLE_AUTO_HIRAGANA` | Keep |
| Ctrl-H / Ctrl-K / Ctrl-R (convert selection, or the whole pane text, to hiragana/katakana/romaji) | accelerators in `.rc` (`TranslationAggregator.res.rc:143-145`); handler `TranslationWindow.cpp:478-585`; the message loop routes accelerators to the pane under focus (`TranslationAggregator.cpp:2049-2054`) | **Correction:** an earlier version of this document called these dead. They are handled in `TranslationWindowProc` and also reachable from the pane context menu (`TranslationWindow.cpp:377-385`) | Keep |
| SJIS/UTF-16 to hex and hex to SJIS/UTF-16 (debug aid for hook work) | `TranslationWindow.cpp:359-369,509-571` | pane context menu only | Keep (as pane actions) |
| ATLAS dictionary search (`/ATLAS:DICTSEARCH`) | `AtlasWindow.cpp:137` (`LaunchAtlasDictSearch`) | requires ATLAS | Cut (D7) |

## 3. Text input and capture

| Feature | Code | Status | Rewrite |
|---|---|---|---|
| Manual input + Translate All / per-pane translate | `TranslationAggregator.cpp:298-320` | Active | Keep |
| Auto translate highlighted text in Original Text | `WMA_TRANSLATE_HIGHLIGHTED` `:1294-1312` | Active | Keep |
| Clipboard monitoring (legacy viewer chain) treated as context `::System`/`Clipboard` | `:1245`, `:1399-1416`, `:1358-1377` | Active | Modernize (`AddClipboardFormatListener`; core input, D5) |
| — (new) Local input API: HTTP/WebSocket on `127.0.0.1` | — | — | New (D5, D18: off by default, token optional) |
| 500-character cap for automatic web requests | `:399-400` | Active | Modernize (per-provider limit in the router; configurable) |
| Translation history back/forward (Alt-Up/Down, Alt-PgUp/PgDn, optional global Ctrl-Alt hotkeys) | `exe/History/History.cpp`, `:1378-1398` | Active | Modernize (searchable history, D11) |
| Context Manager dialog (per process/context: auto translate, disable, merge (insta/flush), text-loop detection, Japanese only, auto clipboard, repeat-char and phrase filters, line-break modes, idle delay, max history, hook port) | `exe/Context.cpp:952-1300`, `exe/Filter.cpp` | Active for clipboard; for game text it is **DLL-only** | Modernize: contexts and filters stay in the core and apply to every input (clipboard, API, hook plugin); the hook port setting goes away |
| Launch game with Japanese locale (AppLocale env vars) | `Shared/DllInjection.cpp:133-164`, `Shared/ProcessUtil.cpp` | Active. AppLocale is an obsolete Microsoft shim, so it is a no-op on systems without it (risk) | Plugin (hook plugin launcher; mechanism to be re-evaluated) |
| Launch/attach AGTH with `/P<pid>` and `/C /KS /KF` options | `exe/util/Injector.cpp:9-21,109-125` | Active if `AGTH.exe` is present (not shipped in this repo) | Cut: external hookers feed the app via clipboard or the local API |
| Inject TA DLL (menu translation, DBCS override, internal hooks) | `Injector.cpp:107-108`, `dll/` | **DLL-only** | Plugin (hook plugin, D6) |
| Internal text hooks (default GDI hooks + user hook codes, AGTH /H code converter) | `dll/Hooks/TextHooks.cpp`, `Dialogs/InjectionDialog.cpp:576-770` | **DLL-only** (converter UI is Active) | Plugin (hook plugin; 64-bit hook-code syntax designed there) |
| Inject into child processes | `dll/...dll.cpp:34-72,549-552` | **DLL-only** | Plugin |
| Translate game menus, dialogs and MessageBoxes with ATLAS | `dll/WindowTranslator.cpp`, `dll/...dll.cpp:316-376` | **DLL-only** and needs ATLAS | Cut (D7) |
| Translation log file `log\<folder>_<exe>.log` | `Config.cpp:1161-1181`, `Atlas.cpp` (`TranslateFullLog`) | **DLL-only** | Replace: history export in the core |
| Launch profiles (per-game INI in `Game Configs\`), drag-and-drop exe onto window/dialog, create `.lnk` shortcut with `/I:<exe>` | `Dialogs/InjectionDialog.cpp:940-1000`, `TranslationAggregator.cpp:1759-1761` | Active | Plugin (hook plugin) |
| Command line `/I:<exe path>` (inject using saved profile; works when another instance is running) | `TranslationAggregator.cpp:103-125,1956-1960` | Active | Plugin (hook plugin's own command line) |

## 4. Text processing

| Feature | Code | Notes | Rewrite |
|---|---|---|---|
| Substitutions: global `*` profile + per-game profiles, longest match, escapes `\n` `\t` `\\` | `Config.cpp:184-589`, dialog `IDD_OPTIONS_DIALOG` (Alt-S) | Stored in `Dictionaries\UserDict.txt` | Keep: TOML storage, one-time `UserDict.txt` import (D16); profiles keyed by input context, not only by running process |
| Plugins (pre/post substitution string transforms, active profile notification) | `Config.cpp:891-936`, `TranslationAggregator.cpp:179-290` | `plugins\*.dll` | Replace: out-of-process text-transform plugins (D15); legacy DLLs unsupported |
| ATLAS rule sets (`.trs`) for sentence breaking/replacement before ATLAS | `Shared/TextRuleSet.cpp` | `Rule Sets\` folder, per-profile `[Atlas] Rule Set` | Cut (D7) |

## 5. Window management / UI

See `ui-reference.md` for the detailed look and behaviour.

| Feature | Code | Rewrite |
|---|---|---|
| Multiple top-level frames. Drag panes between frames (blue drop indicator). Lock order. | `TranslationAggregator.cpp:453-1217,1483-1557` | Modernize (D11) |
| One/two columns. Draggable splitters (double-click resets). | `:680-788,1762-1830` | Keep |
| Transparency (Ctrl-Num+/-), always on top (Alt-A), hide frame/menu (Ctrl-F), hide toolbars (Ctrl-T) | `:502-540,1629-1662` | Keep |
| 10 bindable layouts (Shift-Alt-0..9 save, Alt-0..9 restore). Layout 0 is auto-saved on close. | `:1024-1216,1678-1687` | Keep (named layouts on top) |
| Show all panes (Ctrl-0). Toggle individual panes (Window menu). | `:1669-1702` | Keep |
| Source/target language menus (69 languages, `Config.h:20-94`). Panes enable or disable themselves by capability. | `:1718-1735`, `TranslationWindow::UpdateEnabled` | Modernize (searchable language pickers; capability from provider definitions) |
| Fonts: main and tooltip (`ChooseFont`) | `Config.cpp:1271-1305` | Modernize (system font default, font scaling, per-pane override) |
| Global hotkeys toggle (Ctrl-Alt-Up/Down/PgUp/PgDn/A/Num-/Num+) | `:67-101,1594-1604` | Keep (rebindable) |
| Borderless window, reset placement (Ctrl-P), splitter colour, colours stored in layouts | **Dead** (`SETSUMI_CHANGES` only) | Cut (theming in the rewrite covers colours) |

## 6. Dead / unused code (candidates to drop, not port)

Rewrite: **Cut** for every item below.

- `Text Replacement Hook (Not used)/` (entire folder)
- `dll/GetText.cpp` bitmap text locator (only `SetCacheSize` is reached)
- `exe/util/Regexp.cpp` (stub)
- `Shared/ConversionTable.*` (not compiled)
- `Shared/DllInjection.cpp`: `InjectIntoProcess`, commented-out Toolhelp helpers
- `HonyakuWindow`, `FreeTranslationWindow` (compiled, not instantiated)
- Google `tk()`/`vi()` token code; commented-out cookie code in `GoogleWindow::GetTranslationPath`
- `GOAT` debug block in `wWinMain`
- all `SETSUMI_CHANGES` branches
- `Miscellaneous/CompileConjugations.cpp` + `dictionaries/Conjugations_old.txt` (one-off tooling)
- Commented-out IAT `CreateProcess` replacement (`dll/...dll.cpp:74-113,226-236`), superseded by inline after-hooks
- `3rdParty/meson_toolchains` submodule (no meson build in tree)

(Ctrl-H/K/R were listed here before; they are active, see §2.)
