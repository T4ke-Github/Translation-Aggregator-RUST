# Translation-Aggregator-RUST

A Rust rewrite of Translation Aggregator for Windows (32-bit and 64-bit). It translates text that
arrives through the clipboard or a local API with every translator you configure (official web APIs,
custom web services, LLMs) and shows the results side by side.

**Status:** M0 (scaffolding). The Cargo workspace and CI exist; the crates are still empty.

## Documentation

- [Contributing](CONTRIBUTING.md): the clean-room rule, building and testing
- [Product direction](docs/product-direction.md): the decisions (authoritative)
- [Design specs](docs/specs/README.md): features and acceptance criteria per milestone
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
