# M2 desktop UI design

Draft for owner review. Approval date and the results of the Windows spikes are pending.
This design follows [the legacy UI reference](analysis/ui-reference.md) and
[UI-01](specs/07-user-interface.md#ui-01-ui-design-step-before-the-gui-is-built).

## Workspace

Use an ordinary Windows title bar and a compact menu bar. Below it, place the source and target
language pickers and a swap button. The initial frame is opaque with two equally wide columns,
visible pane headers, and Original Text first. The status bar reports automatic input state,
busy provider count, and whether the local API is active once M5 adds it.

```text
File  Tools  View  Window  Providers  Help
[ Japanese v ]  [ Swap ]  [ English v ]
+---------------------------------+---------------------------------+
| Original Text      [Pause]  […] | LibreTranslate    Ready    […] |
| Clipboard · clipboard           | [Translate] [Copy]    Auto: Off |
|                                 |                                 |
| こんにちは                      | Hello                           |
| [Translate All]                 |                                 |
+---------------------------------+---------------------------------+
  Clipboard active · 0 providers busy
```

The pane header contains a grip, title, status text with status icon, primary action, overflow
menu, and close button. Provider headers show elapsed time after a result. At narrow widths,
secondary actions move into the overflow menu in this order: Copy, Pin, Auto-translate, Settings.
The Original Text pane shows input source and context below its header. The original and
processed text are accessible from the pane, and typing or committing IME text leaves translation
under user control. Closing a pane hides it and stops its automatic requests.

| Menu | Commands |
| --- | --- |
| File | Settings, import legacy settings, enable global shortcuts, quit |
| Tools | Translate All, pause/resume automatic input, contexts, substitutions, kana conversions, command palette |
| View | History, named and numbered layouts, font settings, pane headers, frame chrome, always on top, opacity |
| Window | New frame, show all panes, one/two columns, lock order, list of pane visibility toggles |
| Providers | Provider documentation and account-console links |
| Help | First-run setup, keyboard shortcuts, about |

Menus, the palette and pane context menus share command state. With the frame chrome hidden,
the palette and context menus continue to expose the same commands. Language pickers show recent
pairs and searchable language names. Selecting the opposite language swaps the pair. Unsupported
language pairs disable the pane's Translate action and explain the restriction.

### Drag and layout

Show insertion targets at the top and bottom of each pane and at the column split. In a single
column, an outer-edge target opens the second column. A drop in another frame inserts the pane
into the highlighted slot. A drop outside all frames tears the pane into a new frame. Esc cancels.
A locked source or destination frame shows a blocked cursor and keeps its layout. The same
destinations appear in **Move pane to** in the pane menu and palette, including **New frame**.
Splitters support keyboard resizing and reset. Save frame geometry, pane order, opacity and
chrome in layouts and clamp restored windows onto a visible monitor.

## Settings and first run

Settings use a category list on the left and a form on the right. Pages cover General, Languages,
Translators, Keys, Inputs, Contexts and Filters, Substitutions, Appearance, and Shortcuts. Translators has a
provider list, definition controls, connection and automatic-translation settings, and a Test
section. Save applies valid changes to all frames. Invalid fields show the correction beside the
field and retain focus when Save fails. Credential fields support Replace and Remove and display
only whether a key exists. A remote-provider confirmation names the provider, says text leaves
the device and may cost money, and preserves the previous setting when cancelled.

Start with the target language, then offer legacy import, one official provider, and a connection
test. Default to Japanese input and the supported Windows display language for output, with
English as the fallback. Show an import summary and retain the original files. Ask for provider
settings or a masked key as appropriate. Allow Skip at each step and offer Add a translator in
the workspace if setup ends without one. Setup is available again from Help and the palette.
Dictionaries appear in setup and Settings in M4, Local API in M5, and Plugins in M6.

## Visual tokens, icons and states

Use system UI typography, with Segoe UI and the Windows CJK fallback chain. Set the UI body
to 13 logical px and the initial text-pane body to 16 logical px, independently adjustable.
Use 4/8/12 logical px spacing, 1 px borders, square corners for frames and panes, and a 2 px
focus outline. Set header controls to at least 28 logical px in both dimensions. Multiply the
Windows monitor scale by the user-selected 100–200% UI scale.

| Token | Classic light | Dark |
| --- | --- | --- |
| Frame and headers | Windows 3D face | #252A32 |
| Text surface | Windows window | #171B22 |
| Text | Windows window text | #F1F3F6 |
| Secondary text | Windows window text | #BBC4D2 |
| Divider and shadow | Windows 3D shadow | #6C7787 |
| Selection and focus | Windows highlight / highlight text | #7EB6FF / #102033 |
| Tooltip | Windows info background / info text | #303846 / #F1F3F6 |
| Ready / warning / error | #006343 / #754000 / #95233A | #91E0B4 / #FFC982 / #FFABB8 |

Use new monochrome line icons for grip, play, retry, copy, pin, pause/resume, settings,
close, history, search, swap, chevron, local/remote, and status. Give each action a visible
label or tooltip and an accessible name. Status text and icons carry the meaning independently
of colour. Keep the system palette for Windows high-contrast settings.

| State | Display and action |
| --- | --- |
| Empty input | Focusable editor. Translate All becomes available when there is text. |
| No provider | Add a translator opens Translators. |
| Busy | Busy text and progress indicator, with the previous result kept until completion. |
| Ready or cached | Result remains selectable and copyable. Cached results show Cached. |
| Key missing, configuration invalid, or language unsupported | Specific reason and the relevant Settings action. |
| Timeout, quota, or service error | Sanitized explanation and Retry when the action applies. |
| Automatic input paused | Resume in Original Text and status in every frame. Typed translation stays available. |

## Keyboard map

| Keys | Action |
| --- | --- |
| Alt+0–9 / Shift+Alt+0–9 | Restore / save numbered layout |
| Ctrl+0 / Ctrl+2 | Show all panes / toggle one or two columns |
| Ctrl+T / Ctrl+F | Toggle pane headers / frame chrome |
| Alt+A / Ctrl+Num− / Ctrl+Num+ | Toggle always on top / more transparent / less transparency |
| Alt+F / Alt+T | Open main / tooltip font settings |
| Alt+S / Ctrl+M | Open substitution / context settings |
| Alt+Up or Alt+Page Up / Alt+Down or Alt+Page Down | Back / forward through history |
| Ctrl+H / Ctrl+K / Ctrl+R | Convert selection or text to hiragana / katakana / romaji |
| Alt+X | Quit |
| Ctrl+Shift+P / Ctrl+, / Ctrl+Shift+H | Open command palette / Settings / History |
| Ctrl+Tab or F6 / Ctrl+Shift+Tab or Shift+F6 | Next / previous pane |
| Ctrl+Enter | Translate All from Original Text |
| Alt alone or F10 / Shift+F10 or Menu | Focus menu bar / open focused item's context menu |
| Tab and Shift+Tab / arrows / Enter or Space / Esc | Move focus / navigate choices / activate / dismiss and restore focus |

Legacy accelerators take precedence over menu access keys. Global shortcuts are off by default.
When enabled, Ctrl+Alt+Up/Page Up and Ctrl+Alt+Down/Page Down traverse history, Ctrl+Alt+A toggles
always on top, and Ctrl+Alt+Num−/Num+ adjust transparency. Keep the core's Ctrl+L unbound.
The pane menu and palette expose pane moves, splitter reset and actions without dragging.

## Legacy modernization decisions

| Reference §10 | Proposed disposition |
| --- | --- |
| 1. Pane header | Adopt in M2. LLM usage and cost indicators arrive with M3. |
| 2. Drag handle and drops | Adopt in M2, including keyboard moves and tear-out. |
| 3. Settings | Adopt in M2, with LLM in M3, dictionaries in M4 and Local API in M5. |
| 4. Themes | Adopt in M2. Furigana highlight tokens apply in M4. |
| 5. DPI and fonts | Adopt in M2. |
| 6. Keyboard accessibility | Adopt in M2. Dictionary tooltip keyboard access applies in M4. |
| 7. Command palette | Adopt in M2. |
| 8. Searchable history | Adopt in M2, including copy, retranslating and export. |
| 9. Language pickers | Adopt in M2. |
| 10. Comparison | Adopt as M2 Could scope after required pane behavior. |
| 11. Named layouts | Adopt in M2 alongside numbered slots and autosave. |
| 12. Dictionary tooltips | Defer to M4 when the dictionary panes ship. |
| 13. Source and pause | Adopt in M2, with API and plugin identifiers as those sources ship. |
| 14. Status line | Adopt in M2. Local API state activates with M5. |
| 15. Transparency | Adopt the per-frame slider in M2. Propose rejecting click-through because recovery would depend on a working global shortcut. |
| 16. First run | Adopt in M2. Defer optional dictionary download to M4. |

## Windows spikes and approval

Use the same prototype, scenarios and acceptance observations on i686 and x86_64 Windows.
Every row is pending on both targets. Record the tested commit, OS and evidence before choosing
the final control or mitigation.

| Spike | Test and proposed decision rule |
| --- | --- |
| Drawn vs native menu bar | Compare keyboard, focus, Alt precedence and screen-reader announcements. Use the first approach that meets the keyboard and accessibility spec while preserving borderless commands. |
| Whole-window transparency | Change alpha while resizing, switching focus and restoring a layout. If toolkit support fails, test the Win32 handle path. |
| Cross-window pane drag | Drag between two frames and tear out; repeat with a locked target. Prototype OS-level pointer tracking if toolkit drag ends at a window boundary. |
| Japanese IME | Compose, navigate candidates and commit in the editor. Use a native edit control if caret positioning or inline composition fails. |
| Multi-window focus | Open and close frames, move panes, and invoke commands after focus changes. Track active frame and restore focus explicitly when needed. |
| Software rendering | Start with GPU acceleration unavailable and exercise scrolling, typing and frame movement. Select the renderer path that meets the performance gates. |
| Per-monitor DPI | Move a frame between 100% and 150% displays and check sharpness, resize and hit testing. Use per-monitor DPI awareness and rebuild layout on scale change. |
| Screen reader | Use Windows Narrator on menus, pane names and states, settings and splitter controls. Validate semantic names and actions before selecting native fallbacks for inaccessible controls. |

Record owner approval and its date after review. If the approved design changes a requirement,
update the existing criterion in the same change and preserve its ID.
