//! EDICT-format dictionaries, self-authored conjugation rules, JParser matching and the dictionary cache.
//!
//! `unsafe` is denied here; a later item may allow it locally, with a `// SAFETY:` comment,
//! where the layout says so (memory-mapped files or the optional FFI backend).
//!
//! Status: M0 skeleton. See `docs/specs/` for what this crate will do.

#![deny(unsafe_code)]
