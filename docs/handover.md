# Project handover

The Rust rewrite is in M1, not at the M1 exit gate. M0 scaffolding and several M1 slices are merged into `main`. CI builds and tests both Windows MSVC widths, but a passing CI run does not prove every acceptance criterion. Use the [project plan](project-plan.md), [spec index](specs/README.md), and [product direction](product-direction.md) as the source of work and decisions.

## Next M1 work

The developer console has an opt-in clipboard path through buffering, filters, substitutions, and routing to LibreTranslate over loopback. The server can come from settings with an optional Windows Credential Manager reference, or from an explicit keyless command-line override. The local mock test starts at the clipboard event boundary and checks that rejected input makes no request. Run the [M1 walkthrough](walkthroughs/M1-core.md) on both Windows builds to verify native clipboard delivery and the live-server exit criterion. Remote and paid auto-translation stays off by default, per the approved O10 decision.

The console reuses complete keyless LibreTranslate results and records successful, cached, failed,
and superseded entries in memory. LibreTranslate runs off the capture thread with one active
request and one waiting entry. Transient failures get up to two retries, and newer eligible input
ends the older request's retry wait. The console supports configurable provider pacing, a cooldown
on HTTP 429, and opt-in history retention. Clipboard settings control idle buffering and filters.
The console can pause without reading clipboard contents and capture with automatic translation
off or without a provider. Manual requests split at sentence boundaries before sending any chunks.

Capture Phase-0 black-box fixtures for parity tests without copying legacy source or data. The [fixture guide](../fixtures/legacy/README.md) lists the missing captures. Test the 500-character and 10,000-rule performance target on the stated Windows reference hardware. Record walkthrough results and mark only criteria proved by their required test method in the spec tracker.

## Later gates

M2 covers the GUI and legacy import. The [UI design draft](ui-design.md) is ready for review and
lists the pending Windows spikes. Screen work requires owner approval under UI-01. M3 adds LLM
backends, M4 adds Japanese tools, M5 adds the local API, and M6 adds plugins. M7 cannot start hook
code until its placeholder spec gains approved criteria and safety rules. M8 needs release builds,
licences, docs, and final checks for both Windows widths.

## Work rules and repo state

Use one orchestrator and two subagents for each milestone, with exclusive file areas. Write a criterion-level failing test before code, then make it pass and rerun the linked checks. Keep the clean-room split in [CONTRIBUTING.md](../CONTRIBUTING.md); never implement a feature from legacy C++ source. A helper test is not proof that the app calls the helper.

PRs 5 and 6 updated the TOML and Windows dependency versions and are merged. PR 5 needed a parser-location test fix before both Windows widths passed. At this handover there are no open PRs. Check `git status`, fetch `main`, and read the latest CI run before starting the next branch. This note records scope, not a claim that M1 or later gates are complete.
