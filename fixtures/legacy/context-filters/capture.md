# Context-filter captures

- Legacy build: official Windows x86 release [v1.0.7](https://github.com/Translation-Aggregator/Translation-Aggregator/releases/tag/v1.0.7), archive SHA-256 `5085f30be8aa85eb0d5f5df3bdea41872fb8d346e3d98ba11ada528b038646fc`.
- Captured: 2026-11-21.
- System: Windows 11 Pro build 26300, en-DE, 100% primary display scale.
- Extra data: none.

## Steps

1. Start the legacy app in a fresh directory with only its Original Text pane open. Disable provider auto-translation.
2. For clipboard cases, copy a synthetic line to create the Clipboard context. Select it in Context Manager and set the options in each case. Copy the case input and read the Original Text pane after it settles. Record a rejected input as `null` when the previous display stays in place. Normalize the pane's CRLF to LF in the fixture.
3. For phrase extension, loop detection, and merge stream, connect a synthetic 32-bit sender to the documented legacy loopback context interface. Enable Auto translate for each source and select the case's filter or merge setting. Send only the listed text. For loop detection, read the pane after each input, including when it stays unchanged. For merge cases, record each new displayed entry.
4. Replay with `cargo test --locked -p ta-ingest --test legacy_parity`. The clipboard comparison uses the rewrite's processed text because the console also exposes the raw input separately.

## Observed differences

- `ctx-repeat-constant`: the legacy app collapsed uneven runs. IN-08.2 requires leaving them intact.
- `ctx-phrase-repeat`: the legacy app kept the short repeated phrase. IN-08.4 requires reducing it to one copy.
- `ctx-detect-loop`: the legacy app kept showing the three-entry cycle in this sequence. IN-08.5 requires suppression after the first cycle.
- `ctx-line-breaks-keep-some` and `ctx-line-breaks-keep-some-next`: the legacy app removed all breaks with First = 1 and Last = 0. IN-09.3 requires keeping the first break.

- `ctx-merge-on-flush`: the legacy pane stayed unchanged on repeated runs. Both source contexts retained Auto translate and Merge on flush. The rewrite emits one entry in the merged context, as specified by IN-06.4.
