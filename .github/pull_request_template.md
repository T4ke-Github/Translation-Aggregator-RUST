## What this changes

<!-- One topic. Name the spec features and criteria, e.g. "Implements IN-01.1 to IN-01.4". -->

## Clean-room checklist (D13, NFR-09.5)

- [ ] I wrote this from the specs, `docs/analysis/`, observed behaviour or black-box fixtures, not from legacy source code.
- [ ] No identifiers copied from the legacy code, other than public format names (INI keys, file names, hook-code syntax).
- [ ] No tables, constants or data lists copied from the legacy code.
- [ ] No comments or documentation text copied from the legacy code or its docs.
- [ ] No legacy data files, images or icons added (the `No legacy files` CI job checks hashes).
- [ ] New dependencies, if any, are permissively licensed (`cargo deny` passes).

## Tests

- [ ] Each criterion is tested with the method its spec names (unit, integration, contract, parity, security, CI or manual).
- [ ] No test calls a real provider without `TA_LIVE_TESTS=1`.
- [ ] Intended differences from the legacy app are listed under "Behaviour changes" in `docs/specs/README.md`.
