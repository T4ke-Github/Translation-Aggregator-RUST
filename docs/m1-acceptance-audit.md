# M1 acceptance audit

Baseline commit `68458df`, which merges the Windows checkpoint from PR 13. The spec index
lists 32 M1 Must features. The extractor inherits those feature labels for 182 criterion
rows, including criteria whose text assigns all or part of the work to a later milestone.
The tables below apply those criterion-level qualifications. [Spec index](specs/README.md) ·
[Project plan](project-plan.md) · [PR 13](https://github.com/T4ke-Github/Translation-Aggregator-RUST/pull/13)

The checkpoint records both native Windows builds, loopback and live LibreTranslate,
Credential Manager, and performance on an owner-approved PC. The fixture replay compares 22
observations from the legacy release, with intentional differences documented under D20. Both
parity tests also passed on Linux from an isolated archive of the baseline commit.
Main CI passed Windows build, tests, and Clippy on both MSVC targets, as well as Rust 1.90 and
both dependency checks. Native observations are attributed to the checkpoint's stated build
and were reviewed from the checked-in record. [Windows walkthrough](walkthroughs/M1-core.md) ·
[Legacy captures](../fixtures/legacy/README.md) · [Main CI](https://github.com/T4ke-Github/Translation-Aggregator-RUST/actions/runs/44951202421)

Evidence below is a reconciliation candidate. A row can contain both completed cases and
outstanding portions of the same feature. Keep a criterion open when its evidence covers only
a helper, one branch of the behavior, or a different verification method.

## Clipboard, contexts and filters

Required methods and complete criteria are in the [input spec](specs/01-text-input.md).
The main evidence is the [native walkthrough](walkthroughs/M1-core.md#loopback-smoke-test),
[clipboard gate tests](../crates/ta-ingest/src/clipboard_gate.rs),
[ingest unit tests](../crates/ta-ingest/src/lib.rs),
[console integration tests](../crates/ta-gui/src/clipboard/tests.rs),
and [parity replay](../crates/ta-ingest/tests/legacy_parity.rs).

| Feature | Evidence at the checkpoint | Remaining check or implementation |
| --- | --- | --- |
| IN-01.1–.6 | Native copy-to-console and approved latency for .1. Unit checks cover whitespace (.3) and unpaired UTF-16 (.5). The gate unit test exercises a simulated self-copy. | Native non-text image/file-list case (.2). The current read path supplies no app-owned copy signal. Wire that signal and prove self-copy suppression on Windows (.4). Rerun the complete set under both Windows targets before closing .6. |
| IN-02.1–.5 | Native privacy-marker input was absent from console, history, and requests (.1). Pause/resume with a fresh clipboard change passed on both targets (.2). The gate has unit cases for duplicate, burst, and disabled sequences. | Record actual clipboard access and console responsiveness for a 500 ms busy clipboard (.3), 50 writes in one second (.4), and disabled startup (.5). Prove zero reads for the disabled case. UI thread responsiveness is part of NFR-03.3 in M2. |
| IN-05.1–.6 | Unit checks cover context IDs, templates and validation (.1–.3). Console IDs and configured clipboard options are present, and the Windows run showed `clipboard`. Unit and console tests cover default/configurable scalar limits and rejection output (.6). | Verify restarted options for more than the fixed clipboard context (.4). Record typed and clipboard source display explicitly for .5. The context panel and API/plugin sources come in M2, M5 and M6 respectively. |
| IN-06.1–.6 | Unit checks cover idle/immediate timing, defaults and disable/drop (.1–.3, .6). Legacy replay covers both merge modes (.4–.5), including the recorded merge-on-flush difference. | The merge replay uses the ingest buffer and synthetic contexts. Verify merged entry routing through an app entry point when multiple sources are available, before treating replay as proof of runtime wiring. |
| IN-07.1–.6 | Unit checks cover script detection, preservation and repeat suppression (.1–.2, .4). The romaji-only fixture rejects before transforms (.3). Clipboard cache and manual `translate-all` are exercised separately. | Integrate the same-text cache hit across two input contexts (.5) and make `translate-all` exercise an explicitly suppressed repeat (.6). Automatic romaji conversion itself is M2. |
| IN-08.1–.6 | The 16 context-filter captures and replay cover all named repeat modes, phrase extension, short/long phrase repetition, loop suppression/recovery, and D20 replay (.1–.6). | Keep parity scope tied to the captured inputs. The phrase min/max exclusion cases are unit-checked. Add captured out-of-range phrases if closing the entire .4 by parity. |
| IN-09.1–.5 | Newline unit tests cover remove/keep, CRLF/CR, trimming and blank output (.1–.2, .4–.5). Both keep-some captures are replayed against the documented rewrite behavior (.3). | Preserve the captured mismatch and D20 explanation during tracker reconciliation. |
| IN-10.1–.5 | Console integration covers auto-translate disabled with accepted history (.1). The input gate tests disabling and core unit tests cover per-context UTF-8 byte eviction and isolated clearing (.2, .4–.5). | The console has no auto-copy operation (.3). Add real clipboard write/self-copy integration coverage with IN-01.4. The bounded per-context text history has no console caller. Verify wiring when exposing that context history and verify dropped counts when the context panel ships. |

## Translation, routing and history

Methods and milestone qualifiers are in the [pipeline spec](specs/03-translation-pipeline.md).
See the [recorded routing and timing runs](walkthroughs/M1-core.md#acceptance-evidence),
[router unit tests](../crates/ta-translate/src/lib.rs),
[substitution unit tests](../crates/ta-substitute/src/lib.rs),
[console integration tests](../crates/ta-gui/src/clipboard/tests.rs)
and [history unit tests](../crates/ta-core/src/lib.rs).

| Feature | Evidence at the checkpoint | Remaining check or implementation |
| --- | --- | --- |
| PIP-01.1–.5 | Marker-transform unit tests record order and history insertion (.1) and empty substitution (.4). Console cases show both text forms and send processed text to web providers (.2). Approved Windows timings cover the substitutions portion of .5. | Finish the local-tool and LLM branches of .2 in their milestones. The failed-transform unit case does not verify failure or timeout for an actual plugin (.3, PLG-06 in M6). The GUI-thread portion of .5 depends on NFR-03.3 in M2. |
| PIP-02.1–.6 | Six legacy substitution captures cover longest match, replacement ordering and escapes (.1–.2). Unit tests cover global and scoped precedence, deletions and invalid/disabled rules (.3–.5). Tested portable settings and approved Windows scale results supply the timing and parity evidence for .6. | Preserve the 500-character, 10,000-rule, five-profile conditions when repeating performance verification. |
| PIP-03.1–.6 | The capability and request-policy helpers cover supported pairs and scalar limits (.2–.3). The console's non-streaming results arrive on completion (.5). | Add a registration contract for the M1 web providers (.1), assert unsupported/over-limit diagnostics on the console (.2–.3), and exercise capability updates without restart (.6). UI marking is associated with NFR-05.3 in M2. Streaming and other provider types arrive later (.1, .4–.5). |
| PIP-04.1–.5 | Both native Windows runs and console mock tests cover independent progress of three providers (.1–.2), per-provider metadata in history and console (.3), manual-only providers in Translate All (.4), and the fresh providerless prompt with accepted entry (.5). | The API event stream portion of .3 is M5. Preserve the console and history metadata checks as the new providers and UI are added. |
| PIP-05.1–.5 | O10 defaults have unit coverage (.1). A missing key produces zero requests and disabled providers are excluded (.3). Native and mock runs show that pausing capture still permits Translate All (.4). | .2 is open at this baseline. Record the prescribed manual check for provider name, text leaving the device, potential cost, cancellation, and whether a subsequent enable needs confirmation. The legacy INI import confirmation path (.5) is explicitly M2. |
| PIP-07.1–.5 | Router unit cases exercise exact limits, overrides and Unicode scalars. Console tests cover post-substitution rejection, cap overrides for LibreTranslate, and actual requests at the boundary. Sentence-aware manual splitting has an app test. | Verify the complete automatic rejection text and custom-provider cap settings (.1, .4). Add one integration case that rejects a specific entry automatically, then sends that same stored entry by a manual action (.3). |
| PIP-08.1–.6 | The console tests and native smoke check show a keyless LibreTranslate cache hit without a second request (.1). Cache unit tests cover identity, successful-only inserts, LRU and opt-out (.2–.5). | Record cache-hit timing against NFR-03.2 if treating .1 as complete. The console cache currently covers the keyless preset, and custom definitions have no cache policy wired. Verify stale settings (including reload) and failure paths through the actual sender (.2–.5). `translate-all` bypasses an automatic cache lookup, but an explicit Retry test must show bypass, replacement and a subsequent hit (.6). |
| PIP-09.1–.5 | Unit rate/retry logic plus loopback tests cover pacing, 503 retries, a permanent 401, and 429 cooldown (.1, .3–.4). A native test used timeout alongside fast providers (.2, .5). | Custom-provider pacing is not configured from its definition. Capture bounded default/override timeout and retry diagnostics (.2), connection failures and backoff in the production path (.3), and more than the 401 permanent-error case (.4). Drive repeated refusals alongside a healthy provider and a subsequent recovery (.5). The health indicator is M2. |
| PIP-10.1–.5 | Queue units and console integration cover replacing queued work (.1), storing the active non-streaming result then showing the newest (.3), and a 20-entry burst while a provider is held in flight (.4). The console records skipped pending work and the cache tests cover unsuccessful results (.5). | Add a timed burst across the configured providers for the full .4 case. Verify skip, no retry and no cache together for work cancelled in production (.5). Actual local/streaming cancellation and partial display removal (.2) require the later local and streaming providers. |
| PIP-11.1–.6 | History unit tests cover metadata and bounded eviction (.1, .5). The native walkthrough and console/disk tests prove opt-out defaults, restore and deletion of the saved snapshot/backup (.6). | Back/Forward, untranslated pane and holding an older entry in view (.2–.4) are explicitly M2. |

## Provider definitions, transport and LibreTranslate

Use the [web spec](specs/04-web-connector.md), [provider contract and unit tests](../crates/ta-provider-web/src/lib.rs),
[transport tests](../crates/ta-provider-web/tests/pipeline_transport.rs),
[configured provider walkthrough](walkthroughs/M1-core.md#configured-web-providers)
and [console provider routing](../crates/ta-gui/src/clipboard/provider.rs).

| Feature | Evidence at the checkpoint | Remaining check or implementation |
| --- | --- | --- |
| WEB-01.1–.6 | The console loads enabled definitions from the configured directory (.1). Parser/folder tests exercise malformed TOML and recovery, duplicate IDs, unknown fields, and defaults (.2, .4–.6), and rejection tests exercise missing identity and invalid paths (.3). | Verify .3 across all required fields and the exclusive body kind checks where implemented. Reload and provider management UI are WEB-11 in M2. Surface each valid definition's warnings during console discovery if using the console as the M1 substitute for .5. |
| WEB-02.1–.7 | URL, query and JSON unit tests cover escaping without recursively expanding input, preserve typed JSON data and exercise language map and auto-source omission (.1–.6). Runtime mapping tests inject param values, and the LibreTranslate setup has a native case (.7). | Form bodies and session placeholders require WEB-08 in M2. Custom-provider params are not loaded into the console mapper from settings. Add an app case for success and a missing required value without transport (.7). |
| WEB-03.1–.6 | Local mock and extraction tests cover healthy JSON, wildcards and joins, regex and entity decoding, precedence of service errors, malformed JSON, non-2xx and charset handling (.1–.6). | WEB-08 session refresh in .4 and the WEB-10 Test view details in .2, .5 and .6 come in M2. Preserve the tested console diagnostics and the absence of raw replies by default. |
| WEB-04.1–.6 | Credential substitution and missing-key rejection have console and mapping tests and a native keyed **preset** observation (.1–.2). Redaction tests exercise canary output, errors, request debug and history (.3). The console refuses non-loopback HTTP and rejects redirects, with tests for warnings and redirect refusal (.4–.6). | Verify a custom template receives a disposable Windows Credential Manager value at send time (.1). Add security coverage through every M1 output sink plus later surfaces (.3). Plain-HTTP confirmation (.4) is unmet by refusal. Track the product decision if refusal is retained. WEB-10/UI warnings and key setup actions are later. Prove a cross-host redirect never receives headers or keys before reconciling .6. |
| WEB-05.1–.6 | Unit sentence splitting and loopback pacing, oversize and timeout/retry/cooldown tests cover the main M1 paths. Tests verify ordered manual chunks and refusal before transport for an indivisible sentence (.2–.3). | Wire custom rate limits and add multi-chunk pacing verification (.1), with session-step counts when WEB-08 lands. Verify auto rejection and manual send for the same over-cap entry (.4). Collect complete timeout and permanent/transient cases (.5). A 429 wait is observed but the requested clock-time status is not emitted by the console (.6). |
| WEB-06.1–.5 | Both builds passed fresh settings and configured LibreTranslate, keyless and keyed mock calls, and opt-in live translation (.1–.2, .5). Contract tests extract the recorded translation/error reply (.2–.3). | Test the endpoint classification and its advertised state (.4) for local and off-device configurations. Recheck the provider setup page for .1 in M2. |

## Settings, secrets, performance and privacy

Use the [settings spec](specs/08-settings-and-migration.md),
[quality spec](specs/11-quality-and-release.md),
[settings tests](../crates/ta-platform/src/lib.rs),
[save queue tests](../crates/ta-platform/src/settings_queue.rs),
[credential tests](../crates/ta-platform/src/credentials.rs)
and the [approved Windows timing record](walkthroughs/M1-core.md#windows-results-on-2026-11-21).
The CI workflow covers both Windows targets, but it does not replace independent acceptance
observations. [Main CI](https://github.com/T4ke-Github/Translation-Aggregator-RUST/actions/runs/44951202421)

| Feature | Evidence at the checkpoint | Remaining check or implementation |
| --- | --- | --- |
| CFG-01.1–.5 | Path unit tests cover roaming/local AppData and the portable marker. Both native targets loaded portable settings from a different working directory (.2–.3). | Run a normal AppData install (.1), a read-only settings launch with the required diagnostic (.4), and a same-user cross-bitness settings-path check in CI (.5). |
| CFG-02.1–.5 | Unit tests verify schema key ordering, preserving unknown values/comments and treating a missing version as v1 (.1, .4–.5). Platform tests refuse writes to future-version documents. | There is no pre-v1 migration/versioned backup case (.2). Record future-version notice and preservation on startup, and implement the confirmed backup/save path (.3). Preserve unknown keys and verify warning output for .4. |
| CFG-03.1–.6 | Unit tests cover whole-file replacement, previous backup, preserved comments and save queue debounce/shutdown. | Add the specified 1,000-save interrupted-process run (.1), actual settings-save/backup integration (.2), and wire debounce and timed exit flush into a settings writer (.3–.4). Platform tests verify comment preservation (.5). Second-instance forwarding is missing (.6). |
| CFG-04.1–.5 | Platform tests exercise syntax error recovery, backup fallback, quarantining, default fallback and preservation of broken inputs (.1–.2). The three malformed byte cases have unit coverage (.4). | Verify startup reports the platform diagnostics and passes recovered state through the console (.1–.2). Currently a malformed consumed value can stop launch rather than resetting only that value (.3). Seed a disposable credential and prove it is unchanged across actual recovery (.5). |
| CFG-05.1–.6 | Windows tests and native observations cover the credential prefix, round trip, a missing key and zero provider requests (.1, .3). Console key storage has no file fallback. | Complete the canary search across M1 artifacts and later sinks (.1). Masked settings/Test UI and delete-provider confirmation (.2, .6) require their UI. Prove denied credential writes and moving a portable profile through an integration run (.4–.5). Include the missing-key setting action when the UI is available (.3). |
| NFR-03.1–.5 | Approved native M1 measurements and baseline/load conditions are recorded. In 64-bit/32-bit order, clipboard upper bounds are 0.78/0.87 ms and warm substitutions are 13.71/19.16 ms (.1–.2). | Local API arrival is M5 (.1), dictionaries are M4 (.2), and UI performance (.3–.5) is explicitly M2. |
| NFR-04.1–.9 | Unit and mock tests cover default automatic length, configured input limits, retry timing/policy and default history/cache bounds (.1–.4, .9). | Verify exact timeout defaults and overrides in a contract run (.2), and complete the production retry coverage under PIP-09 (.3). The cache and history capacities have defaults, but both are not configurable in the app (.9). API and plugin limits (.5–.8) are later. |
| NFR-05.1–.6 | Canary tests cover redacted console output and provider history. Remote auto-translate defaults are off. The providerless console mock shows no translation request. CI passed both advisory and non-advisory cargo-deny checks at this revision. | Search all M1 output sinks under a canary security run (.1). Masking and visual off-device labeling depend on later UI (.2–.3), and API protection (.4) is M5. Add an outbound-connection assertion for fresh start and verify the specified local log behavior and rotation (.5). Repeat dependency audit at release for .6. |

## Reconciliation and next checks

1. Run and record PIP-05.2 manually after the consent change. A configured automatic flag, a
   fresh profile, a declined confirmation, and an approved provider with the same and changed
   endpoint must each get an explicit outcome in the walkthrough. Keep the no-automatic-send
   default evidence with PIP-05.1 and NFR-05.3.
2. Add a native verification pass for the missing clipboard boundary cases and app-owned
   clipboard writes. Record the build, Windows version, input and observations for both targets.
3. Prioritize settings startup diagnostics/type recovery, then custom-provider params, pacing
   and runtime reload. Pair helper tests with a production entry-point check.
4. Reconcile the published tracker only once its editable view or a status export is
   available. Use IDs and stated verification methods rather than feature-wide bulk marks.
   Keep partial criteria open and attach the available evidence individually.

The published tracker stores its statuses in a private database outside the checkout. The
extracted rows are an inventory of criteria and do not establish tracker status. This audit
records the repository evidence without changing that database. [Tracker instructions](../tools/spec-tracker/README.md)
