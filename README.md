# Translation-Aggregator-RUST

A Rust rewrite of Translation Aggregator for Windows (32-bit and 64-bit). It translates text that
arrives through the clipboard or a local API with every translator you configure (official web APIs,
custom web services, LLMs) and shows the results side by side.

**Status:** M1 (core) in progress. The workspace has working parts and tests, but the M1 exit gate
is not met. See the [handover note](docs/handover.md) for the next work and open checks.

On Windows, start the developer console against a running keyless LibreTranslate server on
loopback. Copy text after the readiness message and use Ctrl+C to stop capture.

```powershell
cargo run --locked -p ta-gui -- --clipboard --libretranslate-server 127.0.0.1:5000 --source ja --target en
```

To use a saved server address or a key from Windows Credential Manager, see the
[LibreTranslate settings walkthrough](docs/walkthroughs/M1-core.md#settings-and-optional-key).

## Documentation

- [Contributing](CONTRIBUTING.md): the clean-room rule, building and testing
- [Product direction](docs/product-direction.md): the decisions (authoritative)
- [Design specs](docs/specs/README.md): features and acceptance criteria per milestone
- [Project plan](docs/project-plan.md): milestone roles, work order and exit gates
- [M1 walkthrough](docs/walkthroughs/M1-core.md): clipboard checks with a local mock on both Windows builds
- [Analysis](docs/analysis/README.md): how the legacy app works, and the technical approach
- [Custom web services guide](docs/guides/custom-web-services.md)
- [CI](.github/CI.md)
- [Wiki](https://github.com/T4ke-Github/Translation-Aggregator-RUST/wiki)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

This is a clean reimplementation. The legacy Translation Aggregator (GPLv2) is used only as a
reference for features, behaviour and file formats; no legacy code or legacy-authored data files are
included (see D13 in the product direction).

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the
work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
