# Rust rewrite project plan

This plan turns the wiki roadmap and [migration plan](analysis/migration-plan.md) into work packages. The current [spec index](specs/README.md) and [product decisions](product-direction.md) govern scope when older wiki text differs. This is a work plan, not a claim that a milestone is complete.

## Working rules

Each milestone uses one orchestrator and two subagents. The orchestrator owns the spec, task split, merge, and exit gate. Agent A owns one bounded code area and its tests; Agent B owns another. They work in separate branches or worktrees and avoid edits to the same files. The orchestrator resolves shared API changes before either agent starts.

For each feature, the orchestrator creates a small work item with criterion IDs, inputs, outputs, limits, failure cases, test type, and files owned. First, the agents write failing tests for the named criteria and record the failure. Next, they write the least code that passes. Then they clean up the code and rerun both local and linked tests. A passing test written after the feature does not count as a TDD cycle.

Use unit, integration, provider contract, legacy parity, security, and manual tests as the criterion requires. Keep CI free of live provider calls. Live tests are opt-in and use local keys only. A reviewer who reads legacy C++ must not implement that feature; use behavior notes and black-box fixtures instead. Follow [CONTRIBUTING.md](../CONTRIBUTING.md) for the clean-room rule.

At every handoff, each agent supplies criterion IDs, the spec change, a failing-test record, the passing tests, and any open risk. The orchestrator runs the relevant CI checks on both Windows bitness targets, checks `cargo deny`, updates the [spec tracker](../tools/spec-tracker/README.md), and logs intended legacy differences. The exit gate is every assigned Must criterion passing by its stated method. Should and Could work stays separate and never delays a Must gate. The commands and CI scope live in [CI.md](../.github/CI.md).

## M0: Scaffolding

- Orchestrator: Set the crate boundaries, test layout, CI jobs, and clean-room review rule. Track NFR-01, NFR-02, NFR-09, and NFR-12 against their criteria.
- Agent A: Add failing workspace and target checks, then build the shared crates and both Windows targets. Keep the Rust minimum at 1.90.
- Agent B: Add failing CI and licence checks, then build test helpers, fixture rules, and the licence scan. Exit when both target builds and all M0 Must checks pass.

## M1: Core

- Orchestrator: Freeze the text-event and provider contracts before splitting work. Track IN-01, IN-02, IN-05 to IN-10, PIP-01 to PIP-05, PIP-07 to PIP-11, WEB-01 to WEB-06, CFG-01 to CFG-05, and the M1 parts of NFR-03 to NFR-05.
- Agent A: Own clipboard input, context rules, buffering, filters, settings, and secret storage. Test privacy, corruption recovery, and input edge cases before code.
- Agent B: Own the translation pipeline, provider file format, LibreTranslate preset, cache, limits, and history store. Test fan-out, timeouts, cancellation, redaction, and recorded responses before code. Exit with a headless end-to-end text-to-result test.

## M2: GUI and migration

- Orchestrator: Finish UI-01 design review before GUI screen work. Lock the UI-to-core contract and legacy import mapping. Track the M2 Must list in the [spec index](specs/README.md), plus deferred GUI parts of PIP-11 and NFR-03 to NFR-05.
- Agent A: Own windows, panes, layouts, settings screens, shortcuts, first run, accessibility, and UI state tests. Add manual walkthroughs for visual and keyboard criteria.
- Agent B: Own typed input, web provider presets and test view, Japanese conversion actions, and legacy INI, layout, and user-dictionary import. Test import reports, provider failures, and recorded fixtures first. Exit with a GUI walkthrough and passing M2 Must criteria on both targets.

## M3: LLM translation

- Orchestrator: Set the cost, privacy, and auto-translate rules before remote requests. Track LLM-01 to LLM-04 and LLM-06 to LLM-11; treat PIP-06 and LLM-05 as Should work.
- Agent A: Own OpenAI-compatible and Anthropic backends, prompts, dialogue context, and contract fixtures. Test request shape, parse errors, and redaction first.
- Agent B: Own streaming, cancellation, cleanup, usage display, budgets, and failure handling. Test partial output, timeout, and spend limits first. Exit on offline contract and security tests; keep live calls opt-in.

## M4: Japanese tools

- Orchestrator: Approve data sources, licence terms, and the token-to-pane contract. Track JP-01 to JP-06, JP-08, and JP-09; JP-07 is Should work.
- Agent A: Own morphology, dictionary setup and load, and de-inflection. Start with fixture tests for tokens, names, missing data, and bad files.
- Agent B: Own parser, furigana, tooltips, and the MeCab pane. Start with display and integration tests using small licensed fixtures. Exit after dictionary licence and offline UI checks pass.

## M5: Local API

- Orchestrator: Approve the loopback threat model and versioned API schema first. Track API-01 to API-09; API-10 is Should work.
- Agent A: Own lifecycle, text submission, status, stream output, and API schema. Write route and stream contract tests before handlers.
- Agent B: Own token checks, origin and host checks, payload limits, rate limits, and errors. Write security tests for remote access, flood input, and leaked secrets first. Exit only when the API stays local by default and all M5 Must tests pass.

## M6: Plugins

- Orchestrator: Freeze protocol v1, trust rules, and cross-bitness test scope before implementation. Track PLG-01 to PLG-06 and PLG-08 to PLG-12; PLG-07 is Should work. LLM-12 was withdrawn in favor of PLG-11.
- Agent A: Own manifests, discovery, handshake, framing, and the conformance kit. Build a failing sample-plugin test before the host code.
- Agent B: Own trust prompts, child-process supervision, isolation, translator and transform adapters, and LLM plugin support. Test crash, hang, wrong bitness, and secret handling first. Exit with sample-plugin conformance on both targets.

## M7: Hook plugin, spec gate

- Orchestrator: Do not start hook code yet. The [hook spec](specs/10-hook-plugin.md) is a placeholder with no criteria. Set scope, trust and safety rules, test methods, and a Must list; obtain a spec review before code starts.
- Agent A: Draft observable hook input behavior, supported game and bitness cases, and black-box fixtures. Do not derive Rust code from legacy C++.
- Agent B: Draft plugin protocol use, failure isolation, consent, and manual game tests. Once the spec is approved, split code by ownership and run the same red-green-cleanup cycle. Exit only after the new Must criteria pass.

## M8: Release

- Orchestrator: Close NFR-10 and NFR-11 after every earlier Must gate, including the new M7 gate if hooks ship. Approve the licence bundle and release notes.
- Agent A: Own signed or packaged 32-bit and 64-bit builds, clean install and upgrade tests, and release automation. Test both artifacts on supported Windows systems.
- Agent B: Own user, web, API, and plugin guides, plus milestone walkthroughs and a final spec audit. Check every public feature against a passing test and guide. Exit when release artifacts, docs, CI, and licence notices pass.

## First work session

The orchestrator first compares the current repo and spec tracker with the M0 and M1 gates. It marks what is done, what lacks proof, and what needs a new test. It then issues only the next two non-overlapping work items, with criterion IDs and failing tests, to Agents A and B.
