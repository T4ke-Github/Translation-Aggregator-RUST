# Legacy fixtures

Black-box input and output captured from the running legacy Translation Aggregator. Parity tests
compare the rewrite against them for every behaviour that is kept (NFR-12.3,
`docs/analysis/migration-plan.md` Phase 0).

Rules:

- **Inputs are ours.** Write the input texts, sample `UserDict.txt` entries and INI layouts
  yourself (or take them from public-domain text). Never copy data files from the legacy codebase.
- **Outputs are observations.** Record what the legacy app shows or writes for those inputs.
- **Test data only.** Nothing in this folder is shipped with the app.
- **Say how it was captured.** Each set has a `capture.md` (see below), so a fixture can be
  re-captured or questioned later.
- A legacy result that is a bug is still recorded as observed. The parity test marks the intended
  difference (D20) and links it to "Behaviour changes" in `docs/specs/README.md`.

## Sets to capture

| Folder | Behaviour | Used by | Milestone |
|---|---|---|---|
| `substitutions/` | Input text → text after substitutions, for a sample `UserDict.txt` with a global and a per-game profile, longest-match cases and the `\n` `\t` `\\` escapes | PIP-02, PIP-03 | M1 |
| `context-filters/` | Input lines → what reaches the Original Text pane, for each Context Manager option: repeat-character and phrase filters, text-loop detection, merge/flush, Japanese-only, line-break modes ("keep some first/last" needs special care: IN-09) | IN-06 to IN-10 | M1 |
| `kana/` | Text → result of Ctrl-H, Ctrl-K, Ctrl-R (hiragana, katakana, romaji) and the automatic half-width → full-width and romaji → hiragana conversions | JP-10, IN-11 | M2 |
| `ini-layouts/` | Legacy INI files with `[Layout N]` lines → screenshots and the observed frames, pane order, columns and splitter positions | CFG-07 | M2 |
| `userdict-import/` | Sample `UserDict.txt` (UTF-16LE) → substitution results after import | CFG-06 | M2 |
| `jparser/` | A corpus of sentences → JParser segmentation and chosen entries, with a named EDICT version the capturer downloaded | JP-05, JP-06 | M4 |
| `mecab/` | The same corpus → MeCab pane readings and segmentation, with a named dictionary | JP-01, JP-03 | M4 |
| `hook-codes/` | Hook codes → accepted or rejected by "Check codes"; captured text from a small test program we write | HOOK spec (not written yet) | M7 |

Capture the M1 and M2 sets first; the legacy app may stop working on newer Windows versions.

## Format

```
fixtures/legacy/<set>/
  capture.md        how the set was captured (template below)
  cases.jsonl       one case per line
  inputs/           our input files (UserDict.txt, INI files, corpus), if any
  outputs/          observed output files (screenshots, logs), if any
```

One case per line in `cases.jsonl`:

```json
{"id": "sub-001", "input": "…", "settings": {"profile": "*"}, "output": "…", "note": "longest match wins"}
```

`settings` holds whatever the case needs (profile, filter options, line-break mode). Keep `id`s
stable: tests refer to them.

### `capture.md` template

```markdown
# <set> capture

- Legacy build: <version or commit>, <how it was built or where it came from>
- Captured: <date> by <name>
- System: Windows <version>, <language/locale>, <screen scale>
- Extra data: <EDICT / MeCab dictionary name and version, if any>

## Steps
1. …

## Known quirks seen
- <case id>: <what looks like a legacy bug>
```

## Status

No set has been captured yet. Capturing needs a working legacy build; this repository contains
none (`Old-Codebase/` is source only).
