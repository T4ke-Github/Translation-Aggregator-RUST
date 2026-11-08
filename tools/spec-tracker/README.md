# Spec tracker

Builds the spec tracker page from the design specs in `docs/specs/`. The page lists every feature
and acceptance criterion by milestone and area, and lets people mark criteria Passed or Failed.

The published tracker: https://claude.ai/artifact/EUgT2xkofmE4ctjCu81Fc9 (private until shared).

## How it works

- `extract_specs.py` reads `docs/specs/[0-9][0-9]-*.md` and writes `areas.json`, `features.json` and
  `criteria.json`. It relies on the spec format in `docs/specs/README.md`: `### PREFIX-nn Title`
  headings, the `**Priority:** … · **Milestone:** …` line, and criteria as
  `- **PREFIX-nn.m** … *(Verify: method)*`. Items marked *Withdrawn* are kept in the JSON with
  `withdrawn: true` and left out of the page.
- `build_tracker.py` puts that data into `tracker.template.html` and writes the page.
- Statuses (Passed, Failed, notes, who and when) are **not** in the page. They live in the
  published page's own database, keyed by criterion ID, so rebuilding and republishing the page
  keeps them. Because IDs are never reused (spec rule), a status always stays with its criterion.

## Rebuild after a spec change

From the repository root:

```sh
python -I tools/spec-tracker/extract_specs.py docs/specs tools/spec-tracker/out
python -I tools/spec-tracker/build_tracker.py tools/spec-tracker tools/spec-tracker/out/tracker.html
```

The first command prints a summary (features, criteria, priorities, milestones) and lists any
feature without a priority or milestone, any feature without criteria, and any criterion without a
verification method. Fix those in the spec before publishing.

Then publish `tools/spec-tracker/out/tracker.html` to the same artifact URL (for example by asking
Claude to republish the spec tracker). `out/` is not committed.

Requires Python 3.11 or newer; no packages.
