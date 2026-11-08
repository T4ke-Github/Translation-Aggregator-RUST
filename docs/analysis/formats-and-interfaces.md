# External formats and interfaces

Everything a rewrite must read, write, load, or interoperate with. "Compat: MUST" means existing user data or third-party binaries depend on the exact format. "SHOULD" = convenient to keep. "FREE" = internal cache, redesignable.

The Compat ratings describe the legacy app's needs. Each section also has a **Rewrite disposition** that follows `../product-direction.md`. Under D13 the rewrite reads legacy formats from their documented behaviour (clean reimplementation) and ships no legacy-authored data files.

## 1. Main INI: `TranslationAggregator.ini` (Compat: MUST to migrate existing users)

- Location: the current working directory (`exe/Config.cpp:850-864`). A legacy misspelled name is migrated on start.
- Read/written with Win32 `Get/WritePrivateProfile*W`. The sample file is ASCII; non-ASCII values are stored as hex blobs via `WritePrivateProfileStruct` alongside the text key.

### Sections and keys (from code)

| Section | Keys (defaults) | Source |
|---|---|---|
| `[General]` | `Port` (56393); `Flags` (bit0 substitutions, bit1 auto-hiragana, bit2 context full history); `Font Face`/`Font Height` (1/15 pt: `lfHeight=-h/15`)/`Font Color`/`Bold`/`Italic`/`Underline`/`StrikeOut`; `Tooltip Font *` (same 7); `AutoHalfWidthToFull`; `toolTipKanji/Hiragana/Paren/Conj` (COLORREF); `GlobalHotkeys`; `langSrc`/`langDst` (English names, e.g. `Japanese`, `Chinese (Simplified)`) | `Config.cpp:945-1072` |
| `[JParser]` | `HideCrossRefs`, `HideUsage`, `HidePos`, `DefinitionLines`, `ReformatNumbers`, `NoKanaBrackets`, `JParserFlags` (0x11 default) | same |
| `[<pane>]` | `Auto Clipboard` | `TranslationWindow.cpp:22-27` |
| `[Original Text]` | `Trans Highlighted` | `UntranslatedWindow.cpp:9-14` |
| `[JParser]`/`[Mecab]` | `BK Color`, `Furigana BK Color`, `Particle BK Color`, `Trans BK Color`, `Normal Font Size`, `Furigana Font Size`, `Character Set` (0 hira/1 kata/2 romaji/3 none), JParser `Flags` | `FuriganaWindow.cpp:36-86` |
| `[ATLAS]` | `Environment` | `AtlasWindow.cpp:109` |
| `[WWWJDIC]` | `Mirror` | `JdicWindow.cpp:16` |
| `[Injection]` | `Last Launched`, `Last Launched Struct` | `Injector.cpp:61-71` |
| `[Layout N]` N=0..10 | `Count`, `Window i` | `TranslationAggregator.cpp:1024-1216` |

### Layout line format (Compat: SHOULD)
`Window i` = `name1,name2,...; numCols, topmost, showFrame, showToolbars, alpha, colPlacement, left, top, width, height, lockWindows; rowPlacement0, rowPlacement1, ...`
Positions are integers out of `RANGE_MAX = 100000`. Pane names are the `windowType` strings. The `SETSUMI_CHANGES` build appends 9 extra colour fields (not in shipped builds). Example in `exe/TranslationAggregator.ini`.

> **Rewrite disposition:** **Import once, then TOML** (D16). The importer maps `[General]`, `[JParser]`, `[Mecab]`, per-pane `Auto Clipboard`, `[Original Text]` and `[Layout N]`. `[ATLAS]` is ignored (D7), `[Injection]` belongs to the hook plugin (D6), `[WWWJDIC] Mirror` only matters if a user recreates WWWJDIC as a connector definition. `Port` is not imported (the hook listener is gone; the local API has its own setting). Pane names of cut panes are dropped from imported layouts.

## 2. Per-game config: `Game Configs\<folder+exe>.ini` (Compat: SHOULD)
- Path from `GetConfigPath` (`Config.cpp:1074-1093`): backslashes in the profile name become `+`, suffix `.ini`.
- `[Launch] Language` (LCID, default 0x0411 Japanese); `[Atlas] Break On`/`Environment`/`Rule Set`; `[Injection] Exe Path`(+`Struct`), `AGTH params`, `Mode` (injection flags bitset), `AGTH Flags`, `AGTH Repeat`, `AGTH KF 1/2`, `Hook Delay`, `Hook Time`, `Log Len`, `Internal Hooks` (space-separated hook codes); `[Contexts] <contextName>` = a `ContextSettings` struct blob.
- Injection-flag and AGTH-flag bit meanings: `Shared/InjectionSettings.h:5-20`.
- `ContextSettings` is a bit-packed struct with a `version` (`CONTEXT_SETTINGS_VERSION=3`) persisted as a raw `WritePrivateProfileStruct` blob (`Config.h:288-307`). A rewrite that changes struct layout must version-gate it (the loader already discards mismatched versions, `Config.cpp:1190`).

> **Rewrite disposition:** `[Launch]` and `[Injection]` move to the hook plugin's launch profiles (D6); `[Atlas]` is **Cut** (D7). `[Contexts]` blobs may be imported once into the core's per-context settings (D16) or dropped; the core stores its own TOML.

## 3. Substitution list: `Dictionaries\UserDict.txt` (Compat: MUST)
- UTF-16LE, BOM `0xFEFF`, LF line endings (`Config.cpp:434-518`).
- Blocks: a line `Profile:<name>` then pairs of (match line, replacement line), blank-line separated. `\n`, `\t`, `\\` are escaped. `*` is the global profile. Longest-match wins across active profiles.

> **Rewrite disposition:** **Keep the substitution feature**; import `UserDict.txt` once and store TOML afterwards (D16). An export back to `UserDict.txt` is optional.

## 4. Plugin ABI: `plugins\*.dll` (Compat: MUST if third-party plugins are to keep working)
- Header `Shared/TAPlugin.h`. `TA_PLUGIN_VERSION = 2`. All exports are `__stdcall`, C linkage, **32-bit**.
- Required: `unsigned int TAPluginGetVersion(const void*)`.
- Optional: `int TAPluginInitialize(const TAInfo*, void**)`; `wchar_t* TAPluginModifyStringPreSubstitution(const wchar_t*)`; `wchar_t* ...PostSubstitution(const wchar_t*)`; `void TAPluginActiveProfileList(int, const wchar_t**)`; `void TAPluginFree(void*)`.
- Loader: `Config.cpp:891-936`. `TAInfo{ size, version }`; version from `APP_VERSION` as `(major<<16)|(minor<<8)|patch`. Plugins reporting version 0 or > 2 are rejected. Strings returned by a plugin are freed with that plugin's `TAPluginFree` after both pre/post run.
- `Sample Dll/` is an example only and does not exactly match the header (uses non-const `wchar_t*`); treat `TAPlugin.h` as authoritative.

> **Rewrite disposition:** **Cut** (D15). Text-transform plugins use the new out-of-process protocol (pre/post substitution hooks are methods of that protocol).

## 5. Exe <-> hook DLL wire protocol (Compat: only matters if reusing the existing DLL; a from-scratch rewrite controls both ends)
- Loopback TCP, length-prefixed frames (`u32` LE byte length + payload). Port is published in a 2-byte named shared-memory section whose name equals the version-stamped window title, so exe and DLL must share the exact version string (`Shared/Shrink.h:51-66`).
- Handshake payload is the window-title string (UTF-16, NUL-included), verified both ways.
- Settings frame exe->DLL is the raw `InjectionSettings` struct; the DLL requires `len == sizeof(InjectionSettings)`. **The struct embeds two raw pointers (`exeName`, `exeNamePlusFolder`), so its size and layout are bitness- and compiler-dependent** — it is not a portable wire format and a rewrite should define an explicit serialized layout if it keeps any IPC.
- Steady-state frame DLL->exe is `"<context>\n<text>"` (UTF-16, NUL). Context name grammar: `<alias|full hook text>::<hexSub>[;<hexSub>...]`.

> **Rewrite disposition:** **Replaced.** The core has no hook protocol. The hook plugin owns the link to its hook DLLs (internal to `plugins/hook/`) and forwards text to the core through the local input API (D6).

## 6. Hook-code language (Compat: MUST — users share these codes; also AGTH `/H` compatibility)
- Full spec: `docs/Hook Codes.txt`. Parser `Shared/TextHookParser.cpp`; expression compiler/evaluator `Shared/HookEval.{h,cpp}`.
- Form: `[alias]type[be][*]:value:subcontext(s):length:charset@address:module:function`.
- Types: `char`, `wchar`, `UTF8`, `UTF16`, `SJIS` (+ `be` byte-flip, `*` pointer). Expressions over the 8 x86 GP registers, `[...]` dereference, `_X` = `[ESP+X]`, operators `+ - * / % << >> ^ | & ! ~` and parentheses. Address in hex; module/function optional. The parser is 32-bit-register specific.
- `InjectionDialog.cpp:576-770` converts AGTH `/H{A,S,W,Q,B}` codes to this syntax.

> **Rewrite disposition:** **Moves to the hook plugin** (D6). The plugin accepts the existing 32-bit syntax (reimplemented from `docs/Hook Codes.txt`, D13) and defines the 64-bit extension itself.

## 7. Translation dictionaries (`dictionaries\`)
- **Source dictionaries (Compat: MUST load unchanged):** any EDICT/EDICT2/ENAMDICT file, optionally `.gz` (zlib). `LoadFile` sniffs UTF-16 (BOM or heuristic), UTF-8 BOM, EUC-JP and Shift-JIS leading bytes, else UTF-8 (`StringUtil.cpp:683-810`). `/EntL...` tags are stripped. Multi-dictionary. Non-`.txt` extensions trigger `.bin` compilation (ChangeLog r189).
- **Compiled cache `<name>.bin` (FREE):** a private memory-mapped format with header `magic 'TCD\0'`, `version 0x000C`, source and conjugation-table file signatures (size/mtime/ctime), `numJStrings`, flags; then a sorted `u32` jstring-offset table and a string pool (`Dictionary.cpp:36-187,499-522`). Regenerated when source or `Conjugations.txt` changes. A rewrite can invent its own cache.
- **`Conjugations.txt` (Compat: MUST — shipped data):** UTF-16LE, a JSON-ish list parsed by the in-house `Value` parser (`util/Value.cpp`). Each entry: `Name`, `Part of Speech` (`Verb`/`Adj`), `Tenses[]` with `Formal`, `Negative`, `Suffix`, `Tense`, optional `Next Type` (`Dictionary.cpp:584-691`). Tense names partly fixed (`staticTenses`, `:107-131`). `Conjugations_old.txt` is only input to the unused `Miscellaneous/CompileConjugations.cpp` tool.

> **Rewrite disposition:** source dictionaries: **Keep** (the user downloads EDICT/ENAMDICT, or it is shipped with its own licence and attribution, D13). `.bin` cache: **Replaced** by a new private cache. `Conjugations.txt`: **not shipped** (legacy-authored data, D13); the rewrite ships its own conjugation rules written from grammar sources, in its own format. Reading a user-provided legacy file is optional.

## 8. ATLAS rule sets: `Rule Sets\*.trs` (Compat: SHOULD)
- Text files, UTF-16/UTF-8/SJIS auto-detected (`TextRuleSet.cpp:5-67`). Directives: `include <file>`, `break before`/`break after`/`break start before` (optionally prefixed `line `), `replace X with Y`, `//` comments. Used before ATLAS to split/rewrite sentences.

> **Rewrite disposition:** **Cut** (D7). Not read, not shipped.

## 9. External engine DLLs loaded at runtime (Compat: MUST match their ABIs — all 32-bit)
| Engine | Discovery | Entry points used |
|---|---|---|
| Fujitsu ATLAS V13/V14 | registry `HKCU\Software\Fujitsu\ATLAS\V{14,13}.0\EJ` `TRENV EJ`, or `%ProgramFiles%\ATLAS V{14,13}\`; loads `AtleCont.dll`, `awdict.dll`, `awuenv.dll` | `CreateEngine`, `DestroyEngine`, `TranslatePair`, `FreeAtlasData`, `AtlInitEngineData`, `SetTransState`, `AwuWordDel` — all `__cdecl`, Shift-JIS in/out (`Shared/Atlas.cpp:168-280`) |
| LEC Power Translator / Nova | `Plugins\Nova\JaEn\EngineDll_je.dll`, or registry `HKLM\SOFTWARE\LogoMedia\LEC Power Translator 15\Configuration` `ApplicationPath` | `eg_init`/`eg_init2`, `eg_end`, `eg_translate_multi` — `__cdecl`, code page 932 (`LECWindow.{h,cpp}`) |
| MeCab | registry `Software\Mecab` (HKCU then HKLM) `mecabrc`, or `%ProgramFiles%\Mecab\bin\`, or local `libmecab.dll` | `mecab_new2`, `mecab_destroy`, `mecab_sparse_tostr`, `mecab_dictionary_info` — `__cdecl`; dictionary charset must be SJIS/UTF-8/EUC-JP, **not** UTF-16 (`util/Mecab.cpp`) |

> **Rewrite disposition:** ATLAS and LEC: **Cut** (D7), so no 32-bit engine host. MeCab: **Keep** (D9) via a pure-Rust analyzer that reads MeCab-format dictionaries by default; loading an installed `libmecab.dll` of the **same bitness** as the app stays an optional backend.

## 10. Web service endpoints (Compat: none — all unofficial, fragile, several already dead)
Each pane builds a provider-specific request and scrapes the response (`exe/TranslationWindows/HttpWindows/*`). Several embed hard-coded build identifiers, header values, or token/cookie bootstrap flows that are harvested at runtime from the provider's own site; these break whenever the provider changes. A rewrite should treat every one as needing fresh verification and, where possible, replace scraping with official APIs.

Known-defunct or unreliable per the ChangeLog and factory comments: Honyaku (removed), FreeTranslation (dead server), LEC Online (out of translations), Babel Fish / Babylon / Infoseek / SysTran / SDL (all unofficial scrapes of uncertain current status). Google/Bing/DeepL/Yandex/Baidu depend on pinned internal endpoint details that are likely stale as of this snapshot (2021-era identifiers) and must be re-derived. Do not assume any endpoint still works without testing.

> **Rewrite disposition:** **Replaced by the web connector** (D10). None of these endpoints are carried over. Built-in presets use official APIs only (D17: DeepL API, Google Cloud Translation, Microsoft Translator, LibreTranslate). The keyless Google and Bing web versions ship as disabled example definitions marked unofficial, with a setup guide (D21); they are written fresh against today's endpoints, not taken from the legacy code (D13). Per-pane mapping: `features.md` §1.

## 11. Other external touchpoints
- AppLocale launch: sets env `__COMPAT_LAYER=#ApplicationLocale` and `AppLocaleID=<hex LCID>` before `CreateProcess`/`ShellExecute` (`DllInjection.cpp:133-164`, `ProcessUtil.cpp`). AppLocale is a deprecated Microsoft component; on modern Windows this is effectively a no-op.
- AGTH: launched as `AGTH.exe /P<pid> [/C /KS<n> /KF<a>:<b> <params>]` (`Injector.cpp:9-21,109-125`). Not shipped in this repo.
- Shortcuts: `.lnk` created via `IShellLink`/`IPersistFile` with argument `/I:<exe path>` (`InjectionDialog.cpp:940-1000`).
- Browser: "Websites" menu runs `rundll32 url.dll,FileProtocolHandler <srcUrl>` (`TranslationAggregator.cpp:1711-1712`).
- RichEdit 4.1 via `Msftedit.dll` / `MSFTEDIT_CLASS`.

> **Rewrite disposition:** AppLocale launch and `.lnk` shortcuts: **move to the hook plugin**. AGTH: **Cut** (external hookers use clipboard or the local API). Browser link: **Keep** (opens provider docs/console). RichEdit: **Replaced** by `iced` text widgets (D14).
