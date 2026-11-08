# Legacy UI reference

What the legacy UI looks like and how it behaves, as input for the UI design step (D11). This is a description, not a screen design. Under D13 the rewrite rebuilds the UI from this behaviour; no resource scripts or window code are carried over. Paths are relative to `Old-Codebase/`; `TA.cpp` below means `exe/TranslationAggregator.cpp`, `.rc` means `exe/TranslationAggregator.res.rc`, `TW.cpp` means `exe/TranslationWindows/TranslationWindow.cpp`.

Toolkit: **`iced`** (D14), styled like classic Win32. §11 covers what that means and where iced has gaps.

## 1. Big picture

- The app is a set of **frames** (top-level windows) that each hold **panes** stacked in one or two columns. Each pane is either an input pane ("Original Text") or the output of one translator/tool.
- Up to 20 frames (`TA.cpp:456`), up to 20 panes per frame (`TA.cpp:482`). Every pane exists at most once; it is shown in one frame or hidden.
- First start (no saved layout): one frame holding all panes in two columns (`TA.cpp:1194-1199`; defaults of `CreateMasterWindow`, `TA.cpp:818`: 2 columns, split at 50 %, not topmost, frame and toolbars shown, opaque).
- A hidden controller window does the background work (clipboard, timers, hotkeys) and doubles as the drop indicator when panes are dragged (`architecture.md` §2.1).
- Single instance; a second start with `/I:` hands work to the running one (`architecture.md` §2.1).

## 2. Frames

| Aspect | Behaviour | Code |
|---|---|---|
| Window style | Normal overlapped window with menu bar; "Hide Frame" switches to a borderless resizable popup **without menu bar** | `TA.cpp:544-562,581-588` |
| Background | System 3D-face colour; double-buffered composition (`WS_EX_COMPOSITED`) | `TA.cpp:544,1867` |
| Title | `Translation Aggregator <git tag> (Unofficial)` | `Shared/Shrink.h:51-60` |
| Transparency | Whole-window alpha 20–255, changed in steps of 16 (Ctrl-Num- / Ctrl-Num+) | `TA.cpp:527-540,1629-1634` |
| Always on top | Per frame toggle (Alt-A) | `TA.cpp:510-517,1579-1583` |
| Toolbars | Per frame show/hide of all pane toolbars (Ctrl-T) | `TA.cpp:502-508,1658-1662` |
| Columns | One or two columns (Ctrl-2 toggles) | `TA.cpp:519-525,1653-1657` |
| Lock order | Prevents dragging panes in or out | `TA.cpp:1663-1668,1484,1518,882` |
| Close | Closing a frame saves layout 0; closing the last frame quits | `TA.cpp:1468-1478` |
| Placement on load | Saved positions are clamped to a visible monitor | `TA.cpp:1168-1182,425-451` |
| Drop files | Dropping an `.exe` opens the inject dialog with it (hook feature) | `TA.cpp:575,1759-1761` |
| DPI / visual styles | No DPI-awareness declaration and no application manifest found in the tree, so the app is likely DPI-unaware (bitmap-scaled on high-DPI screens) and drawn with classic common controls (inferred, not run) | grep over the tree |

## 3. Pane layout and splitters

- Two columns: rows = ceil(n/2); with an odd number of panes the **first pane spans the full width** (`TA.cpp:692-699,746-749`).
- Panes are separated by 3-pixel gaps that act as splitters (`TA.cpp:745,753-754`). Positions are stored as fractions in units of 1/100000 (`RANGE_MAX`), so layouts survive resizing; rows keep a minimum height of about 6 px (`TA.cpp:702-736`).
- Splitter hit zone is generous: about ±22 px around a gap (`TA.cpp:1806-1817`); cursor changes to ↔, ↕ or ✥ (`TA.cpp:419-423`). Dragging moves the column split or one row boundary, bounded by its neighbours (`TA.cpp:1762-1801`).
- Double-click on a splitter resets: column back to 50 %, rows back to equal heights (`TA.cpp:1823-1830`).

## 4. Pane anatomy

```
+--------------------------------------------------------------+
| [Google ▸] [☑ auto] | [extra buttons] [⚙]            [ ✕ ]   |  <- rebar: toolbar band + close band
+--------------------------------------------------------------+
| translated text (RichEdit, vertical scrollbar)               |
|                                                              |
+--------------------------------------------------------------+
```

| Element | Behaviour | Code |
|---|---|---|
| Border | Thin static edge around pane and text box | `TW.cpp:212,224` |
| Translate button | Labelled with the **pane name**; acts as the pane title. Click translates this pane; click while busy (remote pane) cancels | `TW.cpp:116,588-594` |
| Busy indicator | The translate button's icon cycles through 7 frames | `TW.cpp:641-658` |
| Auto toggle | "Automatic Clipboard Translation" check button; default on for local panes, off for remote ones | `TW.cpp:22,117,595-601` |
| Pane-specific buttons | Original Text: "Translate All" + "Auto Translate Highlighted Text" toggle; JParser, MeCab, WWWJDIC: Configure | `LocalWindows/UntranslatedWindow.cpp:21-39`, `JParseWindow.cpp:254`, `MecabWindow.cpp:150`, `HttpWindows/JdicWindow.cpp:44`, `TW.cpp:130-135` |
| Close button | Separate band at the right edge; hides the pane (it can be re-shown from the Window menu) | `TW.cpp:140-183,602-606` |
| Toolbar metrics | 19×19 icons from one bitmap strip (16 icons), 21×21 buttons, flat style with tooltips; label font Arial 19 px, weight 500 | `TW.cpp:45-63`, `exe/Config.cpp:1069` |
| Body | RichEdit 4.1, plain text except WWWJDIC (coloured rich text), vertical scrollbar, unlimited length | `TW.cpp:222-231`, `JdicWindow.cpp:102-109,201` |
| Disabled state | Body greyed out when the pane cannot translate the selected language pair | `TW.cpp:666-671` |
| Original Text extras | Auto-translate on mouse-up when "highlighted" mode is on | `UntranslatedWindow.cpp:50-67` |
| Drag to move | Press on an **empty part of the toolbar** and drag: the pane tears out into a new frame that follows the mouse; over another frame, a light-blue half-transparent rectangle (RGB 160,170,235 at alpha 128) shows the drop slot; releasing docks it there | `TW.cpp:449-470`, `TA.cpp:1483-1557,1891,1976-1978` |

Pane context menu (right-click in the text; `TW.cpp:315-445`): Cut, Copy, Paste · To Hiragana (Ctrl-H), To Katakana (Ctrl-K), To Romaji (Ctrl-R) · SJIS to Hex, UTF16 to Hex, Hex to SJIS, Hex to UTF16 · Configure (if the pane has it) · then a copy of the **View** menu. Conversions apply to the selection, or to the whole text if nothing is selected (`TW.cpp:478-585`). The menu does not work fully when the frame is hidden, because the state is copied from the frame's menu bar (`TW.cpp:324`).

## 5. Menus

Static part from `.rc:55-108`; dynamic parts added per frame in `TA.cpp:581-628`.

| Menu | Items (shortcut) | Rewrite |
|---|---|---|
| File | Global Hotkeys (toggle; Ctrl-Alt-key) · Quit (Alt-X) | Keep |
| Tools | Launch Text Hooker (Ctrl-L) · Manage Attached Contexts (Ctrl-M) · Auto Convert Half-Width Katakana · Auto Convert to Hiragana when no Japanese · Enable Substitutions · Substitutions… (Alt-S) · Previous / Next History Entry (Alt-Up/Alt-PgUp, Alt-Down/Alt-PgDn) · **Translate From ▸ / Translate To ▸** (added at runtime: 69 languages, separators after Auto, Japanese and Russian; "To" omits Auto) | "Launch Text Hooker" moves to the hook plugin; rest kept |
| View | Font (Alt-F) · Tooltip Font (Alt-T) · Hide Toolbar (Ctrl-T) · Hide Frame (Ctrl-F) · Always on Top (Alt-A) · More / Less Transparent (Ctrl-Num- / Ctrl-Num+) | Keep |
| Window | Lock Order · Show All (Ctrl-0) · Two Column (Ctrl-2) · then one check item **per pane** to show/hide it | Keep |
| Websites | One item per pane that has a home page; opens it in the browser | Modernize (provider docs/console links) |

- Choosing a source language equal to the target swaps them (and vice versa); any language change clears cached translations (`TA.cpp:1718-1735`).
- Check marks reflect per-frame state (topmost, columns, toolbars, lock order) and global state (substitutions, conversions, hotkeys).
- Context Manager has its own small menu bar: Process ▸ Clear, Context ▸ Clear History (`.rc:310-320`).

## 6. Keyboard

Accelerators (`.rc:116-164`). The message loop sends them to the pane under the focus if there is one, otherwise to the frame (`TA.cpp:2043-2063`).

| Keys | Action |
|---|---|
| Alt-0 … Alt-9 | Restore layout 1–10 |
| Shift-Alt-0 … Shift-Alt-9 | Save current frames as layout 1–10 (layout 0 = auto-saved on close) |
| Ctrl-0 | Show all panes |
| Ctrl-2 | Toggle one/two columns |
| Ctrl-T / Ctrl-F | Hide toolbars / hide frame |
| Alt-A | Always on top |
| Ctrl-Num- / Ctrl-Num+ | More / less transparent |
| Alt-F / Alt-T | Main font / tooltip font |
| Alt-S | Substitutions dialog |
| Ctrl-M | Context Manager |
| Ctrl-L | Launch/inject dialog (moves to hook plugin) |
| Alt-Up, Alt-PgUp / Alt-Down, Alt-PgDn | History back / forward |
| Ctrl-H / Ctrl-K / Ctrl-R | Selection (or all) to hiragana / katakana / romaji |
| Alt-X | Quit |

Global hotkeys, when enabled in File (`TA.cpp:67-77`): Ctrl-Alt-Up/PgUp (back), Ctrl-Alt-Down/PgDn (forward), Ctrl-Alt-A (topmost), Ctrl-Alt-Num- / Num+ (transparency).

Furigana panes (JParser, MeCab) are custom views with their own keys: Up/Down, PgUp/PgDn, Home/End scroll; mouse wheel scrolls (`LocalWindows/FuriganaWindow.cpp:614-640`). The legacy UI has no visible focus indicator for panes and no keyboard way to move between panes or open the pane menu besides the standard context-menu key.

## 7. Dialogs

All dialogs use "MS Shell Dlg" 8 pt, are modal, fixed size, with OK/Cancel and sometimes Apply. Colours are typed as hex numbers with a "…" picker button.

| Dialog | Size (dialog units) | Content | Code | Rewrite |
|---|---|---|---|---|
| Pre-Translation Substitutions (Alt-S) | 293×311 | Profile dropdown (explains that active profiles follow running programs; `*` always active), "Add substitution" (original → replacement), list view, Delete/Clear | `.rc:490-511` | Settings page |
| Context Manager (Ctrl-M) | 539×373 | Attached processes list, idle delay, history max (KB), process contexts list; per-context: auto translate, disable, merge stream/on flush, Japanese only, auto copy to clipboard, looping text; character-repeat filter (none / all / auto constant / auto variable / preset), phrase extension filter, phrase repeat filter (min/max), line breaks (remove / keep / keep some first/last); full history toggle, hook port, context text view | `.rc:513-569` | Settings page + live context panel (hook port goes away) |
| JParser Configuration | 303×222 | Furigana (none/hira/kata/romaji), font sizes, definition display options, hide cross refs/usage/POS, word highlight colours (default, translated, particles, with furigana), tooltip colours (kanji, kana, parentheses, conjugations), use MeCab hinting | `.rc:432-488` | Settings page / pane settings |
| MeCab Configuration | 168×154 | Characters (hira/kata/romaji), font sizes, word highlight colours | `.rc:392-419` | Settings page / pane settings |
| WWWJDIC Config | 232×71 | Mirror selection | `.rc:421-430` | Only if recreated as a connector definition |
| Select Program (Ctrl-L) | 517×351 | Profiles, inject/launch mode, locale, text hook type, child processes, ATLAS menu translation, DBCS override, internal hook options and codes, AGTH options, running processes list | `.rc:328-390` | Hook plugin UI (ATLAS options cut) |
| AGTH Code Conversion Tool | 234×66 | AGTH `/H` code → TA code | `.rc:571-582` | Hook plugin UI |
| Atlas Config | 248×127 | Environment, sentence-break options, rule set | `.rc:217-236` | Cut (D7) |
| Font / Tooltip Font | system `ChooseFont` | Face, size, style, colour | `exe/Config.cpp:1271-1305` | Settings page |

## 8. Furigana view and dictionary tooltips

- Words are painted one by one with furigana centred above each word (`FuriganaWindow.cpp:352-399,414-457`).
- Word background highlights: words with furigana, particles, words with a dictionary translation (`FuriganaWindow.cpp:363-379`). Defaults (`FuriganaWindow.cpp:36-47`; INI values are `COLORREF`, i.e. 0xBBGGRR): furigana background light grey `0xE0E0E0` for MeCab, none for JParser; particles `0xFFE0E0` (a light lavender-blue); translated words `0xC8F0C8` (light green). Text size 13 px, furigana 10 px.
- A part-of-speech colour table exists (noun red, verb blue, adjective light blue, adverb green, particle magenta, `FuriganaWindow.cpp:14-24`).
- Disabled pane: 3D-face background (`FuriganaWindow.cpp:409-411,646-656`).
- Hovering a word for 350 ms opens the dictionary tooltip (`FuriganaWindow.cpp:716-730,511-552`); leaving the word closes it (`:468-483`).
- Tooltip: borderless topmost popup that does not take focus, info-background colour, width 400–600 px chosen by text length, placed 24 px below the cursor and outside the word (`exe/Dialogs/MyToolTip.cpp:433-482,809,825`). Text colours: kanji dark red RGB(160,0,0), kana green RGB(30,160,30), parentheses light blue RGB(100,170,230), conjugations grey RGB(150,150,150) (`exe/Config.cpp:1032-1035`). Font: the tooltip font, default the system status-bar font (`exe/Config.cpp:1017-1023`). Long tooltips cannot be scrolled in shipped builds (scrolling exists only behind `SETSUMI_CHANGES`, `MyToolTip.cpp:355-414`).

## 9. Colours and fonts summary

| Item | Default | Code |
|---|---|---|
| Frame and pane background | system 3D-face | `TA.cpp:1867,1879` |
| Text panes | system window / window-text colours (RichEdit default) | `TW.cpp:80-83` |
| Main text font | Arial, INI height 195, converted with `lfHeight = -height/15` → 13 px character height; black | `exe/Config.cpp:1003-1005,1275` |
| Toolbar label font | Arial 19 px, weight 500 | `exe/Config.cpp:1069` |
| Tooltip font | system status font, fallback Arial 11 px | `exe/Config.cpp:1013-1023` |
| Dialog font | MS Shell Dlg 8 pt | `.rc` dialogs |
| Drop indicator | RGB(160,170,235), alpha 128 | `TA.cpp:1891,1978` |
| Dark mode, themes | none | — |

## 10. Modernization opportunities (fit D11)

Keep the layout and workflow recognisable; fix what the legacy UI makes hard.

1. **Pane header** instead of a toolbar whose translate button doubles as the title: name, provider status dot (ok / auth missing / quota / error), last request time, cost or token count for LLM panes, actions (translate/retry, copy, pin, auto toggle, settings, close).
2. **Visible drag handle** in the pane header, and drop zones that also allow "new column/row here", not just "replace this slot". Keep tear-out into a new frame.
3. **Settings UI** replacing INI edits and the scattered modal dialogs: providers (web connector definitions, LLM backends, keys via credential store), languages, inputs (clipboard, local API), contexts and filters, substitutions, dictionaries, appearance, shortcuts.
4. **Themes:** a classic-Win32 light theme as default look, plus dark; colours come from theme tokens instead of hex numbers per dialog. Furigana highlight colours readable in both themes.
5. **High-DPI and font scaling:** per-monitor DPI, one UI scale setting, system UI font by default (e.g. Segoe UI / Yu Gothic UI) with CJK fallback; text-pane font chosen separately.
6. **Keyboard accessibility:** visible focus, Ctrl-Tab/F6 to move between panes, keyboard access to pane actions and tooltips, all shortcuts rebindable and listed, no mouse-only features (splitter reset, drag).
7. **Command palette** (e.g. Ctrl-Shift-P): every menu command, pane toggle, layout and language by name.
8. **Searchable history** with the per-provider results of each entry, re-translate, copy, export (replaces the DLL-only log file).
9. **Language pickers** with search and recent pairs instead of two 69-item menus; show per-pane capability.
10. **Side-by-side comparison:** highlight differences between providers' outputs; optional diff view.
11. **Named layouts** in addition to the 10 numbered slots; layout 0 auto-save kept.
12. **Tooltips:** scrollable, can be pinned, reachable by keyboard; shown after a configurable delay.
13. **Input source indicator** in the Original Text pane (clipboard, local API client, hook plugin, typed), plus a pause button for automatic input.
14. **Status line** per frame: input paused/active, local API on/off, number of providers busy.
15. **Transparency per frame** kept, with a slider in addition to Ctrl-Num-/+; "click-through" overlay mode is a candidate for the UI design step.
16. **First-run flow:** choose target language, add one provider (with key), optionally download a dictionary.

## 11. Toolkit comparison against D11 (decision: iced, D14)

Short and fair comparison of the candidates that pass D3; `slint` is excluded (GPLv3/commercial).

| Criterion | `egui`/`eframe` | **`iced` (chosen)** | Native Win32 via `windows-rs` |
|---|---|---|---|
| MSRV 1.90 | Only up to 0.33.x (newer need 1.92/1.95) | 0.14 OK (1.88) | OK |
| License tree | Permissive, but default fonts add OFL/Ubuntu-font | Permissive | Permissive |
| Multi-pane splits | Via `egui_tiles`/`egui_dock` (newest need 1.95) | Built-in `pane_grid` (split, resize, drag, hotkeys) | Hand-written (as in legacy) |
| Multiple top-level frames | Viewports | `daemon` with multiple windows | Native |
| Look like classic Win32 | Hard (immediate-mode look, custom styling) | Achievable with a custom theme and widget styles; still drawn, not native | Native look for free |
| Light/dark, theming | Good | Good (palette + style functions) | Hard (Win32 dark mode is partial) |
| High-DPI | Good | Good (scale factor) | Manual per-monitor DPI work |
| Menus | Drawn | Drawn (`iced_aw`) or native via window handle (`muda`, unverified) | Native |
| Transparency / topmost | Topmost yes; whole-window alpha via handle | Topmost yes (`Level::AlwaysOnTop`); alpha via handle (unverified) | Native |
| Custom furigana rendering | Painter API | Custom widget / canvas | GDI/DirectWrite by hand |
| IME (Japanese input) | Supported, quality varies | Unverified, test early | Native |
| Accessibility | AccessKit | Limited/unverified | Native UIA via standard controls |
| Effort and `unsafe` | Low effort, little `unsafe` | Medium effort, little `unsafe` | Highest effort, most `unsafe` |
| Architecture fit | Immediate mode; state handling is simple | Elm-style messages fit async translators and streams well | Message loop; async glue by hand |

### What "classic Win32 look" means for iced theming
- **Fonts:** system UI font (Segoe UI; CJK fallback Yu Gothic UI / Meiryo / MS UI Gothic) loaded by name; sizes in points scaled with DPI; text panes use the user's chosen font.
- **Colours:** palette derived from the Windows system colours (3D face, window, window text, highlight, info background) for the light theme; a matching dark palette.
- **Controls:** square or 2-px-radius corners, 1-px borders, flat toolbar buttons with hover/pressed states, standard check boxes and drop-downs, focus rectangles; no heavy shadows or large paddings.
- **Menu bar** under the title bar with access keys (Alt+letter) and right-aligned shortcut text; context menus with the same style.
- **Toolbars:** a compact pane header row (≈ 22–24 px at 100 %), icon buttons with tooltips.
- **Splitters:** thin gaps (≈ 3 px) with a wide hit zone and the classic resize cursors; double-click to reset.
- **Window chrome:** native title bar and borders (iced uses the OS decorations by default); "hide frame" as an undecorated resizable window.

### Gaps in iced and mitigations
| Gap | Mitigation |
|---|---|
| No native menu bar | Drawn menu bar (`iced_aw` menu or own widget) with access keys; or attach a native menu through the window handle (`muda`) — spike first (O14) |
| Pane drag between top-level windows not built in | Own drag state across windows: start a drag in a pane header, show drop zones in the target window, move the pane in app state; `pane_grid` handles drag inside one window |
| Whole-window transparency | `SetLayeredWindowAttributes` on the raw window handle; fall back to per-pixel transparent background if that conflicts with the renderer (spike) |
| Global hotkeys | `RegisterHotKey` on the platform message window in `ta-platform`, or `global-hotkey` |
| Multi-window support details (focus, per-window menus) | Use `iced::daemon`; keep frame state in the app model, not in the windows |
| IME for Japanese typing | Test in the first GUI spike; if insufficient, use a native edit control for the Original Text pane as an escape hatch |
| GPU renderer on old/32-bit machines and VMs | Enable the `tiny-skia` software renderer as fallback |
| Accessibility (screen readers) | Keyboard accessibility per D11 regardless; track iced's accessibility work; re-check before release |
| DPI | iced follows the window scale factor; verify per-monitor changes when moving frames between monitors |
