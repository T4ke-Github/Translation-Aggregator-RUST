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
    loop_history: HashMap<String, LoopState>,
}

#[derive(Default)]
struct LoopState {
    observed: Vec<String>,
    cycle: Option<Vec<String>>,
    next_index: usize,
}

impl TextFilter {
    /// Creates an empty filter with independent state for each context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Suppress a repeated cycle of up to eight entries for one context.
    pub fn accept_with_loop_detection(
        &mut self,
        context: &str,
        text: &str,
        enabled: bool,
    ) -> Option<String> {
        if text.is_empty() {
            return None;
        }
        let state = self.loop_history.entry(context.to_owned()).or_default();
        if !enabled {
            *state = LoopState::default();
            return Some(text.to_owned());
        }
        if let Some(cycle) = &state.cycle {
            if text == cycle[state.next_index] {
                state.next_index = (state.next_index + 1) % cycle.len();
                return None;
            }
            *state = LoopState::default();
        } else if !state.observed.is_empty() && text == state.observed[0] {
            state.cycle = Some(state.observed.clone());
            state.next_index = 1 % state.observed.len();
            return None;
        }
        state.observed.push(text.to_owned());
        if state.observed.len() > 8 {
            state.observed.remove(0);
        }
        Some(text.to_owned())
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

/// Keep the final phrase only when each part extends the previous one.
pub fn reduce_phrase_extensions<I, S>(items: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut items = items.into_iter();
    let mut previous = items.next()?.as_ref().to_owned();
    if previous.is_empty() {
        return None;
    }
    for item in items {
        let current = item.as_ref();
        if !current.starts_with(&previous) || current.len() == previous.len() {
            return None;
        }
        previous = current.to_owned();
    }
    Some(previous)
}

/// Return a repeated phrase of 2 to 40 Unicode scalar values, if found.
pub fn reduce_phrase_repeat(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    for phrase_len in 2..=40.min(chars.len() / 2) {
        if !chars.len().is_multiple_of(phrase_len) {
            continue;
        }
        let copies = chars.len() / phrase_len;
        if copies >= 2 && chars[..phrase_len].repeat(copies) == chars {
            return Some(chars[..phrase_len].iter().collect());
        }
    }
    None
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

    #[test]
    fn phrase_extensions_keep_the_longest_flush() {
        assert_eq!(
            reduce_phrase_extensions(["こ", "こん", "こんに", "こんにちは"]),
            Some("こんにちは".into())
        );
    }

    #[test]
    fn phrase_repeat_is_bounded_and_uses_unicode_characters() {
        assert_eq!(reduce_phrase_repeat("はいはいはい"), Some("はい".into()));
        let phrase = "あいうえおかきくけこ";
        assert_eq!(reduce_phrase_repeat(&phrase.repeat(3)), Some(phrase.into()));
        assert_eq!(reduce_phrase_repeat("ああ"), None);
        assert_eq!(reduce_phrase_repeat(&"あ".repeat(41)), None);
    }

    #[test]
    fn text_loop_detection_is_per_context_and_resets_outside_cycle() {
        let mut f = TextFilter::new();
        for text in ["A", "B", "C"] {
            assert_eq!(
                f.accept_with_loop_detection("x", text, true),
                Some(text.into())
            );
        }
        for text in ["A", "B", "C"] {
            assert_eq!(f.accept_with_loop_detection("x", text, true), None);
        }
        assert_eq!(
            f.accept_with_loop_detection("y", "A", true),
            Some("A".into())
        );
        assert_eq!(
            f.accept_with_loop_detection("x", "D", true),
            Some("D".into())
        );
        assert_eq!(
            f.accept_with_loop_detection("x", "A", true),
            Some("A".into())
        );
    }
}
