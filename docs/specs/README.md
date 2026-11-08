# Design specs

These specs turn the decisions in [`../product-direction.md`](../product-direction.md) into
**features with acceptance criteria**. Together they are the goals of the project: a milestone is
done when every **Must** criterion assigned to it passes.

`product-direction.md` decides *what* and *why*. The specs say *what counts as done*.
`../analysis/` describes the legacy app and the technical approach. If a spec disagrees with
`product-direction.md`, the decision record wins and the spec is fixed.

## Spec files

| # | File | Area | ID prefix |
|---|---|---|---|
| 01 | [01-text-input.md](01-text-input.md) | Clipboard, typed text, contexts and filters | `IN` |
| 02 | [02-local-api.md](02-local-api.md) | Local input/output API | `API` |
| 03 | [03-translation-pipeline.md](03-translation-pipeline.md) | Pipeline, substitutions, router, cache, history | `PIP` |
| 04 | [04-web-connector.md](04-web-connector.md) | Declarative web services, presets, custom services | `WEB` |
| 05 | [05-llm-translation.md](05-llm-translation.md) | LLM translator plugin | `LLM` |
| 06 | [06-japanese-tools.md](06-japanese-tools.md) | MeCab, JParser/EDICT, furigana, kana conversion | `JP` |
| 07 | [07-user-interface.md](07-user-interface.md) | Main window, panes, layouts, settings UI, UX | `UI` |
| 08 | [08-settings-and-migration.md](08-settings-and-migration.md) | TOML settings, secrets, legacy import | `CFG` |
| 09 | [09-plugin-system.md](09-plugin-system.md) | Out-of-process plugin protocol | `PLG` |
| 10 | [10-hook-plugin.md](10-hook-plugin.md) | Text hook plugin | `HOOK` |
| 11 | [11-quality-and-release.md](11-quality-and-release.md) | Builds, performance, security, accessibility, licensing, release | `NFR` |

## Summary

Features per milestone, counted by the feature's own milestone and priority (some features have
single criteria in a later milestone; their header says so). Withdrawn features are not counted.

**M0 Scaffolding:** 4 Must.

| Priority | Count | Features |
|---|---|---|
| Must | 4 | NFR-01, NFR-02, NFR-09, NFR-12 |

**M1 Core:** 32 Must.

| Priority | Count | Features |
|---|---|---|
| Must | 32 | IN-01, IN-02, IN-05, IN-06, IN-07, IN-08, IN-09, IN-10, PIP-01, PIP-02, PIP-03, PIP-04, PIP-05, PIP-07, PIP-08, PIP-09, PIP-10, PIP-11, WEB-01, WEB-02, WEB-03, WEB-04, WEB-05, WEB-06, CFG-01, CFG-02, CFG-03, CFG-04, CFG-05, NFR-03, NFR-04, NFR-05 |

**M2 GUI:** 29 Must, 9 Should, 1 Could (JP-11 withdrawn).

| Priority | Count | Features |
|---|---|---|
| Must | 29 | IN-03, WEB-07, WEB-08, WEB-09, WEB-10, WEB-11, WEB-12, JP-10, UI-01, UI-02, UI-03, UI-04, UI-05, UI-06, UI-07, UI-08, UI-09, UI-10, UI-13, UI-14, UI-15, UI-16, CFG-06, CFG-07, CFG-08, CFG-10, NFR-06, NFR-07, NFR-13 |
| Should | 9 | IN-04, IN-11, IN-12, PIP-12, UI-11, UI-12, CFG-09, CFG-11, NFR-08 |
| Could | 1 | CFG-12 |

**M3 LLM:** 10 Must, 2 Should.

| Priority | Count | Features |
|---|---|---|
| Must | 10 | LLM-01, LLM-02, LLM-03, LLM-04, LLM-06, LLM-07, LLM-08, LLM-09, LLM-10, LLM-11 |
| Should | 2 | PIP-06, LLM-05 |

**M4 Japanese tools:** 8 Must, 1 Should.

| Priority | Count | Features |
|---|---|---|
| Must | 8 | JP-01, JP-02, JP-03, JP-04, JP-05, JP-06, JP-08, JP-09 |
| Should | 1 | JP-07 |

**M5 Local API:** 9 Must, 1 Should.

| Priority | Count | Features |
|---|---|---|
| Must | 9 | API-01, API-02, API-03, API-04, API-05, API-06, API-07, API-08, API-09 |
| Should | 1 | API-10 |

**M6 Plugins:** 11 Must, 1 Should (LLM-12 withdrawn, now PLG-11).

| Priority | Count | Features |
|---|---|---|
| Must | 11 | PLG-01, PLG-02, PLG-03, PLG-04, PLG-05, PLG-06, PLG-08, PLG-09, PLG-10, PLG-11, PLG-12 |
| Should | 1 | PLG-07 |

**M7 Hook plugin:** no features or criteria yet. [10-hook-plugin.md](10-hook-plugin.md) is a
placeholder, so M7 cannot be checked against a definition of done until it is written.

**M8 Release:** 2 Must.

| Priority | Count | Features |
|---|---|---|
| Must | 2 | NFR-10, NFR-11 |

## Behaviour changes (D20)

Criteria that intentionally differ from the legacy app. Parity tests mark these as intended
divergences (NFR-12.3), and the list goes into the user docs and release notes (NFR-10.3).

| ID | Change |
|---|---|
| IN-02.1 | A modern clipboard listener replaces the viewer chain; content that asks monitors to skip it (password managers) is not captured. |
| IN-08.6 | Filter fixtures may diverge where a legacy filter result is a bug; each divergence is listed. |
| IN-09.1 | Line-break mode "remove" puts one space where a removed break sat next to non-Japanese text; between two Japanese characters nothing is inserted. |
| PIP-01.1 | The history entry is written after substitutions and transforms, not before, and keeps both the original and the processed text (stages 2–5 keep the legacy order). |
| PIP-02.3 | Substitution profiles apply by input context, not by which processes are running. |
| PIP-07.2 | An automatic entry of exactly the cap length is sent; the legacy app skipped it. |
| PIP-11.4 | New text no longer moves the view away from an older history entry being read; a "N newer entries" notice with "Go to newest" appears instead. |
| PIP-12.4 | History export replaces the legacy translation log file. |
| WEB-07.6 | The DeepL extra-space quirk is not reproduced (DeepL API preset). |
| WEB-09.1 | Keyless Google and Bing are disabled custom-service examples, not built-in panes (D21). |
| CFG-01.3 | Settings are found the same way whatever the working directory (also NFR-13.4). |
| CFG-03.1 | Settings are saved atomically in one file instead of key by key. |
| CFG-06.3 | Legacy Google, Bing and DeepL panes map to the official API presets, which need a key. |
| CFG-07.5 | Layout lines longer than 6,000 characters are read completely (no truncating buffer). |
| CFG-12.4 | Missing fields in a legacy context blob get defaults (the legacy partial reset is not kept). |
| UI-02.5 | "Launch Text Hooker" and Ctrl-L are gone from the core; the hook plugin may add them (D6). |
| UI-06.3 | With the frame hidden, every menu command stays reachable from the context menu and palette. |
| JP-06.4 | JParser results can differ where the new, self-authored conjugation rules differ (reviewed list). |
| JP-08.2 | Furigana sit over the kanji part of a word only (た over 食 in 食べる), not centred over the whole word. |
| JP-09.4 | Dictionary tooltips scroll, stay on the current monitor and can be pinned. |
| JP-10.4 | A character with no Shift-JIS form, or invalid hex, is reported with its position instead of being replaced silently. |

## Milestones

Milestones match the phases in [`../analysis/migration-plan.md`](../analysis/migration-plan.md).

| Milestone | Goal |
|---|---|
| **M0** Scaffolding | Workspace, CI on both targets, license gate, black-box fixtures from the legacy app |
| **M1** Core | Copying text produces a translation from one official web preset |
| **M2** GUI | Daily use: multi-pane UI, settings, legacy import, all official web presets, custom services |
| **M3** LLM | Streaming LLM translations next to the web panes |
| **M4** Japanese tools | MeCab and JParser panes with furigana and hover definitions |
| **M5** Local API | External tools can send text and receive translations |
| **M6** Plugins | Plugin protocol v1; LLM translator runs out of process |
| **M7** Hook plugin | Hook plugin for 32-bit and 64-bit games |
| **M8** Release | Packaged 32-bit and 64-bit releases |

M1 has no GUI; the GUI arrives in M2. Where an M1 criterion says that a pane, the status line or
a settings page shows something, the developer console harness or the log shows it in M1, and the
GUI part is checked again in M2 (see IN-01.1, NFR-03.1, WEB-06.1).

## Format

Every spec file has the same sections:

1. **Purpose and scope**: what the area covers, and what it doesn't.
2. **Goals**: 3–6 outcomes the area must achieve.
3. **Features**: each with acceptance criteria (format below).
4. **Out of scope**: things deliberately not built (with the decision that cut them).
5. **Open questions**: open items (`O#` in `product-direction.md`) that affect this area.
6. **Traceability**: table of feature, priority, milestone, decisions.

Each feature looks like this:

```markdown
### IN-01 Clipboard capture

**Priority:** Must · **Milestone:** M1 · **Decisions:** D5 · **Legacy:** features.md §3 "Clipboard monitoring" · **Depends on:** PIP-01

As a user, I want text I copy to be translated automatically, so that I can read a game without
switching windows.

**Acceptance criteria**

- **IN-01.1** Given clipboard capture is on, when another program copies Japanese text, then the
  text appears in the Original Text pane within the budget in NFR-03.1. *(Verify: integration)*
- **IN-01.2** ...
```

Rules for features and criteria:

- **IDs:** a feature is `<PREFIX>-<nn>` and a criterion is `<PREFIX>-<nn>.<m>`. IDs are never reused
  or renumbered; removed items are marked *Withdrawn*.
- **Priority:** **Must** (required for the milestone), **Should** (expected, can slip one
  milestone), **Could** (nice to have).
- **Criteria are testable:** Given / When / Then, with concrete numbers where it matters, and a
  *Verify* method: `unit`, `integration`, `contract` (recorded responses or mock server), `parity`
  (black-box comparison with the legacy app), `security`, `CI`, or `manual` (a written walkthrough).
- **Legacy references** name the behaviour (from `../analysis/features.md` or `ui-reference.md`),
  never legacy code to translate (D13).
- **Open questions:** where a criterion depends on an open item, it uses the recommended answer and
  says so, e.g. *(assumes O10 recommendation)*.
- **Shared numbers** (performance, limits, timeouts) are defined once in `11-quality-and-release.md`
  and referenced by ID, e.g. "within the budget in NFR-03.1".

## Definition of done

A milestone is done when:

1. every **Must** criterion assigned to it passes with its stated verification method,
2. CI is green on `i686-pc-windows-msvc` and `x86_64-pc-windows-msvc` (D1), including `cargo deny` (D3),
3. user-facing behaviour changes compared with the legacy app are listed (D20).
