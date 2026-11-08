# 01 Text input (`IN`)

## 1. Purpose and scope

This area covers how text gets into the app and what happens to it before the pipeline's
substitution step (see PIP-01 for the full order):

- the clipboard listener,
- typed and pasted text in the Original Text pane, and translating highlighted text,
- **input contexts**: one per input stream (clipboard, typed text, each local API source, each
  plugin or hook plugin source), each with its own options,
- the per-context options of the legacy Context Manager: auto translate, disable, Japanese only,
  merge, idle delay, repeat and phrase filters, text-loop detection, line-break modes, auto copy to
  clipboard, max history,
- automatic half-width → full-width katakana and romaji → hiragana normalisation (IN-11; this is
  the only spec for it, the JP spec refers here).

Not covered here: the HTTP/WebSocket transport of the local API (see the API spec), substitutions
and translation (see the PIP spec), the hook plugin's capture (see the HOOK spec), and the look of
the Original Text pane and the context panel (see the UI spec).

## 2. Goals

1. Text copied in any program is captured reliably and shown fast, without the user switching
   windows (D5).
2. Every input source goes through the same contexts and filters, so game text from the hook
   plugin, an OCR tool or the clipboard behaves the same (D5, D6).
3. Noisy input (repeated characters, progressive redraws, looping text, stray line breaks) is
   cleaned before anything is sent to a translator, with the legacy filter results as the
   reference (D4).
4. Sensitive or unwanted clipboard content is not captured, and capture can be paused at once.
5. Input handling never blocks the UI (NFR-03.3).

## 3. Features

### IN-01 Clipboard capture

**Priority:** Must · **Milestone:** M1 · **Decisions:** D5 · **Legacy:** features.md §3 "Clipboard monitoring" · **Depends on:** IN-05, PIP-01

As a user, I want text I copy to be picked up automatically, so that I can read a game or a web
page without switching windows. Existing text hookers that write to the clipboard work without any
plugin.

**Acceptance criteria**

- **IN-01.1** Given clipboard capture is on, when another program copies Japanese text, then the
  text enters the pipeline in context `clipboard` and is shown in the Original Text pane (in M1:
  the developer console harness) within the budget in NFR-03.1. *(Verify: integration)*
- **IN-01.2** Given clipboard capture is on, when the clipboard changes to content without a text
  format (an image, a file list), then nothing enters the pipeline and no error is shown.
  *(Verify: integration)*
- **IN-01.3** Given the clipboard holds only whitespace or an empty string, when it changes, then
  nothing enters the pipeline. *(Verify: unit)*
- **IN-01.4** Given the user copies text from one of the app's own panes, when the clipboard
  changes, then the app does not capture its own copy as new input. *(Verify: integration)*
- **IN-01.5** Given the clipboard text contains invalid UTF-16 (an unpaired surrogate), when it is
  read, then the bad unit is replaced with U+FFFD, the rest of the text is kept, and the app does
  not crash. *(Verify: unit)*
- **IN-01.6** Given clipboard capture is on, when the app runs as the 32-bit and as the 64-bit
  build, then IN-01.1 to IN-01.5 pass on both. *(Verify: CI)*

### IN-02 Clipboard privacy and robustness

**Priority:** Must · **Milestone:** M1 · **Decisions:** D5, D20 · **Legacy:** features.md §3 "Clipboard monitoring" (legacy viewer chain replaced) · **Depends on:** IN-01

As a user, I want passwords and other private clipboard content to be left alone, and I want to be
able to stop capture at once, so that private text never reaches a translator.

**Acceptance criteria**

- **IN-02.1** Given a password manager puts text on the clipboard together with the Windows format
  that asks monitors to skip it (`ExcludeClipboardContentFromMonitorProcessing`), when the
  clipboard changes, then the text is not captured, not shown and not stored in history.
  *(Verify: integration)*
- **IN-02.2** Given automatic input is paused (toolbar button, shortcut or command), when the
  clipboard changes, then nothing is captured; when input is resumed, then older clipboard
  content is not captured afterwards, only the next change. *(Verify: integration)*
- **IN-02.3** Given another program keeps the clipboard open, when the app tries to read it, then
  it retries for up to 500 ms off the UI thread, then skips that change and logs one warning; the
  UI keeps responding (NFR-03.3). *(Verify: integration)*
- **IN-02.4** Given the same program updates the clipboard 50 times within one second, when the
  changes arrive, then the app reads each change at most once, the UI stays responsive
  (NFR-03.3), and the repeated-text rule in IN-07 decides what becomes an entry.
  *(Verify: integration)*
- **IN-02.5** Given clipboard capture is turned off in settings, when the clipboard changes, then
  the app does not read the clipboard at all. *(Verify: integration)*

### IN-03 Typed and pasted text

**Priority:** Must · **Milestone:** M2 · **Decisions:** D5, D11 · **Legacy:** features.md §3 "Manual input + Translate All / per-pane translate" · **Depends on:** PIP-04, UI-15

As a user, I want to type or paste text into the Original Text pane and translate it on demand, so
that I can translate text that did not arrive automatically or retry with edits.

**Acceptance criteria**

- **IN-03.1** Given text in the Original Text pane, when the user runs "Translate All", then the
  text is sent as one entry in context `typed` to every enabled translator, whether or not its
  auto-translate option is on. *(Verify: integration)*
- **IN-03.2** Given text in the Original Text pane, when the user runs "Translate" on one pane,
  then only that translator receives the text. *(Verify: integration)*
- **IN-03.3** Given the Original Text pane is empty or holds only whitespace, when the user looks
  at the translate actions, then they are disabled, and the shortcut does nothing.
  *(Verify: manual)*
- **IN-03.4** Given the user types Japanese with an IME, when a composition is still open, then no
  translation starts; editing the pane never starts a translation by itself. *(Verify: manual)*
- **IN-03.5** Given typed text, when it is translated, then buffering, merge, repeat filters and
  the Japanese-only filter (IN-06 to IN-09) are skipped, and normalisation (IN-11) and
  substitutions (PIP-02) still apply. *(Verify: unit)*
- **IN-03.6** Given the user pastes text as long as the maximum input length (NFR-04.4), when the
  paste completes, then the pane shows the text within 1 s and the UI stays responsive (NFR-03.3).
  *(Verify: manual)*

### IN-04 Translate highlighted text

**Priority:** Should · **Milestone:** M2 · **Decisions:** D5, D12 · **Legacy:** features.md §3 "Auto translate highlighted text in Original Text"; formats-and-interfaces.md §1 `[Original Text] Trans Highlighted` · **Depends on:** IN-03

As a user, I want a selected part of the original text to be translated on its own, so that I can
check a single phrase without copying it.

**Acceptance criteria**

- **IN-04.1** Given "Auto translate highlighted text" is on, when the user selects part of the
  Original Text and the selection stays unchanged for 300 ms, then only the selected text is sent
  as an entry in context `typed` to the translators with auto-translate on. *(Verify: integration)*
- **IN-04.2** Given the option is on, when the selection is empty or only whitespace, then nothing
  is sent. *(Verify: unit)*
- **IN-04.3** Given the user drags the selection across text for 2 s, when the selection keeps
  changing, then no request is sent until it has been stable for 300 ms. *(Verify: unit)*
- **IN-04.4** Given the option is off (the default), when the user selects text, then nothing is
  translated. *(Verify: unit)*
- **IN-04.5** Given a legacy INI with `Trans Highlighted` set, when it is imported (CFG-06), then
  the option has the same value. *(Verify: parity)*

### IN-05 Input contexts

**Priority:** Must · **Milestone:** M1 · **Decisions:** D5, D6 · **Legacy:** features.md §3 "Context Manager dialog"; architecture.md §2.2 "Context buffering" · **Depends on:** CFG-01

As a user, I want each input stream to have its own context with its own options, so that the
clipboard, a game hooked by the hook plugin and an OCR tool can be filtered differently.

**Acceptance criteria**

- **IN-05.1** Given text arrives, when it enters the app, then it belongs to exactly one context:
  `clipboard`, `typed`, `api:<source>` for local API input (API-02), or `plugin:<plugin-id>/<source>`
  for input-source plugins (PLG-07). Hook plugin text arrives through the local API with the
  context id defined in the HOOK spec. *(Verify: unit)*
- **IN-05.2** Given text arrives with a context id the app has not seen, when it is accepted, then
  a new context is created with the options of the "new context" template and is listed in the
  context panel (IN-12, from M2). *(Verify: unit)*
- **IN-05.3** Given a context id that is empty, longer than 128 characters, or contains control
  characters, when it is submitted, then the input is rejected with a clear error and no context
  is created. *(Verify: unit)*
- **IN-05.4** Given a context's options are changed, when the app restarts, then the options are
  kept (stored in TOML, CFG-01), keyed by context id. *(Verify: integration)*
- **IN-05.5** Given an entry is shown in the Original Text pane, when the user looks at it, then
  the pane shows which input delivered it (clipboard, typed, API client, plugin or hook plugin).
  *(Verify: manual)*
- **IN-05.6** Given any automatic input longer than the maximum input length (NFR-04.4), when it
  arrives, then it is not accepted, a status note says why and how long it was, and the app keeps
  running. *(Verify: unit)*

### IN-06 Buffering, idle flush and merge

**Priority:** Must · **Milestone:** M1 · **Decisions:** D4, D5 · **Legacy:** features.md §3 "Context Manager dialog" (idle delay, merge stream/on flush); architecture.md §2.2 · **Depends on:** IN-05

As a user, I want text that a game sends in pieces to be joined into one line before it is
translated, so that each translator gets whole sentences.

**Acceptance criteria**

- **IN-06.1** Given a context with an idle delay of 250 ms, when three pieces arrive 50 ms apart,
  then they are joined in arrival order and flushed as one entry 250 ms after the last piece.
  *(Verify: unit)*
- **IN-06.2** Given a context with an idle delay of 0 ms, when a piece arrives, then it is flushed
  at once as its own entry. *(Verify: unit)*
- **IN-06.3** Given default settings, when contexts are created, then `clipboard` and `typed`
  have an idle delay of 0 ms and API and plugin contexts have 250 ms. *(Verify: unit)*
- **IN-06.4** Given two contexts set to "merge on flush", when both flush, then their flushed text
  goes into the `merged` context, which flushes and translates it as one entry; the two source
  contexts create no entries of their own. *(Verify: parity)*
- **IN-06.5** Given two contexts set to "merge stream", when pieces arrive in both, then each piece
  goes into the `merged` context as it arrives, and `merged` applies its own idle delay.
  *(Verify: parity)*
- **IN-06.6** Given a context is changed to "disabled" while text is buffered, when the change is
  saved, then the buffered text is dropped and not translated. *(Verify: unit)*

### IN-07 Japanese-only filter and repeated-text suppression

**Priority:** Must · **Milestone:** M1 · **Decisions:** D4, D5 · **Legacy:** features.md §3 "Context Manager dialog" (Japanese only) · **Depends on:** IN-06

As a user, I want text without Japanese in it, and the same line arriving again, to be ignored, so
that translators (and my budget) are not used for noise.

**Acceptance criteria**

- **IN-07.1** Given "Japanese only" is on for a context, when an entry contains no hiragana, no
  katakana (including half-width katakana) and no CJK ideograph, then it is dropped and not shown
  as a new entry. *(Verify: unit)*
- **IN-07.2** Given "Japanese only" is on, when an entry contains at least one such character
  (e.g. `OK、了解`), then it is kept unchanged. *(Verify: unit)*
- **IN-07.3** Given "Japanese only" is on, when romaji-only text arrives, then it is dropped before
  romaji → hiragana normalisation runs (order in PIP-01.1), as in the legacy app. *(Verify: parity)*
- **IN-07.4** Given "ignore repeated text" is on (the default), when a flushed entry is identical to
  the previous entry of the same context, then no new entry is created and no request is sent.
  *(Verify: unit)*
- **IN-07.5** Given the same text arrives in two different contexts, when both flush, then both
  create entries, and the second is answered from the cache (PIP-08). *(Verify: integration)*
- **IN-07.6** Given an entry was suppressed as repeated text, when the user runs "Translate All",
  then the text is translated again (manual actions always run). *(Verify: integration)*

### IN-08 Repeat, phrase and text-loop filters

**Priority:** Must · **Milestone:** M1 · **Decisions:** D4, D20 · **Legacy:** features.md §3 "Context Manager dialog" (repeat-char and phrase filters, text-loop detection); ui-reference.md §7 "Context Manager" · **Depends on:** IN-06

As a user, I want the typical garbage from game text (each character drawn twice, a phrase drawn
letter by letter, the whole line repeated) to be cleaned up, so that translators get the real line.

**Acceptance criteria**

- **IN-08.1** Given the character-repeat filter is "preset 2", when `ここんんににちちはは` arrives,
  then the entry is `こんにちは`; with "none", it is unchanged. *(Verify: parity)*
- **IN-08.2** Given the character-repeat filter is "auto constant", when every character of an entry
  is repeated the same number of times N ≥ 2, then each run is reduced to one character; when the
  counts differ, the entry is unchanged. "Auto variable" reduces each run independently. "All"
  reduces every run of identical characters to one. *(Verify: parity)*
- **IN-08.3** Given the phrase-extension filter is on, when one flush holds `こ`, `こん`, `こんに`,
  `こんにちは` in sequence (each piece extends the one before), then the entry is `こんにちは`.
  *(Verify: parity)*
- **IN-08.4** Given the phrase-repeat filter is on with min 2 and max 40 characters, when an entry is
  one phrase repeated (`はいはいはい` → phrase `はい`, or a 10-character line repeated 3 times),
  then the entry is the phrase once; phrases shorter than min or longer than max are left as they
  are. *(Verify: parity)*
- **IN-08.5** Given text-loop detection is on, when a context's entries repeat a cycle of up to 8
  entries (A, B, C, A, B, C, …), then after the first full cycle the repeats are dropped until an
  entry outside the cycle arrives. *(Verify: parity)*
- **IN-08.6** Given the Phase-0 filter fixtures, when they are replayed, then outputs match the
  legacy app except for divergences listed in the behaviour-changes list (D20). *(Verify: parity)*

### IN-09 Line-break modes

**Priority:** Must · **Milestone:** M1 · **Decisions:** D4, D20 · **Legacy:** features.md §3 "Context Manager dialog" (line-break modes); ui-reference.md §7 "Context Manager" · **Depends on:** IN-06

As a user, I want to choose whether line breaks in incoming text are kept, so that text wrapped by
a game window is translated as one sentence.

**Acceptance criteria**

- **IN-09.1** Given the mode "remove", when an entry contains line breaks, then all are removed;
  between two Japanese characters nothing is inserted, otherwise one space is inserted (behaviour
  change, listed under D20). *(Verify: unit)*
- **IN-09.2** Given the mode "keep", when an entry contains line breaks, then they are kept.
  *(Verify: unit)*
- **IN-09.3** Given the mode "keep some" with "first" = 1 and "last" = 0, when an entry has three
  line breaks, then only the first is kept. *(Verify: parity)*
- **IN-09.4** Given any mode, when an entry contains `\r\n` or a lone `\r`, then each becomes `\n`
  before the mode is applied, and leading and trailing whitespace of the entry is trimmed.
  *(Verify: unit)*
- **IN-09.5** Given an entry that consists only of line breaks and spaces, when the mode is applied,
  then the entry is empty and is dropped. *(Verify: unit)*

### IN-10 Per-context actions: auto translate, disable, auto copy, max history

**Priority:** Must · **Milestone:** M1 · **Decisions:** D5, D20 · **Legacy:** features.md §3 "Context Manager dialog" (auto translate, disable, auto clipboard, max history) · **Depends on:** IN-05, PIP-04

As a user, I want to decide per context whether its text is translated, ignored or copied, so that
I control which streams reach my translators.

**Acceptance criteria**

- **IN-10.1** Given "auto translate" is off for a context, when it flushes an entry, then the entry
  is shown and stored in history, and no translator receives it until the user asks.
  *(Verify: integration)*
- **IN-10.2** Given a context is "disabled", when text arrives in it, then nothing is shown, stored
  or translated, and the context panel counts the dropped pieces. *(Verify: unit)*
- **IN-10.3** Given "auto copy to clipboard" is on for a context, when it flushes, then the
  entry text is put on the clipboard, and IN-01.4 keeps the app from capturing it again.
  *(Verify: integration)*
- **IN-10.4** Given a context's max history is 64 KB, when its kept text grows beyond 64 KB, then
  the oldest text is dropped first, and memory for that context stays bounded.
  *(Verify: unit)*
- **IN-10.5** Given "clear history" is run on a context, when it completes, then that context's
  kept text is empty and other contexts are unchanged. *(Verify: unit)*

### IN-11 Automatic normalisation (half-width katakana, romaji)

**Priority:** Should · **Milestone:** M2 · **Decisions:** D4, D12 · **Legacy:** features.md §2 "Auto half-width to full-width katakana", "Auto romaji to hiragana when no Japanese is present" · **Depends on:** PIP-01, JP-10

As a user, I want half-width katakana and romaji input to be turned into normal Japanese before
translation, so that translators and dictionaries understand it.

This step is stage 2 of PIP-01.1 and applies to every input. JP-11 is withdrawn in favour of this
feature.

**Acceptance criteria**

- **IN-11.1** Given "auto convert half-width katakana" is on, when `ｶﾀｶﾅ` arrives, then the text
  passed on is `カタカナ`, and `ｶﾞ` (with the half-width voiced mark) becomes `ガ`.
  *(Verify: parity)*
- **IN-11.2** Given the option is on, when text contains half-width ASCII letters and digits, then
  they are unchanged (this is not general NFKC). *(Verify: unit)*
- **IN-11.3** Given "auto convert to hiragana when no Japanese" is on, when `konnichiha` arrives,
  then the text passed on is `こんにちは`. *(Verify: parity)*
- **IN-11.4** Given that option is on, when the text already contains any Japanese character, then
  it is unchanged. *(Verify: unit)*
- **IN-11.5** Given both options are off (the default), when any text arrives, then it is
  unchanged by this step. *(Verify: unit)*

### IN-12 Context panel

**Priority:** Should · **Milestone:** M2 · **Decisions:** D11 · **Legacy:** ui-reference.md §7 "Context Manager" · **Depends on:** IN-05, UI-13

As a user, I want a live list of contexts where I can see their latest text and change their
options, so that I can tune filters while a game is running.

**Acceptance criteria**

- **IN-12.1** Given the context panel is open, when text arrives in a context, then that context's
  row shows the latest text within 1 s and the time it arrived. *(Verify: manual)*
- **IN-12.2** Given the user changes a filter option of a context, when the next piece arrives,
  then the new option applies without a restart. *(Verify: integration)*
- **IN-12.3** Given 200 contexts exist, when the panel is opened and scrolled, then the UI stays
  within NFR-03.3. *(Verify: manual)*
- **IN-12.4** Given the user deletes a context, when the source sends text again, then the
  context is created again with the template options (IN-05.2). *(Verify: unit)*
- **IN-12.5** Given the panel, when the user uses only the keyboard, then every option and action
  can be reached. *(Verify: manual)*

## 4. Out of scope

- Hook injection, hook codes and the per-process "hook port" setting: moved to the hook plugin (D6).
  The core only sees hook text as local API input.
- Launching or attaching AGTH: cut; external hookers use the clipboard or the local API
  (features.md §3).
- The legacy clipboard viewer chain: replaced by a modern clipboard listener (D5, D20).
- OCR or screen capture inside the app: external tools send text through the local API (D5).
- Profiles that switch when a matching process is running: replaced by profiles keyed by context
  (PIP-02, CFG-11).

## 5. Open questions

None of O10–O14 affects this area directly.

Notes for the owner (not yet O items):

- The exact meaning of the legacy "keep some first/last" line-break mode is taken from the
  Phase-0 fixtures; IN-09.3 assumes "keep the first N and the last M line breaks".
- The maximum input length is a shared number (NFR-04.4), used by IN-05.6, IN-03.6 and API-07.1.

## 6. Traceability

| Feature | Priority | Milestone | Decisions |
|---|---|---|---|
| IN-01 Clipboard capture | Must | M1 | D5 |
| IN-02 Clipboard privacy and robustness | Must | M1 | D5, D20 |
| IN-03 Typed and pasted text | Must | M2 | D5, D11 |
| IN-04 Translate highlighted text | Should | M2 | D5, D12 |
| IN-05 Input contexts | Must | M1 | D5, D6 |
| IN-06 Buffering, idle flush and merge | Must | M1 | D4, D5 |
| IN-07 Japanese-only filter and repeated-text suppression | Must | M1 | D4, D5 |
| IN-08 Repeat, phrase and text-loop filters | Must | M1 | D4, D20 |
| IN-09 Line-break modes | Must | M1 | D4, D20 |
| IN-10 Per-context actions | Must | M1 | D5, D20 |
| IN-11 Automatic normalisation | Should | M2 | D4, D12 |
| IN-12 Context panel | Should | M2 | D11 |
