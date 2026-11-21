# Substitution captures

- Legacy build: official Windows x86 release [v1.0.7](https://github.com/Translation-Aggregator/Translation-Aggregator/releases/tag/v1.0.7), archive SHA-256 `5085f30be8aa85eb0d5f5df3bdea41872fb8d346e3d98ba11ada528b038646fc`.
- Captured: 2026-11-21.
- System: Windows 11 Pro build 26300, en-DE, 100% primary display scale.
- Extra data: the synthetic UTF-16LE `inputs/UserDict.txt` in this fixture.

## Steps

1. Start the legacy app in a disposable profile with substitutions enabled, Original Text as the only pane, and provider auto-translation disabled. Put the synthetic UserDict file in its dictionaries directory.
2. Use a temporary 32-bit legacy observer plug-in. Expose `TAPluginGetVersion` with return value 2 and `TAPluginModifyStringPostSubstitution` with the documented calling convention. Save the incoming null-terminated UTF-16 text and return a null pointer so the observer leaves it unchanged.
3. Copy each synthetic input and record the text received by the observer. For the active-profile case, keep a synthetic `debug\translation-aggregator.exe` process running. The rewrite replay maps that legacy process profile to the `api:parity-demo` input context, following PIP-02.3.
4. Replay the results with `cargo test --locked -p ta-ingest --test legacy_parity`.
