# Architecture of the legacy C++ application

All paths are relative to `Old-Codebase/`. Line numbers refer to the snapshot analysed.

This document describes the **legacy** app as it is. The rewrite follows `../product-direction.md`; the short "Rewrite disposition" notes below say what happens to each part.

> **Rewrite disposition (summary).** Clipboard input, context buffering/filters, substitutions, history, JParser and MeCab: **Keep** in the core, reimplemented from behaviour (D13). Local input API: **New** core input (D5). Hook DLL, injector, exe↔DLL TCP protocol, launch profiles: **move to the hook plugin** (`plugins/hook/`, D6), which talks to the app through the local input API instead of this protocol. ATLAS, LEC and the ATLAS menu/dialog translation thread: **Cut** (D7). Web panes (`HttpWindow` subclasses): **Replaced** by the declarative web connector (D10). `__stdcall` plugin DLLs: **Cut**, replaced by the out-of-process plugin protocol (D15). Win32 window classes, RichEdit, rebar/toolbar: **Replaced** by an `iced` GUI styled like classic Win32 (D14, see `ui-reference.md`).

## 1. Deliverables and process boundaries

| Artifact | Built by | Bitness | Runs in | Source |
|---|---|---|---|---|
| `TranslationAggregator.exe` | `CMakeLists.txt` (MinGW GCC/Clang, CI: `.github/workflows/ci.yml`) | **32-bit only** (CI triplet `i686-w64-mingw32`) | its own process | `exe/`, `Shared/`, `3rdParty/zlib` |
| `App Translator.dll` (release) / `App Translator debug.dll` | **Only** `dll/Translation Aggregator dll.vcxproj` (MSVC v140, `Win32` platform only). **Not built by CMake or CI.** | 32-bit only (MSVC `__asm`, `__declspec(naked)`) | injected into the *game* process | `dll/`, subset of `Shared/` |
| Plugin DLLs (`plugins\*.dll`) | third party; sample in `Sample Dll/` (VS2008 `.vcproj`) | must match exe (32-bit) | loaded into the exe | `Shared/TAPlugin.h` |
| `AGTH.exe` + its dlls | external, not in repo | 32-bit | separate process, launched by exe | `exe/util/Injector.cpp:109-125` |
| ATLAS V13/V14, LEC Power Translator/Nova, MeCab | external commercial / third-party DLLs | 32-bit (assumed; see risks) | loaded into the exe **and** (ATLAS) into the game via the DLL | `Shared/Atlas.cpp`, `exe/TranslationWindows/LocalWindows/LECWindow.cpp`, `exe/util/Mecab.cpp` |

Consequence: the CMake-built release archive (`CMakeLists.txt:142-150` packs exe + `docs` + `dictionaries` + `Plugins`) contains **no hook DLL**. `InjectDll()` checks for the file and fails if missing (`Shared/DllInjection.cpp:111`). Even an old DLL from another build would be rejected at handshake, because the handshake string embeds the version (§3). So, judging only by the build files here, internal hooking and menu translation do not work in the current CI release. The owner should confirm this against a real release.

```
                +-------------------------- TranslationAggregator.exe (i686) ---------------------------+
 clipboard ---> | Drag window "hWndSuperMaster" (hidden, layered)                                        |
 (viewer chain) |   - clipboard viewer, TCP listener (WSAAsyncSelect), 200 ms timer, global hotkeys      |
                |   - owns ContextList (Context.cpp) and translation dispatch (TranslationAggregator.cpp)|
                |                                                                                        |
                | MasterWindow[0..20]  (top-level frames, layouts, transparency, docking)                 |
                |   └─ TranslationWindow children ("panes"): Original Text, ATLAS, LEC, Google, ...       |
                |        HttpWindow ──WinHTTP async──> translation web sites                               |
                |        Atlas/LEC/Mecab/JParser ──Thread worker──> in-process engines / dictionaries      |
                |   Plugins (plugins\*.dll)  ── pre/post substitution hooks                               |
                +--------------------------------------------^-------------------------------------------+
                       CreateRemoteThread(LoadLibraryW)       |  TCP 127.0.0.1:<port>, port published via
                       + CreateProcess(AppLocale env)          |  named file mapping "<MAIN_WINDOW_TITLE>"
                                  v                            |
                +----------------- game.exe (i686) -----------------------------+
                |  App Translator.dll                                           |
                |   DllMain: suspend threads, fetch settings over TCP,          |
                |   IAT patches (menus/MessageBox/DBCS), inline hooks (GDI text) |
                |   TextHook -> "context\ntext" frames -> TCP                    |
                |   ThreadProc: ATLAS-translate menus/dialogs every 500 ms       |
                |   CreateProcess post-hook -> inject into children              |
                +---------------------------------------------------------------+
```

## 2. Exe internals

### 2.1 Windows and message loop
- `wWinMain` (`exe/TranslationAggregator.cpp:1920-2075`): `WSAStartup`; single-instance probe via `CreateFileMapping(..., MAIN_WINDOW_TITLE)`; handles `/ATLAS:DICTSEARCH` and `/I:<exe path>` (a second instance does the injection itself and then exits, `:1956-1962`, `CommandLineInject` `:103-125`); registers three window classes (`Register()` `:1843-1897`); `LoadLibraryA("Msftedit.dll")` for RichEdit 4.1; COM STA init.
- **Drag window / "super master"** (`TwigDragWindowProc` `:1224-1436`): a hidden layered popup. It is reused as the light-blue drop-target overlay while a pane is dragged, and it is also the app's **controller**. It handles socket events (`WMA_SOCKET_MESSAGE`), the clipboard viewer chain (`SetClipboardViewer`, `WM_DRAWCLIPBOARD`, `WM_CHANGECBCHAIN`), the animation/flush timer (id 1, 200–300 ms), global hotkeys (`RegisterHotKey`, `:67-89`), history back/forward and the `WMA_TRANSLATE*` commands.
- **MasterWindow** (struct in the same file, `:453-1217`): a top-level frame that holds up to 20 panes in 1 or 2 columns. It stores splitter positions in units of `RANGE_MAX = 100000` and supports alpha (`WS_EX_LAYERED`), frame/no-frame, topmost, toolbar visibility and lock order. Panes can be dragged out into new frames or docked into others (`WMA_DRAGSTART`, `WM_MOVING` + `EnumWindows` hit test `FindInsertionRect` `:873-948`). Layouts 0–10 are saved to the INI (`SaveLayout`/`LoadLayout` `:1024-1216`).
- **TranslationWindow** (`exe/TranslationWindows/TranslationWindow.{h,cpp}`): the base class for every pane. It has an optional rebar and toolbar, and a RichEdit (`MSFTEDIT_CLASS`) or custom child window. Flags are `TWF_SEPARATOR`, `TWF_CONFIG_BUTTON`, `TWF_RICH_TEXT` and `TWF_NO_EDIT`. Per-pane INI state is `[<windowType>] Auto Clipboard`.
- Message loop: one accelerator table is shared by all panes (`:2043-2063`).

### 2.2 Translation pipeline (exe)
1. **Input sources**
   - Typing in the *Original Text* pane, then the toolbar/accelerator triggers `WMA_TRANSLATE_ALL` or `WMA_TRANSLATE`, which call `TranslateAll()` / `Translate(w)` (`:298-320`).
   - Clipboard change (`WM_DRAWCLIPBOARD`). If our own editors own the clipboard, it does `WMA_AUTO_COPY` (re-posts the text as plain CF_UNICODETEXT). Otherwise `WMA_AUTO_TRANSLATE_CLIPBOARD` feeds `Contexts.AddText(L"::System", 0, L"Clipboard", text)` (`:1358-1377`), so clipboard text goes through the same context/filter pipeline as hooked text.
   - Hooked text from the DLL over TCP: `BufferedSocket::ReadData` calls `Contexts.AddText(process, context, text)` (`exe/BufferedSocket.cpp:169-179`).
   - History navigation (`WMA_BACK/FORWARD`, `HistoryTranslate` `:360-382`), which only uses cached translations (`only_use_history=true`).
2. **Context buffering** (`exe/Context.cpp`). Text is accumulated per (process, context name). A context is "flushed" when it has been idle longer than `internalHookTime` ms (default 250 for games, 0 for `::System`; `exe/Config.cpp:1144-1147`), checked from the 200 ms timer (`ContextList::CheckDone` `:305-315`). Text-loop detection (`CheckContext` `:391-512`) and repeat filters (`exe/Filter.cpp`, `AutoFilter`) run at flush time. Flushing can merge into `::Merged`, copy to the clipboard, and/or `SendMessage(hWndSuperMaster, WMA_AUTO_TRANSLATE_CONTEXT)` (`:339-346`). History is capped by `maxLogLen` KB.
3. **AutoTranslate** (`exe/TranslationAggregator.cpp:384-412`): `history.AddEntry`, then `ProcessSubstitutions`, then each pane with `autoClipboard` set is called. Remote panes skip strings of 500 characters or more.
4. **ProcessSubstitutions** (`:143-296`) applies these steps in order:
   1. half-width katakana to full width (`LCMapStringW`, if `AutoHalfWidthToFull`)
   2. optional romaji to hiragana (`CONFIG_ENABLE_AUTO_HIRAGANA`)
   3. plugins `ActiveProfileList` and `ModifyStringPreSubstitution`
   4. user substitutions (longest match across the active profiles, `ReplaceList::FindReplacement`, `exe/Config.cpp:520-589`). Profiles are active when a process whose `folder\exe` matches the profile name is running (`FindActiveAndPopulateCB` enumerates *all* processes, `exe/Config.cpp:316-397`). This runs on every translation.
   5. plugins `ModifyStringPostSubstitution`
5. **Per-pane translation** (`TranslationWindow::Translate(orig, mod, id, onlyHistory)`). Each pane runs at most one job at a time. Remote panes queue the latest request; local panes abort and restart (README).
6. **Display**: `SetWindowTextW` on the pane's edit. For JParser and MeCab, a custom-painted furigana view (`FuriganaWindow.cpp`) with tooltips (`exe/Dialogs/MyToolTip.cpp`). Results are stored in `History` keyed by (entry id, translator id).

### 2.3 Threading model
| Thread | Work | Back to UI via |
|---|---|---|
| UI thread | all windows, sockets (`WSAAsyncSelect`), context buffering, substitutions, plugin calls, **synchronous token/cookie fetches** for Bing/Baidu/SysTran/Yandex (e.g. `HttpWindow::GetCookie` blocks up to 2×5 s, `exe/TranslationWindows/HttpWindows/HttpWindow.cpp:597-653`) | — |
| WinHTTP thread pool | async callbacks `HttpCallback` (`HttpWindow.cpp:258-307`) append to buffer | `PostMessage(WMA_COMPLETE_REQUEST / WMA_CLEANUP_REQUEST)` |
| `Thread` workers (`Shared/Thread.{h,cpp}`): "Atlas", "LEC", plus MeCab/JParser tasks | engine calls, dictionary lookup (guarded by `ReadWriteLock dictionary_read_write_lock`, `exe/util/Dictionary.cpp:15`) | Worker calls `SetWindowTextW(hWndEdit)` **directly cross-thread** (e.g. `LECWindow.cpp:79-84`, `AtlasWindow.cpp:68-72`), then `PostMessage(WMA_THREAD_DONE)` |
| MeCab | not thread-safe; serialized by `mecab_init_lock_` (`exe/util/Mecab.cpp:11`) | — |

Shared mutable globals (`config`, `windows[]`, `history`, `Contexts`) are accessed without locks and assume the UI thread.

## 3. IPC: exe <-> hook DLL

Transport: TCP over loopback, **no authentication beyond loopback-only accept** (`ScreenIP` `exe/BufferedSocket.cpp:319-325`).

1. Exe: binds `127.0.0.1:port+i` for `i` in 0..19 (`StartListen` `:256-310`, default `Port=56393`). It writes the chosen `u16` port into a 2-byte named file mapping called `MAIN_WINDOW_TITLE` = `"Translation Aggregator " GIT_TAG " (Unofficial)"` (`Shared/Shrink.h:51-60`).
2. DLL (`dll/Translation Aggregator dll.cpp:440-492`): opens the same mapping name (it must already exist), reads the port and connects.
3. Framing in both directions: `u32 little-endian byte length` + payload.
4. Handshake: each side sends `MAIN_WINDOW_TITLE` as UTF-16LE including the NUL, and each side verifies the other's. A version mismatch closes the socket.
5. DLL to exe: `"<LauncherName>*<pid>"` (UTF-16, NUL). `LauncherName` is the env var `__TA_LAUNCHER_NAME` if set (for injected children), else the module path.
6. Exe to DLL: raw `InjectionSettings` struct (`Shared/InjectionSettings.h`). The DLL rejects it unless `len == sizeof(InjectionSettings)` (`dll/...dll.cpp:483`). The struct contains two raw pointers, so its size depends on bitness (2648 bytes on i686 by my calculation, see `formats-and-interfaces.md`). Next comes the user hook-code list (UTF-16, NUL-terminated, whitespace-separated). The exe rewrites `atlasConfig.trsPath` to `"<exe dir>\Rule Sets\<name>"` before sending (`BufferedSocket.cpp:153-160`).
7. DLL to exe, steady state: `"<context>\n<text>"` UTF-16 NUL-terminated per hook hit (`dll/Hooks/TextHooks.cpp:299-304`). It is sent **synchronously on the game thread** inside a critical section. The DLL reconnects at most once a second on failure (`SendAll(..., tryConnectOnFail=1)`).

Context name format: `<alias or full hook text>::<hex subcontext>[;<hex subcontext>...]`, where the default subcontext is `_0`, the return address (`TextHooks.cpp:173-185`, `Shared/TextHookParser.cpp:51-56`).

> **Rewrite disposition:** this protocol is not kept. The core has no hook listener; the hook plugin posts captured text to the local input API with a context id (process/thread/hook). The idea of a per-source context name survives as that context id.

## 4. Hook DLL internals (`dll/`)

`DllMain(DLL_PROCESS_ATTACH)` (`dll/Translation Aggregator dll.cpp:505-564`) does all of the following **under the loader lock**:
1. `SuspendThreads()` (every other thread in the process, `dll/Hooks/HookManager.cpp:385-412`)
2. `WSAStartup`
3. TCP handshake (§3)
4. `SetLogFile`
5. `Init()`: IAT patching of every module loaded from the game folder (`OverrideFunctions` `:223-249`): `GetProcAddress`, `LoadLibrary(Ex)`, `IsDBCSLeadByte(Ex)`, `GetOEMCP` (DCBS override), `MessageBoxA/W`, `TrackPopupMenu(Ex)` (menu translation)
6. optional `ThreadProc` (ATLAS translation of windows/menus every 500 ms, re-IAT-patch every 10 s, `:316-376`)
7. inline "after" hooks on `CreateProcessA/W` to inject children (`:549-552`)
8. `HookTextFunctions` (`dll/Hooks/TextHooks.cpp:339-394`), which installs default hooks on `MultiByteToWideChar`, `WideCharToMultiByte`, `DrawText(Ex)A/W`, `(Ext)TextOutA/W`, `GetGlyphOutlineA/W`, `GetTextExtentPoint32A/W` and `CharNext(Ex)A`, plus user hook codes
9. resume threads

Inline hook engine (`dll/Hooks/HookManager.cpp`):
- Uses the Z0mbie length disassembler (`dll/Hooks/disasm.cpp`) to steal at least 5 bytes, and relocates rel8/rel32 jumps, calls and jcc (`IsRelativeJump` `:125-190`).
- Writes `JMP rel32` to a stub in an RWX page.
- The stub is copied from MSVC naked functions. `PreCallStub` does `pushad`/`pushfd`, calls `HookFxn(Registers*, param, id)`, restores and jumps to the relocated original. `PostStdCallStub` is a "call original, then call hook with the return value prepended", stdcall only.
- The stubs are found at runtime by scanning for the `0xDEADBEEF` sentinel.

This is x86-32 only by construction: `pushad`, 32-bit `Registers` (`Shared/HookEval.h:3-14`), 5-byte relative jumps, `__asm`.

> **Rewrite disposition:** moves to the hook plugin (D6), which builds a hook DLL per bitness. The ATLAS `ThreadProc` (step 6) and the menu/MessageBox IAT patches that only serve it are **Cut** (D7).

`TextHook` (`dll/Hooks/TextHooks.cpp:137-309`):
- Recursion guard by return address.
- Default filter: ignores callers inside "system" DLLs, meaning modules outside the game's folder (`AddressIsSystemDll`). The module list is refreshed by a Toolhelp snapshot, delayed `internalHookDelay` seconds after start.
- Evaluates the compiled hook-code expressions (value, contexts, length, code page) against the registers.
- Reads the string (`IsBadReadPtr` guarded) and converts with `MultiByteToWideChar(codePage)`. Supports byte-flipped ("BE") variants.

## 5. Data flow summary (capture to display)

```
game GDI call ─inline hook─> TextHook ─TCP frame─> BufferedSocket ─> ContextList.AddText
clipboard ────────────────────────────────────────────────────────> ContextList.AddText("::System","Clipboard")
      ContextList (per context buffer, idle timer, loop/repeat filters, merge) ── flush ──>
      WMA_AUTO_TRANSLATE_CONTEXT ─> AutoTranslate ─> History.AddEntry ─> ProcessSubstitutions
      ─> for each pane with Auto Clipboard: Translate()
           HttpWindow: History hit? else WinHTTP request ─> FindTranslatedText/StripResponse ─> History.SetTranslation ─> SetWindowText
           ATLAS/LEC: worker thread ─> engine DLL ─> SetWindowText
           MeCab/JParser: worker ─> libmecab / edict .bin dictionaries ─> custom painted furigana + tooltips
```

## 6. Notable architectural quirks a rewrite should not copy blindly

Rewrite disposition: none of these are reproduced (D20: legacy quirks are fixed; every behaviour change is documented).

- **CWD-relative paths.** The INI path comes from `GetCurrentDirectoryW` (`exe/Config.cpp:850-864`). `dictionaries\`, `Game Configs\`, `plugins\` and `Dictionaries\UserDict.txt` are CWD-relative too, but `Rule Sets\` and `Plugins\Nova` are relative to the module path. If TA is launched from a shortcut with a different working directory, it reads a different INI.
- The handshake string is the version string, so any version bump makes the exe and DLL incompatible. This is intentional (`Shared/Shrink.h:65-66`).
- `Shrink.h` redefines `wcscmp`, `wcsicmp` and similar via `CompareStringW(LOCALE_INVARIANT, …)` (`Shared/Shrink.h:104-132`). Sort orders for substitutions and dictionaries depend on this.
- Bugs observed while reading (parity tests should decide whether to keep or fix them):
  - `BufferedSocket::SendData` passes `outBufferLen` rather than `outBufferLen - sent` to `send` (`exe/BufferedSocket.cpp:209`).
  - `LoadInjectionContextSettings` does `memset(settings, 0, sizeof(settings))` on a pointer, which clears only 4 bytes (`exe/Config.cpp:1192`).
  - `#ifdef SETSUMI_CHANEGS` typo in the `.rc` accelerators (`exe/TranslationAggregator.res.rc`).
  - The fall-through from `WMA_AUTO_TRANSLATE_CONTEXT` into `WMA_AUTO_TRANSLATE_CLIPBOARD` when `srcWindow->autoClipboard` is 0 leaks the `SharedString` reference (`exe/TranslationAggregator.cpp:1345-1358`).
