# 06 Japanese tools (`JP`)

## 1. Purpose and scope

The Japanese tools help a reader who is learning or partly reads Japanese. They run on this machine
only and send no text anywhere. This spec covers:

- morphological analysis with MeCab-format dictionaries (D9) and the MeCab pane,
- the JParser pane: segmentation with EDICT/ENAMDICT, de-inflection with self-authored conjugation
  rules (D12, D13), name detection,
- furigana display and hover dictionary tooltips,
- dictionary download, selection and licence display,
- kana/romaji conversion actions (Ctrl-H/K/R) and hex conversion actions.

The automatic kana conversions of incoming text (half-width katakana, romaji to hiragana) are
specified once, in IN-11; JP-11 is withdrawn.

Not covered: substitutions and the pipeline order (see the pipeline spec, `PIP`), pane layout,
fonts and shortcut rebinding (see the UI spec, `UI`), importing legacy `[JParser]`/`[Mecab]` INI
values (see the settings spec, `CFG`).

Parity tests are black-box: the same input in the legacy app and in the rewrite, compared on output
(D13). Divergences caused by new data or by fixed quirks are reviewed and listed, not forced to
match (D20, migration plan phase 4).

## 2. Goals

1. Japanese text in the Original Text pane appears segmented with readings in the MeCab and
   JParser panes within the budget in NFR-03.2.
2. MeCab output matches the legacy MeCab pane when both use the same dictionary.
3. JParser finds dictionary forms of conjugated words using rules the project wrote itself, with no
   legacy-authored data.
4. A new user gets working dictionaries from the settings UI in a few clicks, sees each licence, and
   gets a clear message when a dictionary is missing or unreadable.
5. Furigana, colours and tooltips are readable in light and dark themes, at any DPI, and by
   keyboard.

## 3. Features

### JP-01 Morphological analysis backend

**Priority:** Must · **Milestone:** M4 · **Decisions:** D9, D1, D3 · **Legacy:** features.md §1 "Mecab" · **Depends on:** JP-02

As a user, I want MeCab-style analysis to work in both the 32-bit and the 64-bit app without
installing MeCab, so that furigana and word splitting just work.

**Acceptance criteria**

- **JP-01.1** Given a MeCab-format dictionary selected in settings, when Japanese text is analysed,
  then the built-in pure-Rust analyzer (`vibrato`) returns words with surface form, part of speech,
  base form and reading, in both builds, with no DLL. *(Verify: integration)*
- **JP-01.2** Given dictionary source files in EUC-JP, Shift_JIS or UTF-8, when the dictionary is
  prepared, then the encoding is detected (from the dictionary's charset setting or by sniffing),
  the files are converted, and the compiled dictionary is cached; preparing runs in the background
  with progress, and the pane shows "Preparing dictionary…". UTF-16 dictionaries are refused with a
  clear message. *(Verify: integration)*
- **JP-01.3** Given a prepared dictionary, when it is loaded, then the app checks it by analysing a
  fixed sample (for example 日本語) and expects a katakana reading; if the reading is garbled, then
  the pane shows "Dictionary looks like the wrong encoding; choose the encoding in settings" instead
  of nonsense readings. *(Verify: unit)*
- **JP-01.4** Given the optional backend "installed MeCab" and a `libmecab.dll` of the same bitness
  as the app (found via the registry, the MeCab program folder, or a path the user picks), when it
  is selected, then analysis uses it, and calls to it are serialised. *(Verify: integration)*
- **JP-01.5** Given a `libmecab.dll` of the other bitness, when the user selects it, then the app
  says "This MeCab is 32-bit (or 64-bit); this app needs a matching one. Using the built-in
  analyzer." and keeps working with the built-in backend. *(Verify: integration)*
- **JP-01.6** Given text up to 500 characters, when it is analysed, then the result is shown within
  NFR-03.2, and analysis never blocks the UI (NFR-03.3). *(Verify: integration)*

### JP-02 Dictionary download, selection and licences

**Priority:** Must · **Milestone:** M4 · **Decisions:** D13, D9, D12 · **Legacy:** features.md §1 "JParser" (needs `dictionaries\edict2`) · **Depends on:** CFG-01

As a user, I want to get the dictionaries I need from the settings window and see their licences,
so that I don't have to hunt for files and I know the terms of use.

**Acceptance criteria**

- **JP-02.1** Given **Settings > Dictionaries**, when the user opens it, then it lists the offered
  dictionaries (at least: a MeCab dictionary based on IPADIC, UniDic 2.1.2, EDICT2, ENAMDICT) with
  size, source and licence name, and marks each as installed or not (assumes O11 recommendation:
  download on first use). *(Verify: manual)*
- **JP-02.2** Given the user clicks **Download**, when the download starts, then the full licence
  text and the source URL are shown first and the user must accept; the file is fetched over HTTPS,
  its checksum is verified against the app's catalogue, and progress and **Cancel** are shown.
  *(Verify: integration)*
- **JP-02.3** Given a failed or cancelled download, a checksum mismatch, or a source URL that has
  moved (HTTP 404), when it ends, then no partial dictionary is installed, the previous one stays
  active, and the message offers "Open the source page" and **Use a file I downloaded**.
  *(Verify: integration)*
- **JP-02.4** Given **Use a file I downloaded** (or a folder for MeCab sources), when the user picks
  a file, then it is checked (JP-01.2/JP-01.3 for MeCab, JP-04 for EDICT) before it becomes active;
  the user enters or confirms its licence name for the About page. *(Verify: integration)*
- **JP-02.5** Given installed or bundled dictionaries, when the user opens **Help > About >
  Third-party data**, then each one is listed with its licence text and the required attribution
  (EDRDG for EDICT/ENAMDICT, NAIST/ICOT notice for IPADIC). *(Verify: manual)*
- **JP-02.6** Given no MeCab or EDICT dictionary is installed, when the MeCab or JParser pane is
  shown, then it says "No dictionary installed" with a button to **Settings > Dictionaries**; other
  panes and translators work normally. *(Verify: manual)*

### JP-03 MeCab pane

**Priority:** Must · **Milestone:** M4 · **Decisions:** D9, D11 · **Legacy:** features.md §1 "Mecab"; ui-reference.md §7 "MeCab Configuration" · **Depends on:** JP-01, JP-08

As a user, I want a pane that shows the incoming text split into words with readings, so that I can
read kanji I don't know.

**Acceptance criteria**

- **JP-03.1** Given Japanese text in the Original Text pane, when the MeCab pane updates, then it
  shows the text split into words, with the reading as furigana over words that contain kanji, and
  particles highlighted (JP-08). *(Verify: manual)*
- **JP-03.2** Given a word the dictionary does not know, when it is shown, then it is shown without
  a reading and is not dropped; non-Japanese parts (Latin text, numbers, symbols, line breaks) are
  kept as they are. *(Verify: unit)*
- **JP-03.3** Given the Phase-0 corpus and the same dictionary as the legacy capture (IPADIC 2.7.0),
  when the MeCab pane output is compared with the legacy MeCab pane, then word boundaries and
  readings are the same for every line, or the difference is in the reviewed divergence list.
  *(Verify: parity)*
- **JP-03.4** Given an empty input or text with no Japanese, when the pane updates, then it shows
  the text without furigana and no error. *(Verify: unit)*
- **JP-03.5** Given a word in the MeCab pane and an installed EDICT, when the user hovers it, then
  the tooltip shows MeCab's base form, part of speech and reading plus the EDICT entries for the
  base form (JP-09). *(Verify: manual)*

### JP-04 EDICT and ENAMDICT loading

**Priority:** Must · **Milestone:** M4 · **Decisions:** D12, D13 · **Legacy:** formats-and-interfaces.md §7 "Source dictionaries" · **Depends on:** JP-02

As a user, I want the app to read the EDICT-format files I already have, in any common encoding, so
that I don't have to convert them.

**Acceptance criteria**

- **JP-04.1** Given EDICT, EDICT2 or ENAMDICT files, plain or `.gz`, in UTF-8 (with or without
  BOM), UTF-16 (with BOM), EUC-JP or Shift_JIS, when they are loaded, then all entries are read with
  the same headwords, readings and glosses. `/EntL…/` sequence tags are removed from glosses.
  *(Verify: unit)*
- **JP-04.2** Given a file where encoding detection fails (more than 1 % of characters cannot be
  decoded), when it is loaded, then it is not used and the message says "Could not read <file> as
  <encoding>; choose the encoding", with an encoding picker. *(Verify: unit)*
- **JP-04.3** Given malformed lines, when the file is loaded, then they are skipped, the rest
  loads, and settings show "N lines skipped" with the first line number. *(Verify: unit)*
- **JP-04.4** Given several dictionaries (for example EDICT2 and ENAMDICT), when they are active,
  then all are searched, and each result shows which dictionary it came from. *(Verify: unit)*
- **JP-04.5** Given a loaded dictionary, when the app starts again, then it uses its own cache; the
  cache is rebuilt when the source file's size or modification time changes, when the conjugation
  rules version changes, or when the cache is unreadable. The window is usable while loading
  (NFR-03.3) and the JParser pane shows "Loading dictionary…". *(Verify: integration)*

### JP-05 Conjugation rules and de-inflection

**Priority:** Must · **Milestone:** M4 · **Decisions:** D13, D12 · **Legacy:** formats-and-interfaces.md §7 "`Conjugations.txt`" (not shipped) · **Depends on:** JP-04

As a user, I want conjugated verbs and adjectives traced back to their dictionary form, so that I
can look up words as they appear in real sentences.

**Acceptance criteria**

- **JP-05.1** Given the project's own conjugation rules file, when it is reviewed, then it states
  its author and the grammar references it was written from, and the review checklist confirms it
  contains no legacy-authored content (D13). The legacy `Conjugations.txt` is not shipped.
  *(Verify: manual)*
- **JP-05.2** Given the rules, when they are checked, then they cover godan verbs (all endings,
  including the 行く exception), ichidan verbs, する, 来る, i-adjectives and the copula, with
  negative, past, te-form, polite (ます forms), potential, passive, causative, causative-passive,
  volitional, imperative, conditional (ば, たら), desire (たい), progressive (ている/てる),
  てしまう/ちゃう and adjective forms (く, くて, かった, ければ, そう). *(Verify: unit)*
- **JP-05.3** Given a test table of at least 300 inflected forms with expected dictionary form, word
  class and conjugation chain (for example 食べさせられなかった → 食べる, ichidan,
  "causative-passive, negative, past"), when de-inflection runs, then every expected result is among
  the candidates. *(Verify: unit)*
- **JP-05.4** Given a de-inflected candidate, when it is matched against the dictionary, then it is
  kept only if the entry's part-of-speech tag fits the rule (for example v5k for a godan く verb);
  given 行った, then both 行く (いった) and 行う (おこなった) are offered. *(Verify: unit)*
- **JP-05.5** Given a de-inflected match, when it is shown in a tooltip, then the conjugation chain
  is shown in plain English (for example "Negative, Past"). *(Verify: unit)*

### JP-06 JParser segmentation and name detection

**Priority:** Must · **Milestone:** M4 · **Decisions:** D12, D13, D20 · **Legacy:** features.md §2 "JParser word segmentation" · **Depends on:** JP-04, JP-05, JP-08

As a user, I want the JParser pane to split a sentence into dictionary words and names, so that I can
see each word's meaning.

**Acceptance criteria**

- **JP-06.1** Given Japanese text and an installed EDICT, when the JParser pane updates, then the
  text is split into words using the dictionary and de-inflection; longer matches are preferred, and
  words with a dictionary entry are highlighted as "translated" (JP-08). *(Verify: unit)*
- **JP-06.2** Given ENAMDICT is installed and a part of the text matches a name entry better than
  any EDICT entry, when the text is segmented, then that part is a name, and its tooltip shows the
  name type (surname, given name, place, …). *(Verify: unit)*
- **JP-06.3** Given the option "use MeCab", when MeCab is available, then MeCab word boundaries are
  used as hints; when MeCab is not available, then the option is greyed out with "needs a MeCab
  dictionary". *(Verify: integration)*
- **JP-06.4** Given the Phase-0 JParser corpus and the same EDICT file as the legacy capture, when
  segmentation and chosen entries are compared, then every line matches the legacy output or has a
  reviewed entry in the divergence list (expected where the new conjugation rules differ).
  *(Verify: parity)*
- **JP-06.5** Given text up to 500 characters, when JParser runs, then the result is shown within
  NFR-03.2. *(Verify: integration)*

### JP-07 JParser display options

**Priority:** Should · **Milestone:** M4 · **Decisions:** D12, D16 · **Legacy:** features.md §2 (JParser config dialog); ui-reference.md §7 "JParser Configuration" · **Depends on:** JP-06, CFG-06

As a user, I want the same JParser options as in the old app, so that the pane looks the way I am
used to.

**Acceptance criteria**

- **JP-07.1** Given the JParser settings page, when it is shown, then it offers: show conjugations,
  Japanese on its own line, match single kanji, match single hiragana, use MeCab, hide cross
  references, hide usage notes, hide part of speech, number of definition lines, reformat numbers,
  and no kana brackets. *(Verify: manual)*
- **JP-07.2** Given any option is changed, when the user closes the page or presses **Apply**, then
  the JParser pane and its tooltips update at once, without a restart. *(Verify: integration)*
- **JP-07.3** Given a Phase-0 fixture for each option (captured with that option on and off), when
  the same input is shown with the same option, then the output matches the legacy output or a
  reviewed divergence. *(Verify: parity)*
- **JP-07.4** Given a legacy INI with `[JParser]` values, when it is imported, then the options take
  the imported values (CFG-06.2). *(Verify: parity)*

### JP-08 Furigana display

**Priority:** Must · **Milestone:** M4 · **Decisions:** D11, D12, D20 · **Legacy:** features.md §2 "Furigana display"; ui-reference.md §8 · **Depends on:** UI-14

As a user, I want readings shown above kanji in the style I choose, so that reading is comfortable.

**Acceptance criteria**

- **JP-08.1** Given the furigana mode for the MeCab pane and, separately, the JParser pane
  (hiragana, katakana, romaji, none), when a word with kanji is shown, then its reading appears
  above it in that script (Hepburn romaji), or no reading in mode "none". Kana-only words get no
  furigana. *(Verify: unit)*
- **JP-08.2** Given a word with okurigana (for example 食べる), when it is shown, then the reading
  is placed over the kanji part only (た over 食). This differs from the legacy pane, which centred
  the reading over the whole word, and is listed as a behaviour change (D20). *(Verify: manual)*
- **JP-08.3** Given a reading wider than its word, when the line is laid out, then spacing grows so
  readings never overlap, and lines wrap at word boundaries. *(Verify: manual)*
- **JP-08.4** Given default settings, when the panes are shown in the light theme, then the default
  colours follow the legacy look: furigana background light grey for MeCab and none for JParser,
  particles light lavender-blue, words with a dictionary entry light green; text 13 px and
  furigana 10 px at 100 % scaling. The dark theme has its own defaults, and text on every highlight
  keeps a contrast of at least 4.5:1. *(Verify: manual)*
- **JP-08.5** Given the user changes colours or the two font sizes, or the DPI or font scaling
  changes, when the pane redraws, then the new values apply at once and sizes scale with DPI.
  *(Verify: manual)*
- **JP-08.6** Given the pane has focus, when the user presses Up/Down, PgUp/PgDn, Home/End or uses
  the mouse wheel, then it scrolls; **Copy** copies the plain text without readings.
  *(Verify: manual)*

### JP-09 Dictionary tooltips

**Priority:** Must · **Milestone:** M4 · **Decisions:** D11, D12 · **Legacy:** features.md §2 "Hover tooltips"; ui-reference.md §8 · **Depends on:** JP-03, JP-06

As a user, I want to see a word's dictionary entry by pointing at it, so that I can understand it
without leaving the game.

**Acceptance criteria**

- **JP-09.1** Given the mouse rests on a word for the hover delay (setting, default 350 ms), when
  the delay passes, then a tooltip opens next to the word, without covering it and without taking
  focus; moving off the word closes it unless it is pinned. *(Verify: manual)*
- **JP-09.2** Given a word with dictionary entries, when the tooltip opens, then it shows each
  entry's headword, reading, part of speech (unless hidden, JP-07), numbered definitions, the
  conjugation chain if de-inflected, and the source dictionary; ambiguous words show all candidates
  (for example 行く and 行う for 行った). *(Verify: manual)*
- **JP-09.3** Given default colours, when the tooltip is shown in the light theme, then kanji are
  dark red, kana green, parentheses light blue and conjugations grey, as in the legacy app; the dark
  theme uses readable equivalents; colours and the tooltip font can be changed. *(Verify: manual)*
- **JP-09.4** Given a tooltip taller than the screen space, when it is shown, then it scrolls (the
  legacy tooltip could not), stays fully on the current monitor, and can be pinned so it stays open.
  *(Verify: manual)*
- **JP-09.5** Given keyboard use, when the user moves word by word with the arrow keys in a focused
  furigana pane and presses Enter (or the context-menu key), then the tooltip opens for the selected
  word; Esc closes it; its text can be copied. *(Verify: manual)*

### JP-10 Kana, romaji and hex conversion actions

**Priority:** Must · **Milestone:** M2 · **Decisions:** D12, D4 · **Legacy:** features.md §2 "Ctrl-H / Ctrl-K / Ctrl-R", "SJIS/UTF-16 to hex" · **Depends on:** UI-07, UI-09

As a user, I want to convert text in a pane to hiragana, katakana or romaji with one key, so that I
can read or type Japanese more easily.

**Acceptance criteria**

- **JP-10.1** Given a text pane with focus, when the user presses Ctrl-H, Ctrl-K or Ctrl-R (or uses
  the pane menu), then the selection, or the whole pane text if nothing is selected, is converted to
  hiragana, katakana or romaji. *(Verify: integration)*
- **JP-10.2** Given the Phase-0 kana fixtures (hiragana, katakana, romaji, long vowels, small
  っ/ッ, ん before vowels, mixed with kanji, ASCII and full-width characters), when each conversion
  runs, then the output equals the legacy output, except reviewed divergences. *(M1 for the
  conversion functions, as in migration phase 1; M2 for the pane actions.)* *(Verify: parity)*
- **JP-10.3** Given the editable Original Text pane, when a conversion runs, then it is one undo
  step (Ctrl-Z restores the text) and does not start a translation by itself; in a translation pane
  it changes only the shown text. *(Verify: integration)*
- **JP-10.4** Given the pane actions "Shift-JIS → hex", "UTF-16 → hex", "hex → Shift-JIS" and
  "hex → UTF-16", when they run on a selection, then the result matches the legacy fixtures; a
  character with no Shift-JIS form, or invalid hex, is reported with its position instead of being
  replaced silently. *(Verify: parity)*
- **JP-10.5** Given the shortcuts are rebound in settings, when the new keys are pressed, then the
  same conversions run. *(Verify: integration)*

### JP-11 Automatic kana conversions of incoming text *(Withdrawn)*

**Priority:** *Withdrawn* (was Should) · **Milestone:** M2 · **Decisions:** D12, D4 · **Legacy:** features.md §2 "Auto half-width to full-width katakana", "Auto romaji to hiragana when no Japanese is present" · **Depends on:** IN-11

*Withdrawn:* the same feature is specified as IN-11 in the text input spec (stage 2 of PIP-01.1).
It is kept there only.

**Acceptance criteria**

- **JP-11.1** *Withdrawn*, see IN-11.1.
- **JP-11.2** *Withdrawn*, see IN-11.3 and IN-11.4.
- **JP-11.3** *Withdrawn*, see IN-11.5 (both options are off by default).

## 4. Out of scope

- ATLAS dictionary search (D7).
- WWWJDIC web dictionary pane: replaced by local JParser; a user may write a web connector
  definition (features.md §1).
- Shipping the legacy `Conjugations.txt` or any legacy-authored data (D13). Reading a user's own
  legacy `Conjugations.txt` is optional per formats-and-interfaces.md §7 and not planned.
- The legacy `.bin` dictionary cache: replaced by the app's own cache, not imported (risks §10).
- `lindera` as a user-visible choice: it is only an internal fallback (rust-mapping.md §5).
- Pitch accent, example sentences, JMdict XML and Kanjidic: not in the first version.

## 5. Open questions

- **O11** (bundle or download dictionaries): this spec assumes the recommendation: download on first
  use with the licence shown; anything bundled ships with its licence and attribution (JP-02).
- The EDRDG licence terms (CC BY-SA per the EDRDG statement) and newer UniDic licences are not
  re-verified (risks §1); the catalogue records the licence text checked when each entry is added.

## 6. Traceability

| Feature | Priority | Milestone | Decisions |
|---|---|---|---|
| JP-01 Morphological analysis backend | Must | M4 | D9, D1, D3 |
| JP-02 Dictionary download, selection and licences | Must | M4 | D13, D9, D12 |
| JP-03 MeCab pane | Must | M4 | D9, D11 |
| JP-04 EDICT and ENAMDICT loading | Must | M4 | D12, D13 |
| JP-05 Conjugation rules and de-inflection | Must | M4 | D13, D12 |
| JP-06 JParser segmentation and name detection | Must | M4 | D12, D13, D20 |
| JP-07 JParser display options | Should | M4 | D12, D16 |
| JP-08 Furigana display | Must | M4 | D11, D12, D20 |
| JP-09 Dictionary tooltips | Must | M4 | D11, D12 |
| JP-10 Kana, romaji and hex conversion actions | Must | M2 | D12, D4 |
| JP-11 Automatic kana conversions of incoming text | *Withdrawn* (see IN-11) | M2 | D12, D4 |
