# 08 Settings and migration (`CFG`)

## 1. Purpose and scope

This area covers where and how the app keeps its settings, how secrets are kept out of files, and
how users of the legacy app bring their data over:

- the TOML settings file: location, portable mode, schema version, atomic save, recovery from
  corruption,
- secrets (API keys, the local API token) in the OS credential store,
- the one-time import of the legacy INI, including `[Layout N]`,
- import and export of `UserDict.txt` (UTF-16LE),
- the import report,
- profiles per game or context.

Not covered here: what each setting does (see the area that owns it), the settings UI pages (see the
UI spec), the web connector definition format (see the WEB spec), and the hook plugin's launch
profiles (see the HOOK spec).

## 2. Goals

1. Settings survive crashes, power loss and bad edits; the app always starts (D16, D20).
2. Secrets never land in a file, a log or an export (NFR-05).
3. A legacy user gets their languages, fonts, pane choices, layouts and substitutions on first
   start without editing anything (D16).
4. Every imported, changed or dropped legacy item is reported, with the reason (D20).
5. Settings are readable TOML, versioned, and stay compatible across releases.

## 3. Features

### CFG-01 Settings location and portable mode

**Priority:** Must · **Milestone:** M1 · **Decisions:** D16, D20 · **Legacy:** formats-and-interfaces.md §1 (INI in the working directory); architecture.md §6 "CWD-relative paths" · **Depends on:** —

As a user, I want the app to find the same settings however I start it, and to run from a folder on
a USB stick if I want, so that my setup doesn't depend on the shortcut I used.

**Acceptance criteria**

- **CFG-01.1** Given a normal install, when the app starts, then settings are read from
  `settings.toml` in `%APPDATA%\Translation Aggregator\`, and data the app can rebuild (dictionary
  caches) goes to `%LOCALAPPDATA%\Translation Aggregator\`. *(Verify: integration)*
- **CFG-01.2** Given an empty file named `portable` next to the executable, when the app starts, then
  all settings and data live in a `data` folder next to the executable. *(Verify: integration)*
- **CFG-01.3** Given the app is started from a shortcut with a different working directory, when it
  loads settings, then it uses the same file as a normal start (legacy bug fixed, D20).
  *(Verify: integration)*
- **CFG-01.4** Given the settings folder is not writable, when the app starts, then it runs with the
  settings it could read, shows "Settings can't be saved: <path>", and never writes to another
  place silently. *(Verify: integration)*
- **CFG-01.5** Given the 32-bit and 64-bit builds are installed for the same user, when either one
  starts, then both use the same settings file. *(Verify: CI)*

### CFG-02 Settings schema versioning

**Priority:** Must · **Milestone:** M1 · **Decisions:** D16 · **Legacy:** formats-and-interfaces.md §2 (versioned `ContextSettings` blobs) · **Depends on:** CFG-01

As a user, I want settings from an older version to be upgraded and settings from a newer version to
be left intact, so that I never lose my setup when I change versions.

**Acceptance criteria**

- **CFG-02.1** Given any settings file the app writes, when it is opened, then its first key is
  `schema_version` with an integer. *(Verify: unit)*
- **CFG-02.2** Given a file with an older schema version, when the app loads it, then it is migrated
  step by step to the current version, and before the first save the old file is copied to
  `settings.v<old>.bak`. Each migration step has an old → new fixture test. *(Verify: unit)*
- **CFG-02.3** Given a file with a newer schema version than the app supports, when the app loads it,
  then it uses the keys it knows, shows a warning, and does not save over the file unless the user
  agrees (then it first copies it to `settings.v<new>.bak`). *(Verify: integration)*
- **CFG-02.4** Given a file with keys the app doesn't know, when it saves, then those keys are kept,
  and the log lists them once. *(Verify: unit)*
- **CFG-02.5** Given a file without `schema_version`, when it is loaded, then it is treated as version
  1 with a warning. *(Verify: unit)*

### CFG-03 Atomic save

**Priority:** Must · **Milestone:** M1 · **Decisions:** D16, D20 · **Legacy:** none (the legacy app wrote INI keys one by one) · **Depends on:** CFG-01

As a user, I want a crash during saving to never leave a half-written settings file, so that the app
always starts with a complete set of settings.

**Acceptance criteria**

- **CFG-03.1** Given a change is saved, when the app writes it, then it writes a temporary file in the
  same folder, flushes it to disk, and replaces `settings.toml` in one rename. Killing the process at
  random points during 1,000 saves always leaves either the complete old or the complete new file.
  *(Verify: integration)*
- **CFG-03.2** Given a save replaces the file, when it does, then the previous good file is kept as
  `settings.toml.bak`. *(Verify: integration)*
- **CFG-03.3** Given the user changes several settings within 1 s, when they are saved, then they are
  written together in one save, off the UI thread (NFR-03.3). *(Verify: unit)*
- **CFG-03.4** Given a save is pending, when the app exits, then the save completes before exit
  (waiting at most 2 s). *(Verify: integration)*
- **CFG-03.5** Given the user added comments to the file by hand, when the app saves, then comments on
  keys that did not change are kept. *(Verify: unit)*
- **CFG-03.6** Given the app is already running, when a second instance starts, then it hands its
  command line to the first instance and exits, so only one process writes the settings.
  *(Verify: integration)*

### CFG-04 Corruption recovery

**Priority:** Must · **Milestone:** M1 · **Decisions:** D16, D20 · **Legacy:** none (new) · **Depends on:** CFG-03

As a user, I want the app to start even if the settings file is broken, and to keep the broken file
so that nothing is lost.

**Acceptance criteria**

- **CFG-04.1** Given `settings.toml` has a syntax error, when the app starts, then it loads
  `settings.toml.bak` if that is valid, shows a notice with the file path, line and column, and
  renames the broken file to `settings.corrupt-<timestamp>.toml`. *(Verify: integration)*
- **CFG-04.2** Given both the file and the backup are broken, when the app starts, then it starts with
  defaults, shows a notice, and deletes neither file. *(Verify: integration)*
- **CFG-04.3** Given one value has the wrong type or is out of range, when the file loads, then only
  that key is reset to its default, the rest is kept, and the notice lists the reset keys.
  *(Verify: unit)*
- **CFG-04.4** Given a 0-byte file, a file with NUL bytes, or a file that is not valid UTF-8, when the
  app starts, then it is handled as in CFG-04.1. *(Verify: unit)*
- **CFG-04.5** Given any recovery, when it runs, then secrets in the credential store are not changed
  (CFG-05). *(Verify: integration)*

### CFG-05 Secrets in the OS credential store

**Priority:** Must · **Milestone:** M1 · **Decisions:** D8, D10, D18 · **Legacy:** none (the legacy app had no keys) · **Depends on:** —

As a user, I want my API keys and the API token kept in Windows Credential Manager, so that they
are not in files I might share or back up.

**Acceptance criteria**

- **CFG-05.1** Given the user enters an API key, when it is saved, then it is stored in Windows
  Credential Manager under a name with the prefix `Translation Aggregator/`, and the settings file
  holds only the secret's name. A test that searches the settings, logs, history export, import
  report and crash report for the key finds nothing (NFR-05.1). *(Verify: security)*
- **CFG-05.2** Given a saved secret, when it is shown in settings or in a provider's Test view, then
  it is masked; the user can replace or delete it but not read it back in plain text (NFR-05.2).
  *(Verify: manual)*
- **CFG-05.3** Given a provider refers to a secret that doesn't exist, when a request would be sent,
  then nothing is sent and the pane shows "Needs key" (WEB-12.1) with a link to the setting.
  *(Verify: integration)*
- **CFG-05.4** Given the credential store can't be written, when the user saves a key, then an error
  says so, and the key is not stored anywhere else. *(Verify: integration)*
- **CFG-05.5** Given the user exports settings or moves a portable folder to another PC, when the
  settings are used there, then no secret comes along, and the affected providers show
  "Needs key"; the user docs say so. *(Verify: integration)*
- **CFG-05.6** Given the user deletes a provider, when it is removed, then the app asks whether to
  delete its secret as well. *(Verify: manual)*

### CFG-06 Legacy INI import

**Priority:** Must · **Milestone:** M2 · **Decisions:** D16, D7, D6, D20 · **Legacy:** formats-and-interfaces.md §1 "Main INI" · **Depends on:** CFG-02, CFG-10

As a legacy user, I want my old settings imported on first start, so that the new app feels like
mine from the beginning.

**Acceptance criteria**

- **CFG-06.1** Given no settings file exists, when the app starts for the first time, then it offers
  to import legacy settings, lets the user pick the legacy folder or `TranslationAggregator.exe`,
  and finds `TranslationAggregator.ini` (or the old misspelled name) there. *(Verify: manual)*
- **CFG-06.2** Given a legacy INI from the Phase-0 fixtures, when it is imported, then languages
  (English names to language codes), main and tooltip fonts (`Font Height` / 15 = points), colours
  (`COLORREF` 0xBBGGRR to RGB), the `Flags` bits (substitutions, auto hiragana, context full
  history), `AutoHalfWidthToFull`, `GlobalHotkeys`, `[JParser]`, `[Mecab]` and
  `[Original Text] Trans Highlighted` give the same values. *(Verify: parity)*
- **CFG-06.3** Given per-pane `Auto Clipboard` keys, when they are imported, then Google maps to the
  Google Cloud Translation preset, Bing to Microsoft Translator, DeepL to the DeepL API preset, and
  MeCab and JParser to themselves, with the same auto-translate value; mapped presets stay disabled
  until a key is added. Other panes are dropped and reported. *(Verify: unit)*
- **CFG-06.4** Given `[ATLAS]`, `[General] Port`, `[Injection]` and `[WWWJDIC]`, when they are found,
  then they are not imported and the report gives the reason (D7; hook listener gone; hook plugin,
  D6; only for a user-made connector). *(Verify: unit)*
- **CFG-06.5** Given an INI saved as UTF-16LE or in the system ANSI code page, with non-ASCII values
  stored as hex blobs, when it is imported, then text values are decoded correctly; a malformed value
  is skipped and reported, and the rest is imported. *(Verify: unit)*
- **CFG-06.6** Given an import, when it runs, then the legacy files are not changed; it runs
  automatically only once, and "Import legacy settings…" can run it again after a confirmation that
  mapped settings will be overwritten. *(Verify: integration)*

### CFG-07 Legacy layout import (`[Layout N]`)

**Priority:** Must · **Milestone:** M2 · **Decisions:** D16, D11, D20 · **Legacy:** formats-and-interfaces.md §1 "Layout line format"; features.md §5 "10 bindable layouts" · **Depends on:** CFG-06, UI-05

As a legacy user, I want my saved window layouts to come over, so that my panes are where I am used
to them.

**Acceptance criteria**

- **CFG-07.1** Given a legacy INI with `[Layout 0]` to `[Layout 10]`, when it is imported, then each
  layout has the same frames, pane order, number of columns, always-on-top, frame and toolbar
  visibility, transparency and window position as in the fixture. *(Verify: parity)*
- **CFG-07.2** Given splitter positions stored out of 100,000, when they are imported, then column and
  row sizes match the legacy layout within 1 px at 100 % display scaling. *(Verify: parity)*
- **CFG-07.3** Given a layout that lists cut panes (ATLAS, LEC, scraper panes), when it is imported,
  then those panes are removed, the others keep their order, and a frame left with no panes is
  dropped; the report lists each removal. *(Verify: unit)*
- **CFG-07.4** Given a malformed `Window i` line (wrong number of fields, non-numbers), when it is
  imported, then that layout is skipped and reported, and the other layouts are imported.
  *(Verify: unit)*
- **CFG-07.5** Given a layout line longer than 6,000 characters, when it is imported, then it is read
  completely (the legacy buffer limit is not kept, D20). *(Verify: unit)*
- **CFG-07.6** Given a layout whose window lies outside every current monitor, when it is restored,
  then the window is moved as in UI-05.4; legacy layout 0 becomes the auto layout (UI-05.3) and is
  used as the first session layout. *(Verify: integration)*

### CFG-08 `UserDict.txt` import

**Priority:** Must · **Milestone:** M2 · **Decisions:** D16, D12 · **Legacy:** formats-and-interfaces.md §3 "Substitution list"; features.md §4 "Substitutions" · **Depends on:** PIP-02, CFG-10

As a legacy user, I want my substitution lists imported, so that names and terms keep being
replaced as before.

**Acceptance criteria**

- **CFG-08.1** Given a `UserDict.txt` in UTF-16LE with BOM, LF or CRLF line endings, `Profile:<name>`
  blocks and match/replacement line pairs, when it is imported, then every rule is stored in TOML,
  and the Phase-0 fixture texts give the same substitution results as the legacy app.
  *(Verify: parity)*
- **CFG-08.2** Given the profile `*`, when it is imported, then it becomes the global profile;
  profiles named after a legacy `folder\exe`, when imported, keep the name and get a context pattern
  for that game, which the report asks the user to check. *(Verify: unit)*
- **CFG-08.3** Given a file in UTF-8 (with or without BOM) or UTF-16LE without BOM, when it is
  imported, then the encoding is detected, it is imported, and the report notes the encoding; a file
  in any other encoding is not imported and the report says "unsupported encoding".
  *(Verify: unit)*
- **CFG-08.4** Given a block ends with a match line and no replacement line, or has an empty match
  line, when it is imported, then that pair is skipped and reported, and the other pairs are
  imported. *(Verify: unit)*
- **CFG-08.5** Given the same match appears twice in one profile, when it is imported, then the last
  one wins and the report lists the duplicate. *(Verify: unit)*
- **CFG-08.6** Given rules already exist, when the user imports a file again, then the user chooses
  "replace" or "merge"; the source file is never changed. *(Verify: integration)*

### CFG-09 `UserDict.txt` export

**Priority:** Should · **Milestone:** M2 · **Decisions:** D16 · **Legacy:** formats-and-interfaces.md §3 · **Depends on:** CFG-08

As a user, I want to export my substitutions in the legacy format, so that I can share them with
people who still use the old app.

**Acceptance criteria**

- **CFG-09.1** Given substitution profiles, when they are exported, then the file is UTF-16LE with
  BOM and LF line endings, one `Profile:<name>` block per profile, with `\n`, `\t` and `\\` escaped.
  *(Verify: unit)*
- **CFG-09.2** Given any set of rules, when it is exported and imported again, then the rules are the
  same. *(Verify: unit)*
- **CFG-09.3** Given an exported file, when the legacy app loads it, then it gives the same
  substitution results for the fixture texts. *(Verify: parity)*
- **CFG-09.4** Given a profile with a context pattern the legacy format can't express, when it is
  exported, then the pattern is used as the profile name and the export summary says so.
  *(Verify: unit)*

### CFG-10 Import report

**Priority:** Must · **Milestone:** M2 · **Decisions:** D16, D20 · **Legacy:** none (new) · **Depends on:** —

As a legacy user, I want a report of what was imported and what was left behind, so that I know what
to set up again.

**Acceptance criteria**

- **CFG-10.1** Given any import (INI, layouts, `UserDict.txt`), when it finishes, then a report lists
  each item as imported, mapped (old → new), ignored (with reason and decision) or failed (with
  reason), and is shown to the user. *(Verify: integration)*
- **CFG-10.2** Given a report, when it is shown, then it starts with counts per status, and the counts
  match the listed items. *(Verify: unit)*
- **CFG-10.3** Given a report, when the import finishes, then it is saved as
  `import-report-<date>.txt` in the data folder and can be opened again from settings.
  *(Verify: integration)*
- **CFG-10.4** Given an import fails because a file can't be read, when it stops, then no setting from
  that file is applied, and the report says which file and why. *(Verify: integration)*

### CFG-11 Profiles per game or context

**Priority:** Should · **Milestone:** M2 · **Decisions:** D5, D6, D16 · **Legacy:** formats-and-interfaces.md §2 "Per-game config"; features.md §4 (per-game substitution profiles) · **Depends on:** IN-05, PIP-02

As a user, I want settings that apply only to one game or source, so that switching games switches
filters, substitutions and translators with it.

**Acceptance criteria**

- **CFG-11.1** Given a profile with a name, one or more context patterns (for example
  `api:hook/game.exe/*`) and overrides (context options, substitution profiles, languages, enabled
  translators and their auto-translate), when an entry from a matching context arrives, then the
  overrides apply; entries from other contexts use the defaults. *(Verify: unit)*
- **CFG-11.2** Given two profiles match the same context, when an entry arrives, then the one with the
  longer literal part of its pattern wins; on a tie, the one listed first; the context panel shows
  which profile applies. *(Verify: unit)*
- **CFG-11.3** Given the active profile changes because text arrives from another game, when it
  switches, then the status line shows the profile name. *(Verify: manual)*
- **CFG-11.4** Given a profile, when the user exports it, then it is written as a TOML file without
  secrets, and importing it on another PC recreates it. *(Verify: integration)*

### CFG-12 Legacy per-game context settings import

**Priority:** Could · **Milestone:** M2 · **Decisions:** D16, D6, D20 · **Legacy:** formats-and-interfaces.md §2 "`[Contexts]` blobs" · **Depends on:** CFG-11

As a legacy user, I want my per-game context options imported, so that tuned filters for a game are
not lost.

**Acceptance criteria**

- **CFG-12.1** Given a `Game Configs\*.ini` with `[Contexts]` blobs of version 3, when it is imported,
  then a profile named after the game is created with those context options, keyed by the legacy
  context name (matched if the hook plugin reports the same name; see the HOOK spec).
  *(Verify: parity)*
- **CFG-12.2** Given a blob of another version or of the wrong size, when it is imported, then that
  context is skipped and reported; the rest is imported. *(Verify: unit)*
- **CFG-12.3** Given `[Launch]`, `[Injection]` and `[Atlas]` sections, when they are found, then the
  core does not import them; the report says `[Launch]`/`[Injection]` belong to the hook plugin
  (D6) and `[Atlas]` is cut (D7). *(Verify: unit)*
- **CFG-12.4** Given a blob where fields are missing, when it is imported, then the missing fields
  get defaults (the legacy partial reset is not reproduced, D20). *(Verify: unit)*

## 4. Out of scope

- Keeping the INI as the live settings format: settings are imported once, then TOML (D16).
- Secrets in any file, including portable mode: never (NFR-05).
- Importing the legacy `.bin` dictionary cache: rebuilt instead (formats-and-interfaces.md §7).
- ATLAS settings and rule sets: cut (D7).
- Hook plugin launch profiles and injection settings: owned by the hook plugin (D6).
- Cloud sync of settings between PCs: not planned.

## 5. Open questions

None of O10–O14 affects this area directly. CFG-06.3 assumes the legacy Google, Bing and DeepL panes
map to the official presets (D17); the keyless examples (D21) are never enabled by an import.

## 6. Traceability

| Feature | Priority | Milestone | Decisions |
|---|---|---|---|
| CFG-01 Settings location and portable mode | Must | M1 | D16, D20 |
| CFG-02 Settings schema versioning | Must | M1 | D16 |
| CFG-03 Atomic save | Must | M1 | D16, D20 |
| CFG-04 Corruption recovery | Must | M1 | D16, D20 |
| CFG-05 Secrets in the OS credential store | Must | M1 | D8, D10, D18 |
| CFG-06 Legacy INI import | Must | M2 | D6, D7, D16, D20 |
| CFG-07 Legacy layout import | Must | M2 | D11, D16, D20 |
| CFG-08 `UserDict.txt` import | Must | M2 | D12, D16 |
| CFG-09 `UserDict.txt` export | Should | M2 | D16 |
| CFG-10 Import report | Must | M2 | D16, D20 |
| CFG-11 Profiles per game or context | Should | M2 | D5, D6, D16 |
| CFG-12 Legacy per-game context settings import | Could | M2 | D6, D16, D20 |
