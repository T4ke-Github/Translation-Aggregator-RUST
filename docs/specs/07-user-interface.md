# 07 User interface (`UI`)

## 1. Purpose and scope

This spec covers the GUI of the app (`ta-gui`): frames, panes, columns and splitters, saved
layouts, frame options, keyboard shortcuts, language selection, pane actions and states,
provider health, the history view, the command palette, the settings UI, themes, high-DPI and
fonts, Japanese input in the Original Text pane, and the first-run flow.

It keeps the recognisable multi-pane layout of the legacy app and adds the modern UX listed in
D11. The toolkit is `iced`, styled like classic Win32 (D14).

Not covered here:

- **Concrete screen design.** Pixel layouts, icons, exact colours and spacing are decided in the
  UI design step (UI-01). This spec says what the UI must do, not how it looks in detail.
- The furigana view, dictionary tooltips and pane content of MeCab and JParser (see the JP spec).
- The hook plugin's own UI (inject/launch dialog, see the HOOK spec).
- Settings storage, secrets storage and legacy INI/layout import (see the CFG spec). This spec
  only covers the UI on top of them.
- History back/forward behaviour, search and export (PIP-11, PIP-12). This spec only adds the
  history view (UI-11).
- Shared numbers (performance, limits, accessibility levels) are defined in
  `11-quality-and-release.md` and referenced by ID.

## 2. Goals

1. A legacy user recognises the layout and finds the familiar shortcuts working on day one.
2. Every feature can be used without editing a file by hand, and without a mouse.
3. The user always sees what each pane is doing: busy, done, cached, failed and why, and whether
   text leaves the machine.
4. The UI stays responsive whatever the providers, plugins or network do (NFR-03.3).
5. The app looks like a classic Win32 tool in a light theme, with a matching dark theme, sharp on
   high-DPI screens.
6. A new user with no provider configured reaches a first translation through a guided setup.

## 3. Features

### UI-01 UI design step before the GUI is built

**Priority:** Must · **Milestone:** M2 (precondition: done before GUI screen work starts) ·
**Decisions:** D11, D14, D13 · **Legacy:** `ui-reference.md` (all sections) · **Depends on:** —

As the project owner, I want the concrete look agreed before screens are built, so that the GUI
is built once and matches D11.

**Acceptance criteria**

- **UI-01.1** Given Phase 2 is about to start, when GUI screen work begins, then a UI design
  document exists in `docs/` and records the owner's approval with a date. No `ta-gui` screen code
  is merged before that approval (headless state logic and spikes are allowed). *(Verify: manual)*
- **UI-01.2** Given the UI design document, when it is reviewed for approval, then it covers at
  least: frame and pane anatomy (pane header), menus, drag-and-drop zones, the settings UI
  structure, theme tokens for light and dark, the icon set (new icons, no legacy bitmaps, D13),
  empty/loading/error states, the first-run flow and the full keyboard map. *(Verify: manual)*
- **UI-01.3** Given the UI design document, when it is reviewed for approval, then it starts from
  `ui-reference.md` and marks each item of its §10 modernization list as adopted, deferred (with
  milestone) or rejected (with reason). *(Verify: manual)*
- **UI-01.4** Given the known iced gaps, when the design step runs, then a spike for each is run on
  both bitnesses and its result recorded with the chosen mitigation: native vs drawn menu bar
  (O14), whole-window transparency, pane drag between top-level windows, Japanese IME in a text
  editor, multi-window focus, the software renderer fallback, per-monitor DPI changes, and
  screen-reader support. *(Verify: manual)*
- **UI-01.5** Given the design changes a criterion in this spec, when the design is approved, then
  this spec is updated in the same change. Changed criteria keep their IDs; removed ones are marked
  *Withdrawn*. *(Verify: manual)*

### UI-02 Main window and panes

**Priority:** Must · **Milestone:** M2 · **Decisions:** D5, D11, D12 · **Legacy:** `ui-reference.md`
§1, §5; `features.md` §1, §5 "Show all panes" · **Depends on:** PIP-04, CFG-01

As a user, I want one Original Text pane and one pane per translator, so that I can read all
translations of the same line side by side.

**Acceptance criteria**

- **UI-02.1** Given translators and tools are configured, when the app runs, then there is exactly
  one Original Text pane and one pane per enabled translator or tool (web presets, custom web
  services, LLM backends, MeCab, JParser, translator plugins). Each pane exists at most once and is
  either shown in one frame or hidden. *(Verify: unit)*
- **UI-02.2** Given no saved layout exists, when the app starts, then it shows one frame with all
  panes in two columns, the column split at 50 %, Original Text first, not topmost, opaque, with
  frame and pane headers shown (the legacy defaults). *(Verify: unit)*
- **UI-02.3** Given a pane is shown, when the user clicks its close button, then the pane is hidden,
  not removed. The Window menu has one check item per pane to show or hide it, and Ctrl-0 shows all
  hidden panes in the current frame. *(Verify: integration)*
- **UI-02.4** Given a pane is hidden, when new text arrives, then that pane sends no automatic
  request. Showing it again does not send a request until the user translates or new text arrives.
  *(Verify: integration)*
- **UI-02.5** Given a frame with its menu bar shown, when the user opens the menus, then there are
  File, Tools, View, Window, Providers and Help menus with the kept legacy commands
  (`ui-reference.md` §5) minus cut ones (ATLAS, D7; "Launch Text Hooker", D6). The Providers menu
  (legacy "Websites") opens each provider's documentation or console link in the default browser.
  *(Verify: manual)*
- **UI-02.6** *(Could)* Given two or more translator panes show results for the same text, when the
  user turns on "Compare", then the differences between two chosen panes are highlighted in both.
  *(Verify: manual)*

### UI-03 Frames and moving panes

**Priority:** Must · **Milestone:** M2 · **Decisions:** D11, D14 · **Legacy:** `ui-reference.md` §2,
§4 "Drag to move"; `features.md` §5 "Multiple top-level frames" · **Depends on:** UI-02

As a user, I want to spread panes over several windows and rearrange them by dragging, so that I
can place translations next to the game.

**Acceptance criteria**

- **UI-03.1** Given the app runs, when the user opens new frames, then at least 20 frames can be
  open, each a top-level window with its own panes. Closing a frame that is not the last one hides
  its panes; closing the last frame saves the auto layout (UI-05.3) and quits.
  *(Verify: integration)*
- **UI-03.2** Given a frame with two or more panes, when the user drags a pane by the drag handle
  in its header, then drop zones are shown and releasing over one moves the pane there. Esc during
  the drag cancels it and nothing changes. *(Verify: manual)*
- **UI-03.3** Given two frames, when the user drags a pane over the other frame, then the target
  slot is highlighted with a half-transparent accent overlay, and releasing docks the pane there.
  Releasing outside every frame tears the pane out into a new frame at the cursor position.
  *(Verify: manual)*
- **UI-03.4** Given "Lock Order" is on for a frame, when the user tries to drag a pane out of it or
  into it, then the drag does not start or the drop is refused with a not-allowed cursor, and the
  layout is unchanged. *(Verify: integration)*
- **UI-03.5** Given any move that works by drag, when the user uses only the keyboard, then the
  pane menu and the command palette offer "Move pane to" with the targets new frame, frame *N*, and
  a position in a frame. *(Verify: manual)*
- **UI-03.6** *(Should)* Given a pane is dragged, when the drop zones are shown, then they also
  allow "insert before", "insert after" and "new column", not only "replace this slot".
  *(Verify: manual)*

### UI-04 Columns and splitters

**Priority:** Must · **Milestone:** M2 · **Decisions:** D11 · **Legacy:** `ui-reference.md` §3,
§11 "Splitters" · **Depends on:** UI-02

As a user, I want one or two columns with draggable splitters, so that I can give long
translations more room.

**Acceptance criteria**

- **UI-04.1** Given a frame, when the user presses Ctrl-2, then it toggles between one and two
  columns. In two columns there are ceil(n/2) rows, and with an odd number of panes the first pane
  spans the full width. *(Verify: unit)*
- **UI-04.2** Given a frame with several panes, when the pointer nears a gap between panes, then
  the gap (about 3 px at 100 % scale) acts as a splitter with a hit zone of at least 8 px on each
  side and shows the horizontal, vertical or both-ways resize cursor. *(Verify: manual)*
- **UI-04.3** Given a frame, when the user drags a splitter, then it moves the column split or one
  row boundary, bounded by its neighbours. No pane gets smaller than its header plus one text line.
  *(Verify: unit)*
- **UI-04.4** Given a frame with moved splitters, when the user double-clicks a splitter or runs
  "Reset splitters" from the menu or the palette, then the column split goes back to 50 % and rows
  to equal heights. *(Verify: integration)*
- **UI-04.5** Given splitter positions stored as fractions of the frame size, when a frame is
  resized, then the proportions stay within 1 px of rounding, and a save/restore round trip keeps
  them within 0.1 %. *(Verify: unit)*

### UI-05 Saved layouts

**Priority:** Must · **Milestone:** M2 · **Decisions:** D11, D16 · **Legacy:** `ui-reference.md` §6;
`features.md` §5 "10 bindable layouts" · **Depends on:** UI-03, UI-04, CFG-01, CFG-03

As a user, I want to save and restore window arrangements with one key, so that I can switch
between setups for different games.

Legacy layouts are imported by CFG-07.

**Acceptance criteria**

- **UI-05.1** Given any frames are open, when the user presses Shift-Alt-*digit* (0–9), then all
  frames are saved to the slot for that digit: position, size, monitor, panes and their order,
  columns, splitter fractions, always on top, transparency, hidden frame, hidden headers and lock
  order. The status line confirms "Layout *digit* saved". *(Verify: integration)*
- **UI-05.2** Given a slot holds a layout, when the user presses Alt-*digit*, then exactly that
  arrangement is restored. Given the slot is empty, then nothing changes and the status line says
  so. *(Verify: integration)*
- **UI-05.3** Given the auto layout (legacy "layout 0"), a separate slot that is not one of the 10
  digit slots, when a frame closes, the app quits or 5 s have passed after a layout change, then
  the auto layout is saved; it is restored at start. *(Verify: integration)*
- **UI-05.4** Given a saved frame position is no longer on any monitor, when the layout is
  restored, then the frame is moved so that its title bar is fully visible on the nearest monitor.
  *(Verify: unit)*
- **UI-05.5** Given a layout names a pane that no longer exists, when it is restored, then that
  pane is skipped. Panes enabled after the layout was saved stay hidden and are listed in the
  Window menu. *(Verify: unit)*
- **UI-05.6** *(Should)* Given the Window menu or the palette, when the user saves a layout under a
  name, then it can be restored, renamed or deleted there, and the numbered slots keep working.
  *(Verify: manual)*

### UI-06 Frame display options

**Priority:** Must · **Milestone:** M2 · **Decisions:** D11, D14, D20 · **Legacy:** `ui-reference.md`
§2, §5 "View"; `features.md` §5 "Transparency…" · **Depends on:** UI-03

As a user, I want to make frames transparent, keep them on top and hide their chrome, so that a
frame can sit over a game like an overlay.

**Acceptance criteria**

- **UI-06.1** Given a frame, when the user presses Ctrl-Num- (more transparent) or Ctrl-Num+ (less
  transparent), then the frame's opacity changes in steps of 16 in the range 20–255 (the legacy
  range). A slider in the View menu sets the same value. The value is per frame and saved in
  layouts. *(Verify: integration)*
- **UI-06.2** Given a frame, when the user presses Alt-A, then the frame toggles always on top. The
  View menu shows the state with a check mark. *(Verify: integration)*
- **UI-06.3** Given a frame, when the user presses Ctrl-F, then the frame toggles to a window
  without title bar, borders and menu bar that can still be moved and resized. All menu commands
  stay reachable from the pane context menu and the command palette (a fix of the legacy
  limitation, listed per D20). *(Verify: manual)*
- **UI-06.4** Given a frame, when the user presses Ctrl-T, then all pane headers in that frame are
  hidden or shown. With headers hidden, pane actions stay reachable through the context menu and
  shortcuts. *(Verify: manual)*
- **UI-06.5** *(Could)* Given a transparent frame in "click-through" mode, when the user clicks on
  it, then the click passes to the window below; a global hotkey turns the mode off again. Decided
  in UI-01. *(Verify: manual)*

### UI-07 Keyboard shortcuts

**Priority:** Must · **Milestone:** M2 · **Decisions:** D11, D6, D20 · **Legacy:** `ui-reference.md`
§6 "Keyboard"; `features.md` §5 "Global hotkeys" · **Depends on:** UI-02, UI-13

As a legacy user, I want the shortcuts I know to work unchanged, so that I don't have to relearn
the app.

Default shortcuts (kept from the legacy app):

| Keys | Action |
|---|---|
| Alt-0 … Alt-9 | Restore the layout slot of that digit (10 slots) |
| Shift-Alt-0 … Shift-Alt-9 | Save the layout slot of that digit |
| Ctrl-0 | Show all panes |
| Ctrl-2 | Toggle one/two columns |
| Ctrl-T | Hide/show pane headers (toolbars) |
| Ctrl-F | Hide/show frame and menu |
| Alt-A | Always on top |
| Ctrl-Num- / Ctrl-Num+ | More / less transparent |
| Alt-F / Alt-T | Main font / tooltip font (opens the font settings) |
| Alt-S | Substitutions (opens the settings page) |
| Ctrl-M | Contexts (opens the context page) |
| Alt-Up, Alt-PgUp / Alt-Down, Alt-PgDn | History back / forward (PIP-11.2) |
| Ctrl-H / Ctrl-K / Ctrl-R | Selection (or all text) to hiragana / katakana / romaji (JP-10) |
| Alt-X | Quit |

Global hotkeys (off by default): Ctrl-Alt-Up or Ctrl-Alt-PgUp (back), Ctrl-Alt-Down or
Ctrl-Alt-PgDn (forward), Ctrl-Alt-A (always on top), Ctrl-Alt-Num- / Ctrl-Alt-Num+ (transparency).

New defaults (may change in UI-01): Ctrl-Shift-P command palette, Ctrl-Tab / Ctrl-Shift-Tab and
F6 / Shift-F6 move focus between panes, Ctrl-Enter "Translate All" in the Original Text pane,
Ctrl-Comma settings, Ctrl-Shift-H history view. Legacy Ctrl-L (launch text hooker) is not bound in
the core; the hook plugin may register a command for it.

**Acceptance criteria**

- **UI-07.1** Given default settings, when the user presses any key in the table or the global
  hotkey list above, then the listed action runs. *(Verify: integration)*
- **UI-07.2** Given a pane has focus, when the user presses a pane shortcut (e.g. Ctrl-H), then it
  acts on that pane. Otherwise frame shortcuts act on the active frame. *(Verify: unit)*
- **UI-07.3** Given global hotkeys are turned on in the File menu, when another application has
  focus and the user presses one, then it works. Given one key is already taken by another
  program, then the app names the failed key and the others still work. *(Verify: manual)*
- **UI-07.4** Given the shortcuts settings page, when the user rebinds a shortcut, then the new key
  works; binding a key that another command in the same scope uses shows the conflict and does not
  save until resolved. "Reset to defaults" restores the table above. *(Verify: unit)*
- **UI-07.5** Given any command with a shortcut, when it is shown next to a menu item, in the
  command palette or in the "Keyboard shortcuts" list in the Help menu, then its shortcut is shown
  with it. *(Verify: manual)*
- **UI-07.6** Given a menu access key (Alt+letter) clashes with a legacy shortcut (e.g. Alt-F,
  Alt-T), when the user presses it, then the legacy shortcut wins, and the menu is opened with Alt
  alone or F10. *(Verify: manual)*

### UI-08 Source and target languages

**Priority:** Must · **Milestone:** M2 · **Decisions:** D5, D10, D11 · **Legacy:** `ui-reference.md`
§5 "Translate From / To"; `features.md` §5 "Source/target language menus" · **Depends on:** PIP-03,
PIP-04

As a user, I want to pick the source and target language quickly, and see which panes can handle
that pair, so that I am never left waiting for a pane that cannot work.

**Acceptance criteria**

- **UI-08.1** Given configured providers, when the user opens the source or target picker, then it
  has a search field, lists the union of the languages those providers support (PIP-03.1), and
  shows the 5 most recent pairs first. "Auto" is offered as a source only. *(Verify: unit)*
- **UI-08.2** Given a language pair, when the user picks a source equal to the target (or the other
  way round), then the two are swapped. *(Verify: unit)*
- **UI-08.3** Given a pane's provider cannot translate the chosen pair, when the pair is chosen,
  then the pane shows the disabled state with the reason ("does not support ja → de") and sends no
  request (PIP-03.2). *(Verify: integration)*
- **UI-08.4** Given results are shown, when the pair changes, then results for the old pair are
  cleared from all panes, and panes with auto translate on translate the current text again.
  *(Verify: integration)*
- **UI-08.5** Given the pickers and the settings pane list, when they are shown, then each language
  entry shows how many enabled panes support it, and the pane list shows each provider's supported
  languages. *(Verify: manual)*

### UI-09 Pane header and actions

**Priority:** Must · **Milestone:** M2 · **Decisions:** D11, D17, D21, D20 · **Legacy:**
`ui-reference.md` §4 "Pane anatomy", context menu; `features.md` §1 "Common pane behaviour" ·
**Depends on:** UI-02, PIP-04, PIP-08

As a user, I want each pane to have a clear header with its actions, so that I can translate,
copy, retry or pin a result without hunting through menus.

**Acceptance criteria**

- **UI-09.1** Given a pane, when it is shown, then its header shows the pane name and a translate
  button. Clicking translate sends the current text to this pane only (IN-03.2); clicking it while
  the pane is busy cancels the request. *(Verify: integration)*
- **UI-09.2** Given a translator pane, when it is shown, then it has an "auto translate" toggle,
  saved per pane, whose default follows PIP-05.1: off for every added translator, on for MeCab and
  JParser *(assumes O10 recommendation)*. *(Verify: unit)*
- **UI-09.3** Given a pane with a result, when the user runs "Copy", then the result goes on the
  clipboard as plain text without being captured as input (IN-01.4); when the user runs "Retry",
  then the current text is sent again, bypassing the cache (PIP-08.6). *(Verify: integration)*
- **UI-09.4** Given a pane is pinned, when new text arrives, then the pane keeps its current text
  and result and sends no request until it is unpinned. The header shows the pinned state.
  *(Verify: integration)*
- **UI-09.5** Given an enabled provider that sends text off the machine (NFR-05.3), or a custom
  service marked unofficial (D21), when its pane is shown, then it carries a "text leaves this
  machine" or an "unofficial" badge, and neither badge can be hidden while the provider is enabled.
  *(Verify: integration)*
- **UI-09.6** Given a pane, when the user opens its context menu, then it has Cut, Copy, Paste; To
  Hiragana (Ctrl-H), To Katakana (Ctrl-K), To Romaji (Ctrl-R); SJIS/UTF-16 to hex and hex to
  SJIS/UTF-16; Configure; Move pane to; and the View commands. Conversions apply to the selection,
  or to all text if nothing is selected (JP-10). *(Verify: parity)*

### UI-10 Pane status, states and provider health

**Priority:** Must · **Milestone:** M2 (cost/streaming M3, API status M5) · **Decisions:** D11, D20 ·
**Legacy:** `ui-reference.md` §4 "Busy indicator", "Disabled state", §10 items 1 and 14 ·
**Depends on:** UI-09, PIP-09

As a user, I want to see what each pane and provider is doing and why something failed, so that I
can fix problems myself.

**Acceptance criteria**

- **UI-10.1** Given a pane, when it is shown, then it shows its state: empty, queued, busy,
  streaming, done, cached, cancelled, error or disabled. After a request it shows the request time
  in ms, and "cached" for cache hits (PIP-08.1). LLM panes also show token count and the
  user-entered cost (LLM-09) *(M3)*. *(Verify: unit)*
- **UI-10.2** Given a request, when it finishes within 100 ms, then no busy indicator appears;
  when it takes longer, then an inline busy indicator appears in the header after 100 ms, and
  streaming text appears as it arrives *(M3)*. *(Verify: manual)*
- **UI-10.3** Given a request fails, when the pane shows it, then it shows a short message with the
  cause category (needs key, quota or rate limit, timeout, offline, provider error, plugin stopped)
  and a fitting action (Retry, Open settings). Details (HTTP status, provider message) are one click
  away, with secrets masked (NFR-05.2). No modal error dialog is shown for request failures. Web
  providers use the states of WEB-12.1. *(Verify: integration)*
- **UI-10.4** Given no text or no translator yet, when the panes are shown, then the Original Text
  pane says how to get text in (copy, type, local API); a translator pane before any text says it
  is waiting for text; with no translator configured, the translator area offers "Add a
  translator" (PIP-04.5, UI-16). *(Verify: manual)*
- **UI-10.5** Given a provider, when its pane header or the settings provider list is shown, then
  it has a health indicator: ok, needs key, quota or rate limited, offline, error, or disabled. It
  uses shape or text as well as colour (NFR-07.3) and its tooltip shows the last error and time.
  *(Verify: unit)*
- **UI-10.6** Given a frame, when its status line is shown, then it shows whether automatic input
  is active or paused, whether the local API is on *(M5)*, and how many panes are busy.
  *(Verify: manual)*

### UI-11 History view

**Priority:** Should · **Milestone:** M2 · **Decisions:** D11, D12 · **Legacy:** `ui-reference.md` §6;
`features.md` §3 "Translation history back/forward" · **Depends on:** PIP-11, PIP-12

As a user, I want a view of earlier lines that I can search, so that I can reread what I missed.

Back/forward behaviour is specified once, in PIP-11.2 to PIP-11.4; search and export in PIP-12.
UI-11.1 to UI-11.3 are withdrawn for that reason, and UI-11 is now Should (it was Must because of
those criteria).

**Acceptance criteria**

- **UI-11.1** *Withdrawn*, see PIP-11.2 (back/forward keys and what the panes show).
- **UI-11.2** *Withdrawn*, see PIP-11.4 (oldest/newest entry; new text does not move the view away from the entry being read).
- **UI-11.3** *Withdrawn*, see PIP-11.3 (pane without a stored result).
- **UI-11.4** *(Should)* Given history entries, when the user opens the history view
  (Ctrl-Shift-H), then it lists entries newest first with their time and context, and offers the
  search and filters of PIP-12.1 and PIP-12.2 with their time limit. *(Verify: integration)*
- **UI-11.5** *(Should)* Given the history view, when the user opens an entry, then it shows all
  per-provider results and offers translate again (PIP-12.3), copy, and export of selected entries
  (PIP-12.4). *(Verify: manual)*

### UI-12 Command palette

**Priority:** Should · **Milestone:** M2 · **Decisions:** D11 · **Legacy:** `ui-reference.md` §10
item 7 · **Depends on:** UI-07

As a user, I want to run any command by typing its name, so that I don't need to remember menus or
shortcuts.

**Acceptance criteria**

- **UI-12.1** Given any frame has focus, including a hidden frame, when the user presses
  Ctrl-Shift-P, then the palette opens with the input focused. *(Verify: integration)*
- **UI-12.2** Given the palette is open, when it lists commands, then it holds every menu command,
  show/hide for every pane, save/restore for every layout slot and named layout, every source and
  target language, every theme and "Move pane to". *(Verify: unit)*
- **UI-12.3** Given the palette is open, when the user types, then the list is filtered by matching
  words or initials in the command name (e.g. "aot" finds "Always on top"); each entry shows its
  shortcut, and recently used commands come first. *(Verify: unit)*
- **UI-12.4** Given the palette is open, when the user presses Enter, then the selected command
  runs; when the user presses Esc, then the palette closes and focus returns to where it was.
  *(Verify: manual)*

### UI-13 Settings UI

**Priority:** Must · **Milestone:** M2 (dictionaries M4, local API M5, plugins M6) · **Decisions:**
D16, D10, D17, D21, D18 · **Legacy:** `ui-reference.md` §7 "Dialogs"; `features.md` §1, §3, §4 ·
**Depends on:** CFG-01, CFG-05, IN-05

As a user, I want one settings window for everything, so that I never edit an INI or TOML file by
hand.

**Acceptance criteria**

- **UI-13.1** Given the settings window, when it is opened, then it has pages for: providers, keys,
  languages, inputs, contexts and filters, substitutions, appearance (fonts, themes, scale),
  shortcuts, dictionaries *(M4)*, local API *(M5)* and plugins *(M6)*, and every user setting in the
  settings file has a control on one of these pages. *(Verify: manual)*
- **UI-13.2** Given the providers page, when it is opened, then it lists all providers with health
  (UI-10.5) and lets the user enable or disable, add from a preset, import a definition file, edit,
  duplicate and remove a provider (WEB-11), and run "Test", which shows the raw request and reply
  with secrets masked (WEB-10). *(Verify: manual)*
- **UI-13.3** Given the user types a key into its masked field, when it is saved, then it is stored
  in the OS credential store (CFG-05.1), and the UI shows only "set" or "not set" with "Replace" and
  "Remove"; it never shows the stored value (NFR-05.2). *(Verify: security)*
- **UI-13.4** Given the user changes a setting, when it is saved, then it applies to all frames
  without a restart; settings that need a restart say so next to the control. *(Verify: manual)*
- **UI-13.5** Given the user enters an invalid value, when the field is left or saved, then the
  field shows what is wrong and how to fix it, and the invalid value is not saved. *(Verify: unit)*
- **UI-13.6** Given the user enables a remote provider, or turns on its auto translate, when the
  change is made, then the settings UI states that text will leave the machine and asks once for
  confirmation, as in PIP-05.2 *(assumes O10 recommendation)*. *(Verify: manual)*

### UI-14 Appearance: themes, high-DPI and fonts

**Priority:** Must · **Milestone:** M2 · **Decisions:** D11, D14 · **Legacy:** `ui-reference.md` §2
"DPI", §9, §11 "What classic Win32 look means" · **Depends on:** UI-01

As a user, I want a classic Windows look, a dark theme and sharp text on any screen, so that the
app is comfortable for long sessions.

**Acceptance criteria**

- **UI-14.1** Given default settings, when the app is shown, then the theme is "Classic (light)":
  colours from the Windows system colours (3D face, window, window text, highlight, info
  background), 1-px borders, square or 2-px corners, flat header buttons with hover and pressed
  states, and focus rectangles. *(Verify: manual)*
- **UI-14.2** Given the appearance page, when the user picks the dark theme, then it has the same
  layout and tokens and applies to all frames without a restart. *(Should)* With "Follow Windows",
  the theme switches within 2 s when the Windows app mode changes. *(Verify: manual)*
- **UI-14.3** Given a frame is moved between a 100 % and a 150 % monitor, when it arrives, then it
  re-renders at the new scale within 1 s, with no bitmap-scaled blur (per-monitor DPI awareness).
  *(Verify: manual)*
- **UI-14.4** Given the UI scale setting (100 % to 200 % in 25 % steps), when the user changes it,
  then it multiplies the Windows scale. *(Verify: manual)*
- **UI-14.5** Given a standard Windows 10 or 11 install, when Japanese test text is shown, then the
  UI font is the system UI font (Segoe UI) with CJK fallback (Yu Gothic UI, Meiryo, MS UI Gothic)
  and no missing-glyph boxes appear. The text-pane font and size are chosen separately;
  *(Could)* per pane. *(Verify: manual)*

### UI-15 Original Text pane and Japanese input

**Priority:** Must · **Milestone:** M2 · **Decisions:** D5, D11, D14 · **Legacy:** `ui-reference.md`
§4 "Original Text extras", §10 item 13; `features.md` §1 pane 0, §3 "Manual input" ·
**Depends on:** IN-02, IN-03, IN-05

As a user, I want to type or paste Japanese into the Original Text pane, so that I can translate
text that no hooker or clipboard delivered.

**Acceptance criteria**

- **UI-15.1** Given the Original Text pane has focus, when the user types with the Windows Japanese
  IME, then the composition string is shown inline at the caret and the candidate window opens next
  to the caret. Committing the composition inserts the text and does not send it for translation by
  itself (IN-03.4). *(Verify: manual)*
- **UI-15.2** Given text in the Original Text pane, when the user clicks "Translate All" or presses
  Ctrl-Enter, then the text goes through the pipeline to every enabled pane (IN-03.1).
  *(Verify: integration)*
- **UI-15.3** *(Should, with IN-04)* Given "Auto translate highlighted text" is on, when the user
  selects text in the Original Text pane with the mouse or keyboard, then only the selection is
  translated, as in IN-04.1. *(Verify: integration)*
- **UI-15.4** Given the current text, when the Original Text pane shows it, then it shows which
  input delivered it (clipboard, local API with client name, hook plugin, typed) and the context
  name (IN-05.5). *(Verify: unit)*
- **UI-15.5** Given automatic input is active, when the user presses the pause button, then
  automatic input (clipboard, local API and plugins) stops as in IN-02.2 without losing typed
  input, and the pause state shows in the pane and the status line. *(Verify: integration)*
- **UI-15.6** Given the pane holds 10 000 characters, when the user types, then typing stays within
  the frame-stall budget of NFR-03.3. If the iced editor fails UI-15.1 in the UI-01 spike, a native
  edit control is used for this pane and the criteria apply to it. *(Verify: manual)*

### UI-16 First-run guided setup

**Priority:** Must · **Milestone:** M2 (dictionary step M4) · **Decisions:** D5, D16, D17, D11 ·
**Legacy:** `ui-reference.md` §10 item 16 · **Depends on:** UI-13, CFG-06

As a new user, I want a short guided setup, so that my first copied line gets translated without
reading a manual.

**Acceptance criteria**

- **UI-16.1** Given no settings file exists, when the app starts, then a first-run flow opens with
  at most 5 steps: target language, import legacy settings (optional, CFG-06.1), add one
  translator, test it, dictionaries (optional, *M4*). *(Verify: manual)*
- **UI-16.2** Given the first-run flow, when the language step is shown, then the target language
  defaults to the Windows display language if a provider supports it, otherwise English, and the
  source defaults to Japanese. *(Verify: unit)*
- **UI-16.3** Given the "Add one translator" step, when it is shown, then it lists the official
  presets first (D17); presets that need a key ask for it in a masked field, and LibreTranslate
  asks for a URL. Unofficial examples are not offered in this flow (D21). "Test" translates a
  sample sentence and shows the result or the error (UI-10.3). *(Verify: manual)*
- **UI-16.4** Given any step, when the user skips it or finishes the flow, then the main window is
  usable with clipboard input; if no translator was added, the empty state of UI-10.4 offers to
  add one. *(Verify: manual)*
- **UI-16.5** Given the Help menu or the palette, when the user starts the flow again, then it
  runs again, and it never turns on auto translate for a remote provider without the confirmation
  in UI-13.6. *(Verify: manual)*

## 4. Out of scope

| Item | Reason |
|---|---|
| Concrete screen design (pixel layouts, icons, exact colours) | Decided in the UI design step (UI-01), not in this spec |
| ATLAS options, ATLAS menu/dialog translation | Cut (D7) |
| Launch/inject dialog, AGTH code converter, launch profiles, drop-`.exe`-to-inject | Hook plugin UI (D6, see the HOOK spec) |
| Configuration UI for legacy `__stdcall` plugin DLLs | Not supported (D15) |
| Legacy dead UI: borderless mode, reset placement (Ctrl-P), splitter colour, colours stored in layouts | Dead code in legacy (`features.md` §5); theming covers colours |
| Copying legacy resource scripts, toolbar bitmap or icons | Clean reimplementation (D13) |
| Furigana view, dictionary tooltip content and behaviour | JP spec |

## 5. Open questions

| # | Effect on this spec | Assumption used |
|---|---|---|
| O10 | Default of the auto-translate toggle for remote panes (UI-09.2, UI-13.6, UI-16.5) | Off, with a confirmation when the user turns it on |
| O11 | Dictionary step in the first-run flow (UI-16.1) | Download on first use with licence shown; step is optional |
| O12 | Warning on the local API settings page (UI-13.1) | Token required for the output stream when it is on (API-10.2) |
| O14 | Menu bar of UI-02.5, UI-07.5, UI-07.6 | Either native or iced-drawn; decided in UI-01 after a spike. The criteria hold for both |
| New (for UI-01) | Menu access keys clash with legacy Alt shortcuts (Alt-F, Alt-T, maybe Alt-A, Alt-S, Alt-X) | Legacy shortcuts win; menus open with Alt alone or F10 (UI-07.6) |

Known iced risks that criteria depend on (all spiked in UI-01.4): whole-window transparency
(UI-06.1), pane drag between top-level windows (UI-03.3), Japanese IME (UI-15.1, with the native
edit control fallback), per-monitor DPI changes (UI-14.3), and screen-reader names (NFR-07.5).

## 6. Traceability

| Feature | Priority | Milestone | Decisions |
|---|---|---|---|
| UI-01 UI design step before the GUI is built | Must | M2 (precondition) | D11, D14, D13 |
| UI-02 Main window and panes | Must | M2 | D5, D11, D12 |
| UI-03 Frames and moving panes | Must | M2 | D11, D14 |
| UI-04 Columns and splitters | Must | M2 | D11 |
| UI-05 Saved layouts | Must | M2 | D11, D16 |
| UI-06 Frame display options | Must | M2 | D11, D14, D20 |
| UI-07 Keyboard shortcuts | Must | M2 | D11, D6, D20 |
| UI-08 Source and target languages | Must | M2 | D5, D10, D11 |
| UI-09 Pane header and actions | Must | M2 | D11, D17, D21, D20 |
| UI-10 Pane status, states and provider health | Must | M2 (cost/streaming M3, API status M5) | D11, D20 |
| UI-11 History view | Should (navigation is PIP-11) | M2 | D11, D12 |
| UI-12 Command palette | Should | M2 | D11 |
| UI-13 Settings UI | Must | M2 (dictionaries M4, local API M5, plugins M6) | D16, D10, D17, D21, D18 |
| UI-14 Appearance: themes, high-DPI and fonts | Must | M2 | D11, D14 |
| UI-15 Original Text pane and Japanese input | Must | M2 | D5, D11, D14 |
| UI-16 First-run guided setup | Must | M2 (dictionary step M4) | D5, D16, D17, D11 |
