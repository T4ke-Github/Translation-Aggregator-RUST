//! Text input: the clipboard listener and the local input/output API, feeding the pipeline.
//!
//! This crate contains no `unsafe` code (`docs/analysis/workspace-layout.md`).
//!
//! Status: M0 skeleton. See `docs/specs/` for what this crate will do.

#![forbid(unsafe_code)]

use std::collections::HashMap;

/// Pure text acceptance state; callers invoke this after a flush.
#[derive(Default)]
pub struct TextFilter {
    last_by_context: HashMap<String, String>,
}

impl TextFilter {
    /// Creates an empty filter with independent state for each context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns accepted text unchanged. `manual` always bypasses duplicate suppression.
    pub fn accept(
        &mut self,
        context: &str,
        text: &str,
        japanese_only: bool,
        suppress_repeats: bool,
        manual: bool,
    ) -> Option<String> {
        if text.is_empty() || (japanese_only && !contains_japanese_script(text)) {
            return None;
        }
        let previous = self.last_by_context.get(context);
        if suppress_repeats && !manual && previous.is_some_and(|last| last == text) {
            return None;
        }
        self.last_by_context
            .insert(context.to_owned(), text.to_owned());
        Some(text.to_owned())
    }
}

fn contains_japanese_script(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(ch as u32,
        0x3041..=0x3096 | 0x309D..=0x309F | 0x30A1..=0x30FA | 0x30FD..=0x30FF | 0x31F0..=0x31FF |
        0xFF66..=0xFF9D | 0x3400..=0x4DBF | 0x4E00..=0x9FFF |
        0xF900..=0xFAFF | 0x20000..=0x3134F)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn japanese_filter_requires_japanese_script_and_preserves_text() {
        let mut filter = TextFilter::new();
        assert_eq!(
            filter.accept("ctx", "OK、了解", true, true, false),
            Some("OK、了解".into())
        );
        assert_eq!(filter.accept("ctx", "hello 123!", true, true, false), None);
        assert_eq!(
            filter.accept("ctx", "ｶﾀｶﾅ", true, true, false),
            Some("ｶﾀｶﾅ".into())
        );
        assert_eq!(
            filter.accept("ctx", "ー・ゝ", true, true, false),
            Some("ー・ゝ".into())
        );
        assert_eq!(filter.accept("ctx", "。、！？", true, true, false), None);
    }

    #[test]
    fn duplicate_suppression_is_per_context_after_flush_and_manual_bypasses() {
        let mut filter = TextFilter::new();
        assert_eq!(
            filter.accept("a", "同じ", false, true, false),
            Some("同じ".into())
        );
        assert_eq!(filter.accept("a", "同じ", false, true, false), None);
        assert_eq!(
            filter.accept("b", "同じ", false, true, false),
            Some("同じ".into())
        );
        assert_eq!(
            filter.accept("a", "同じ", false, true, true),
            Some("同じ".into())
        );
        assert_eq!(filter.accept("a", "同じ", false, true, false), None);
        assert_eq!(
            filter.accept("a", "同じ", false, false, false),
            Some("同じ".into())
        );
    }
}
