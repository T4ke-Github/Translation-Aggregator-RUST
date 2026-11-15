//! Text input: the clipboard listener and the local input/output API, feeding the pipeline.
//!
//! This crate contains no `unsafe` code (`docs/analysis/workspace-layout.md`).
//!
//! Status: M0 skeleton. See `docs/specs/` for what this crate will do.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use ta_core::{ContextId, History, HistoryEntry, HistoryEntryId, TextInput};
use ta_substitute::SubstitutionEngine;
use ta_text::{CharacterRepeatMode, NewlineMode};

/// Pipeline-level switches for duplicate and repeat cleanup.
#[derive(Clone, Copy, Debug)]
pub struct PipelineOptions {
    /// Accept only inputs containing Japanese script.
    pub japanese_only: bool,
    /// Suppress the same text repeated in one context.
    pub suppress_repeats: bool,
    /// Bypass automatic duplicate suppression for a user action.
    pub manual: bool,
    /// Suppress repeated cycles of input entries.
    pub detect_loops: bool,
    /// Character-repeat cleanup mode.
    pub repeat_mode: CharacterRepeatMode,
    /// Line-break cleanup mode.
    pub newline_mode: NewlineMode,
}
impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            japanese_only: false,
            suppress_repeats: false,
            manual: false,
            detect_loops: false,
            repeat_mode: CharacterRepeatMode::None,
            newline_mode: NewlineMode::Keep,
        }
    }
}

/// Reason an input did not proceed to translation routing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkipReason {
    /// Cleanup produced empty text.
    EmptyAfterNormalization,
    /// A per-context filter rejected the input.
    Filtered,
    /// The input repeated a detected cycle.
    LoopDetected,
    /// Substitutions or transforms removed all text.
    EmptyAfterSubstitutions,
}

/// Result of preparing an input for routing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreparedInput {
    /// Prepared text and its history entry.
    Ready(PreparedText),
    /// Input rejected before routing.
    Skipped(SkipReason),
}

/// Text prepared for all eligible translators.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedText {
    /// ID of the stored history entry.
    pub history_id: HistoryEntryId,
    /// Validated source identity.
    pub context: ContextId,
    /// Original accepted input.
    pub original_text: String,
    /// Final text after cleanup and substitutions.
    pub processed_text: String,
    /// Non-fatal transform failures.
    pub warnings: Vec<String>,
}

/// Hooks run before and after substitution, in that order.
pub struct PipelineTransforms<P, Q> {
    /// Transform applied before substitution.
    pub pre_substitution: P,
    /// Transform applied after substitution.
    pub post_substitution: Q,
}

/// Run deterministic text stages only; transform failures warn and preserve input.
pub fn prepare_input<P, Q>(
    input: TextInput,
    context: ContextId,
    filter: &mut TextFilter,
    substitutions: &SubstitutionEngine,
    history: &mut History,
    options: PipelineOptions,
    transforms: PipelineTransforms<P, Q>,
) -> PreparedInput
where
    P: FnOnce(&str) -> Result<String, String>,
    Q: FnOnce(&str) -> Result<String, String>,
{
    let PipelineTransforms {
        pre_substitution,
        post_substitution,
    } = transforms;
    let original = input.as_str().to_owned();
    let Some(accepted) = filter.accept(
        context.as_str(),
        &original,
        options.japanese_only,
        options.suppress_repeats,
        options.manual,
    ) else {
        return PreparedInput::Skipped(SkipReason::Filtered);
    };
    if filter
        .accept_with_loop_detection(context.as_str(), &accepted, options.detect_loops)
        .is_none()
    {
        return PreparedInput::Skipped(SkipReason::LoopDetected);
    }
    let mut text = ta_text::normalize_repeats(&accepted, options.repeat_mode);
    text = ta_text::normalize_newlines(&text, options.newline_mode);
    if text.trim().is_empty() {
        return PreparedInput::Skipped(SkipReason::EmptyAfterNormalization);
    }
    let mut warnings = Vec::new();
    text = match pre_substitution(&text) {
        Ok(value) => value,
        Err(error) => {
            warnings.push(format!("pre-substitution transform skipped: {error}"));
            text
        }
    };
    text = substitutions.apply(&text, context.as_str());
    if text.trim().is_empty() {
        history.push(HistoryEntry::new(
            HistoryEntryId::new(),
            std::time::SystemTime::now(),
            context,
            original,
            text,
        ));
        return PreparedInput::Skipped(SkipReason::EmptyAfterSubstitutions);
    }
    text = match post_substitution(&text) {
        Ok(value) => value,
        Err(error) => {
            warnings.push(format!("post-substitution transform skipped: {error}"));
            text
        }
    };
    if text.trim().is_empty() {
        history.push(HistoryEntry::new(
            HistoryEntryId::new(),
            std::time::SystemTime::now(),
            context,
            original,
            text,
        ));
        return PreparedInput::Skipped(SkipReason::EmptyAfterSubstitutions);
    }
    let id = HistoryEntryId::new();
    history.push(HistoryEntry::new(
        id,
        std::time::SystemTime::now(),
        context.clone(),
        original.clone(),
        text.clone(),
    ));
    PreparedInput::Ready(PreparedText {
        history_id: id,
        context,
        original_text: original,
        processed_text: text,
        warnings,
    })
}

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

#[cfg(test)]
mod pipeline_tests {
    use super::*;

    #[test]
    fn marker_transforms_bracket_substitutions_and_history_precedes_routing() {
        let context = ta_core::ContextId::new("typed").unwrap();
        let input = ta_core::TextInput::new("hello").unwrap();
        let engine =
            ta_substitute::SubstitutionEngine::new(
                true,
                vec![ta_substitute::SubstitutionProfile::new("*").with_rules([
                    ta_substitute::SubstitutionRule::try_new("hello", "sub").unwrap(),
                ])],
            );
        let mut filter = TextFilter::new();
        let mut history = ta_core::History::default();
        let result = prepare_input(
            input,
            context,
            &mut filter,
            &engine,
            &mut history,
            PipelineOptions::default(),
            PipelineTransforms {
                pre_substitution: |text: &str| Ok(format!("pre[{text}]")),
                post_substitution: |text: &str| Ok(format!("post[{text}]")),
            },
        );
        let PreparedInput::Ready(prepared) = result else {
            panic!("expected ready")
        };
        assert_eq!(prepared.original_text, "hello");
        assert_eq!(prepared.processed_text, "post[pre[sub]]");
        assert_eq!(history.len(), 1);
        assert_eq!(
            history.get(&prepared.history_id).unwrap().processed_text,
            prepared.processed_text
        );
    }

    #[test]
    fn empty_substitution_skips_and_plugin_failure_warns_and_passes_through() {
        let context = ta_core::ContextId::new("typed").unwrap();
        let input = ta_core::TextInput::new("hello").unwrap();
        let engine = ta_substitute::SubstitutionEngine::new(
            true,
            vec![
                ta_substitute::SubstitutionProfile::new("*")
                    .with_rules([ta_substitute::SubstitutionRule::try_new("hello", "").unwrap()]),
            ],
        );
        let mut filter = TextFilter::new();
        let mut history = ta_core::History::default();
        assert!(matches!(
            prepare_input(
                input,
                context,
                &mut filter,
                &engine,
                &mut history,
                PipelineOptions::default(),
                PipelineTransforms {
                    pre_substitution: |s: &str| Ok(s.to_owned()),
                    post_substitution: |s: &str| Ok(s.to_owned()),
                },
            ),
            PreparedInput::Skipped(SkipReason::EmptyAfterSubstitutions)
        ));
        assert_eq!(history.len(), 1);

        let context = ta_core::ContextId::new("typed").unwrap();
        let input = ta_core::TextInput::new("hello").unwrap();
        let engine = ta_substitute::SubstitutionEngine::new(false, vec![]);
        let result = prepare_input(
            input,
            context,
            &mut filter,
            &engine,
            &mut history,
            PipelineOptions::default(),
            PipelineTransforms {
                pre_substitution: |_: &str| Err("plugin broke".into()),
                post_substitution: |s: &str| Ok(s.to_owned()),
            },
        );
        let PreparedInput::Ready(prepared) = result else {
            panic!("expected ready")
        };
        assert_eq!(prepared.processed_text, "hello");
        assert_eq!(
            prepared.warnings,
            ["pre-substitution transform skipped: plugin broke"]
        );
    }
}
