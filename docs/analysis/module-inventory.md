# Module inventory

Line counts are `.cpp/.h/.c/.rc` lines per directory (non-recursive), measured with `wc -l`. "In CMake" means listed in `CMakeLists.txt:31-84`. "In DLL proj" means listed in `dll/Translation Aggregator dll.vcxproj:230-252`.

## Top level

| Path | Purpose | Notes |
|---|---|---|
| `CMakeLists.txt` | Builds **only** the exe. Static MinGW link (`-static`, `-municode`, subsystem windows). Defines `GIT_TAG` from `git describe`. `archive` target makes a `.7z` of exe + `docs` + `dictionaries` + `Plugins`. | Links `winmm ws2_32 comctl32 gdi32 winhttp psapi user32 comdlg32 ole32 shell32 advapi32`. Includes `3rdParty/json/single_include/nlohmann`. |
| `.github/workflows/ci.yml` | Cross-compiles on Ubuntu with Homebrew mingw-w64 (GCC) and llvm-mingw (Clang), **i686 only**, and publishes `TranslationAggregator-win32.7z`. | No tests. No DLL build. |
| `.gitmodules` | Submodules `3rdParty/PicoSHA2`, `3rdParty/json` (nlohmann), `3rdParty/meson_toolchains`. **All three are empty in this snapshot.** | Usage is inferred from includes below. |
| `README.md`, `docs/ChangeLog.txt`, `docs/Hook Codes.txt`, `docs/GPL.txt`, `LICENSE` | User docs. `Hook Codes.txt` is the only spec of the hook-code syntax. | `LICENSE` is the plain GPLv2 text. No per-file license headers in the sources (verified by grep). |

## `exe/` (about 18.5k lines total including subdirs)

| File(s) | Lines | Purpose | Key Windows APIs / deps |
|---|---|---|---|
| `TranslationAggregator.cpp` | 2075 | WinMain, window classes, MasterWindow layout/docking, controller window proc, clipboard, hotkeys, substitution pipeline, layout save/load | `CreateFileMapping`, `SetClipboardViewer`, `RegisterHotKey`, `SetLayeredWindowAttributes`, `DeferWindowPos`, `EnumWindows`, `EnumDisplayMonitors`, `DragAcceptFiles`, `ShellExecute`, `CommandLineToArgvW`, `LCMapStringW` |
| `Config.{h,cpp}` | 1617 | Global `Config`, language enum (69 languages), INI I/O, substitution lists (`UserDict.txt`), per-game configs, context-settings blobs, plugin loader, font dialog | `Get/WritePrivateProfile{String,Int,Struct}`, `EnumProcesses`, `GetModuleFileNameEx`, `ChooseFont`, `LoadLibraryEx`, `GetProcAddress`, `FindFirstFile` |
| `Context.{h,cpp}` | 1408 | Context manager for hooked/clipboard text: buffering, idle flush, loop detection, merge, auto clipboard, Context Manager dialog | `timeGetTime`, ListView, clipboard APIs |
| `Filter.{h,cpp}` | 505 | Repeat/phrase/line-break filters for hooked text (`AutoFilter`) | pure logic, apart from `wchar_t` |
| `BufferedSocket.{h,cpp}` | 366 | TCP server for the hook DLL, framing, handshake, port publication | Winsock (`WSAAsyncSelect`, `WSAAccept`), `CreateFileMapping`/`MapViewOfFile` |
| `History/History.{h,cpp}` | 172 | In-memory translation history (entries, per-translator results, back/forward, memory trimming) | `std::list`/`std::map` |
| `Dialogs/InjectionDialog.cpp` | 1075 | "Launch / Inject" dialog: process list, launch profiles, locale, AGTH options, internal hook codes + syntax check, AGTH /H to TA code converter, drag-drop, `.lnk` shortcut creation | `EnumWindows`, `EnumSystemLocales`, `IShellLink`/`IPersistFile` (COM), `GetOpenFileName` |
| `Dialogs/MyToolTip.cpp` | 840 | Custom tooltip window for dictionary definitions | GDI. `SETSUMI_CHANGES`-only `WH_MOUSE_LL` hook |
| `TranslationWindows/TranslationWindow.{h,cpp}` | 768 | Pane base class: toolbar, rebar, RichEdit, drag start, busy animation | comctl32 toolbar/rebar, Msftedit RichEdit |
| `TranslationWindows/TranslationWindowFactory.cpp` | 54 | Instantiates the 18 panes in fixed order | — |
| `TranslationWindows/HttpWindows/HttpWindow.{h,cpp}` | 745 | Base for web translators: async WinHTTP, GET/POST templates, charset sniffing, cookie fetch, history cache | WinHTTP async (`WinHttpSetStatusCallback`) |
| `HttpWindows/*Window.cpp` (14 services) | about 3.1k | Per-service request building and response scraping (see `formats-and-interfaces.md` §6) | WinHTTP. `json.hpp` (Google, DeepL). `picosha2.h` (SysTran) |
| `LocalWindows/UntranslatedWindow` | 113 | "Original Text" input pane | RichEdit |
| `LocalWindows/AtlasWindow` | 399 | ATLAS pane + ATLAS config dialog + `/ATLAS:DICTSEARCH` launcher | `Shared/Atlas`, `Thread` |
| `LocalWindows/LECWindow` | 258 | LEC Power Translator / Nova engine (`EngineDll_je.dll`, `eg_init2`/`eg_translate_multi`) | `LoadLibraryA`, registry `HKLM\SOFTWARE\LogoMedia\LEC Power Translator 15\Configuration` |
| `LocalWindows/FuriganaWindow` | 916 | Custom-painted word/furigana view shared by MeCab and JParser | GDI `DrawText`, `TextOutW` |
| `LocalWindows/MecabWindow` | 326 | MeCab output pane | `util/Mecab` |
| `LocalWindows/JParseWindow` | 519 | Built-in parser pane (edict + conjugations + optional MeCab) | `util/Dictionary` |
| `util/Dictionary.{h,cpp}` | 1615 | EDICT(2)/ENAMDICT loader, `.bin` compiled cache (memory-mapped), conjugation table, match/scoring | `CreateFileMapping`/`MapViewOfFile`, zlib via `LoadFile` |
| `util/DictionaryUtil.cpp` | 109 | Helpers for dictionary display | — |
| `util/Mecab.cpp` | 175 | Dynamic `libmecab.dll` loader (`mecab_new2`, `mecab_sparse_tostr`, `mecab_dictionary_info`, `mecab_destroy`) | registry `Software\Mecab\mecabrc` (HKCU, then HKLM), `SHGetFolderPath` |
| `util/HttpUtil.cpp` | 213 | `UnescapeHtml` (named and numeric entities, tag stripping) | — |
| `util/Injector.cpp` | 154 | Find process by path, launch with AppLocale, inject DLL, launch AGTH | `EnumProcesses`, `OpenProcess`, `CreateProcess` |
| `util/Value.{h,cpp}` | 879 | Hand-written JSON value model + parser (used for `Conjugations.txt`) | — |
| `util/Regexp.cpp` | 38 | **Stub. Never matches.** Not referenced elsewhere. | dead |
| `TranslationAggregator.res.rc`, `resource.h`, `icon1.ico`, `toolbar.bmp` | 916 | Menus, accelerators, 8 dialogs, toolbar | `.rc`; llvm-rc workaround for `TOOLBAR` |
| `TranslationAggregator.ini` | 84 | Sample/default INI (not installed by CMake) | — |

## `Shared/` (4.6k lines; used by both exe and DLL)

| File | Lines | In CMake | In DLL proj | Purpose |
|---|---|---|---|---|
| `Shrink.h/.cpp` | 273 | yes | yes (PCH) | Global header: `windows.h`, `winsock2`, etc. `APP_NAME`, `MAIN_WINDOW_TITLE`, `WMA_*` messages, locale-invariant string comparison overrides, `unify()` kana/width/case folding |
| `StringUtil.{h,cpp}` | 1001 | yes | yes | Kana/kanji classification, `ToHiragana/Katakana/Romaji`, `SpiffyUp` (whitespace normalisation), `ParseJSON` (ad-hoc), `LoadFile` (gzip-transparent, BOM/EUC-JP/SJIS sniffing), `SharedString` refcounted string |
| `EUCJPtoUTF16.cpp` | 257 (125 KB table) | yes | yes | Fast EUC-JP decoder used instead of `MultiByteToWideChar(20932)` |
| `ConversionTable.{h,cpp}` | 278 | **no** | no | Generic table-driven converter. Only referenced inside `#else` of `#if 1` (`StringUtil.cpp:585-596`). **Dead.** |
| `Atlas.{h,cpp}` | 558 | yes | yes | ATLAS V13/V14 DLL loading (`AtleCont.dll`, `awdict.dll`, `awuenv.dll`) and translation (`TranslatePair`). Rule-set driven sentence splitting (`TranslateFull`). Logging (`TranslateFullLog`) |
| `TextRuleSet.{h,cpp}` | 484 | yes | yes | `.trs` rule files (`include`, `break before/after`, `break start before`, `line …`, `replace X with Y`) |
| `HookEval.{h,cpp}` | 716 | yes | yes | Hook-code expression compiler and evaluator over 32-bit registers |
| `TextHookParser.{h,cpp}` | 333 | yes | yes | Hook-code string parser: `[alias]type:value:ctx:len:cp@addr:module:function` |
| `InjectionSettings.h` | 40 | — | — | Wire struct sent to the DLL (bitness-dependent) |
| `DllInjection.{h,cpp}` | 228 | yes | yes | `InjectDll` (`CreateRemoteThread(LoadLibraryW)`), `Run` (AppLocale env vars `__COMPAT_LAYER=#ApplicationLocale`, `AppLocaleID`), `getDebugPriv`. Header comment "Not currently used" is **stale**: `InjectDll`/`Run` are used by `exe/util/Injector.cpp` and the DLL. `InjectIntoProcess` is unused. |
| `ProcessUtil.{h,cpp}` | 10 | yes | yes | `LaunchJap` (ShellExecute with AppLocale env) |
| `Thread.{h,cpp}` | 142 | yes | no | Minimal worker thread with task queue |
| `Lock`, `ReadWriteLock`, `scoped_ptr.h` | 194 | yes | no | `CRITICAL_SECTION` wrapper, RW lock, pre-C++11 smart pointer |
| `TAPlugin.h` | 52 | — | — | Plugin ABI (see `formats-and-interfaces.md` §4) |

## `dll/` (2.6k lines; MSVC-only project, output `App Translator.dll`)

| File | Lines | Purpose | Key APIs |
|---|---|---|---|
| `Translation Aggregator dll.cpp` | 576 | `DllMain`, settings handshake, IAT patching (`OverrideFunction`), menu/dialog translation thread, child-process injection, TCP client | Toolhelp32, `EnumProcessModules`, `VirtualProtect`, Winsock, `CreateFileMapping` |
| `Hooks/HookManager.{h,cpp}` | 443 | Inline hook engine (naked asm stubs, RWX trampolines), thread suspend/resume | `VirtualAlloc(PAGE_EXECUTE_READWRITE)`, `FlushInstructionCache`, `SuspendThread` |
| `Hooks/TextHooks.{h,cpp}` | 398 | Default GDI/text hooks, user hook codes, `TextHook` capture and send | `MultiByteToWideChar`, `CharNextExA`, `IsBadReadPtr` |
| `Hooks/disasm.{h,cpp}` | 247 | Z0mbie x86 length disassembler (third-party code, license unknown) | — |
| `WindowTranslator.{h,cpp}` | 296 | Translate window titles/controls/menus/MessageBox text via ATLAS (`TranslateFullLog`) | `EnumThreadWindows`, `GetMenuItemInfo`, `SetWindowText` |
| `GetText.{h,cpp}` | 637 | Bitmap "string locator" heuristics (`LocateStrings`, `GuessChar`, `DisplayStrings`). **Only `SetCacheSize` is ever called** (`dll/...dll.cpp:537`). Effectively **dead**. | GDI |
| `*.vcproj`, `*.vcxproj(.filters,.user)` | — | VS2008 and VS2015 projects. Platform `Win32` only. Defines `NO_STD;NO_SHELL32;NO_ZLIB`. | — |

## Other directories

| Path | Status |
|---|---|
| `Plugins/place_Nova_here.txt` | Placeholder. LEC Nova engine is expected at `Plugins\Nova\JaEn\EngineDll_je.dll` (`LECWindow.cpp:7,154`) |
| `dictionaries/Conjugations.txt` | **Required runtime data** (UTF-16LE JSON-like list of verb types, 4820 lines). Without it JParser loads no dictionaries (`Dictionary.cpp:699`) |
| `dictionaries/Conjugations_old.txt` | Source format for `Miscellaneous/CompileConjugations.cpp`. Not read at runtime |
| `Miscellaneous/CompileConjugations.cpp` | One-off converter (old TSV to new JSON). Not built. **Dead/tooling.** |
| `Sample Dll/` | Example plugin (VS2008). Only an example, and buggy: it copies in the wrong direction (`in++[0] = outPos++[0]`, `SampleDll.cpp:39`), and its signatures differ from `TAPlugin.h` (`wchar_t*` vs `const wchar_t*`) |
| `Text Replacement Hook (Not used)/` | 1.7k lines of an in-game text-replacement/DirectDraw/font-override experiment. In no build. **Dead.** |
| `3rdParty/zlib` | Inflate-only subset (`NO_GZCOMPRESS`), used by `LoadFile` for `.gz` dictionaries |
| `3rdParty/json` (empty) | nlohmann/json single header. `#include <json.hpp>` in `GoogleWindow.cpp:9` and `DeepLWindow.cpp:5` |
| `3rdParty/PicoSHA2` (empty) | SHA-256. `SysTranWindow.cpp:7,135` (request signature) |
| `3rdParty/meson_toolchains` (empty) | Cross-compile toolchain files. No `meson.build` in tree, so it looks unused |

## Compile-time variants
- `SETSUMI_CHANGES` (borderless window, colours saved in layouts, tooltip mouse hook, IE user agent, `JAP_WORD_TOP`) is **not defined** by `CMakeLists.txt` or the vcxproj. All `#ifdef SETSUMI_CHANGES` code is inactive in shipped builds (about 30 sites, `grep -rn SETSUMI_CHANGES`).
- `GOAT` block in `wWinMain` (`TranslationAggregator.cpp:1984-2039`) is a dictionary-statistics debug tool. Dead.
- `TKK` Google token code (`GoogleWindow.cpp:15-81`): defined, but `tk()` is no longer called.

## Rewrite disposition

What happens to each legacy module in the rewrite (`../product-direction.md`). Nothing is translated line by line (D13); "Keep" means the behaviour is reimplemented. Target crates: `workspace-layout.md`.

| Legacy module | Disposition | Target |
|---|---|---|
| `exe/TranslationAggregator.cpp` window/layout/docking code, `TranslationWindow`, toolbar/rebar, RichEdit | Replace (iced GUI in classic Win32 style, D14) | `ta-gui` |
| `exe/TranslationAggregator.cpp` clipboard viewer chain, hotkeys | Modernize (`AddClipboardFormatListener`) | `ta-ingest`, `ta-platform` |
| `exe/TranslationAggregator.cpp` substitution pipeline, `AutoTranslate` | Keep | `ta-substitute`, `ta-translate` |
| `exe/Config.*` INI I/O | Replace (one-time import, then TOML, D16) | `ta-core` |
| `exe/Config.*` plugin loader | Cut (D15) | `ta-plugin-protocol` replaces it |
| `exe/Context.*`, `exe/Filter.*` | Keep (applies to every input source) | `ta-core` / `ta-ingest` |
| `exe/BufferedSocket.*` | Cut (no hook listener in the core) | local input API in `ta-ingest` |
| `exe/History/*` | Keep / Modernize (searchable) | `ta-core` |
| `exe/Dialogs/InjectionDialog.cpp` | Plugin (D6) | `plugins/hook/` |
| `exe/Dialogs/MyToolTip.cpp` | Modernize | `ta-gui` |
| `HttpWindows/HttpWindow.*` + 14 services | Replace (declarative connector, official-API presets, D10/D17) | `ta-provider-web` |
| `LocalWindows/AtlasWindow`, `LECWindow`, `Shared/Atlas.*`, `Shared/TextRuleSet.*` | Cut (D7) | — |
| `LocalWindows/FuriganaWindow`, `MecabWindow`, `JParseWindow` | Keep / Modernize | `ta-gui` (views), `ta-morph`, `ta-dict` |
| `util/Dictionary.*`, `util/DictionaryUtil.cpp` | Keep | `ta-dict` |
| `util/Mecab.cpp` | Keep (optional same-bitness FFI backend; pure-Rust default) | `ta-morph` |
| `util/HttpUtil.cpp`, `Shared/StringUtil.*`, `Shared/EUCJPtoUTF16.cpp`, `Shared/Shrink.h` `unify()` | Keep (reimplemented; encodings via `encoding_rs`) | `ta-text` |
| `util/Value.*` | Replace (`serde_json` / own conjugation format) | `ta-dict` |
| `util/Injector.cpp`, `Shared/DllInjection.*`, `Shared/ProcessUtil.*`, `Shared/InjectionSettings.h` | Plugin (D6) | `plugins/hook/` |
| `Shared/HookEval.*`, `Shared/TextHookParser.*` | Plugin (D6) | `plugins/hook/ta-hookcode` |
| `dll/` (hook DLL) | Plugin (D6), built per bitness; `WindowTranslator.*` Cut (D7); `GetText.*` Cut (dead) | `plugins/hook/ta-hook-dll`, `ta-hook-engine` |
| `Shared/Thread.*`, `Lock`, `ReadWriteLock`, `scoped_ptr.h` | Replace (std / tokio) | — |
| `dictionaries/Conjugations.txt` | Not shipped (legacy-authored data, D13); own rules instead | `ta-dict` data |
| `Sample Dll/`, `Text Replacement Hook (Not used)/`, `Miscellaneous/`, `3rdParty/*`, `Shared/ConversionTable.*`, `util/Regexp.cpp` | Cut | — |
