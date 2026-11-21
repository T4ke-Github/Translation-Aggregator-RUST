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
    /// Reduce an exact repeated phrase of 2 to 40 Unicode scalar values.
    pub phrase_repeat: bool,
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
            phrase_repeat: false,
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
    if options.phrase_repeat
        && let Some(phrase) = reduce_phrase_repeat(&text)
    {
        text = phrase;
    }
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

/// How source contexts feed their configured merged context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeMode {
    /// Forward each piece to the merged context on arrival.
    Stream,
    /// Forward buffered text when the source context flushes.
    OnFlush,
}

/// Buffer policy. Time values are caller-supplied monotonic milliseconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BufferSettings {
    /// Whether the context accepts text.
    pub enabled: bool,
    /// Idle time after the last piece before flushing, in milliseconds.
    pub idle_delay_ms: u64,
    /// Optional route into the merged context.
    pub merge: Option<MergeMode>,
    /// Keep only the last piece when each piece extends the one before.
    pub phrase_extensions: bool,
}
impl Default for BufferSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            idle_delay_ms: 0,
            merge: None,
            phrase_extensions: false,
        }
    }
}
impl BufferSettings {
    /// Choose the default idle delay for an input context.
    pub fn default_for(context: &str) -> Self {
        Self {
            idle_delay_ms: if matches!(context, "api" | "plugin") {
                250
            } else {
                0
            },
            ..Self::default()
        }
    }
}

/// Text emitted by an idle flush, with its destination context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlushedText {
    /// Destination context.
    pub context: String,
    /// Text in arrival order.
    pub text: String,
}

#[derive(Default)]
struct Pending {
    text: String,
    pieces: Vec<String>,
    last_at: u64,
}

/// Deterministic idle buffer. Call `flush_due` with the same monotonic clock as `push`.
/// Merges are emitted into a separately configured context, in sorted source-context order on flush.
pub struct InputBuffer {
    settings: HashMap<String, BufferSettings>,
    pending: HashMap<String, Pending>,
}
impl Default for InputBuffer {
    fn default() -> Self {
        Self::new()
    }
}
impl InputBuffer {
    /// Create an empty buffer with default policies.
    pub fn new() -> Self {
        Self {
            settings: HashMap::new(),
            pending: HashMap::new(),
        }
    }
    /// Set a context policy, dropping pending text when it becomes disabled.
    pub fn configure(&mut self, context: &str, settings: BufferSettings) {
        self.settings.insert(context.to_owned(), settings);
        if !settings.enabled {
            self.pending.remove(context);
        }
    }
    /// Add a piece at monotonic `now_ms`, returning any immediate output.
    pub fn push(&mut self, context: &str, text: &str, now_ms: u64) -> Vec<FlushedText> {
        let cfg = self
            .settings
            .get(context)
            .copied()
            .unwrap_or_else(|| BufferSettings::default_for(context));
        if !cfg.enabled {
            return Vec::new();
        }
        if context != "merged" && cfg.merge == Some(MergeMode::Stream) {
            return self.push("merged", text, now_ms);
        }
        if cfg.idle_delay_ms == 0 {
            if context != "merged" && cfg.merge == Some(MergeMode::OnFlush) {
                return self.push("merged", text, now_ms);
            }
            return vec![FlushedText {
                context: context.into(),
                text: text.into(),
            }];
        }
        let pending = self.pending.entry(context.into()).or_default();
        pending.text.push_str(text);
        pending.pieces.push(text.to_owned());
        pending.last_at = now_ms;
        Vec::new()
    }
    /// Emit contexts whose idle delay has passed at monotonic `now_ms`.
    pub fn flush_due(&mut self, now_ms: u64) -> Vec<FlushedText> {
        let mut due: Vec<String> = self
            .pending
            .iter()
            .filter_map(|(ctx, p)| {
                let cfg = self
                    .settings
                    .get(ctx)
                    .copied()
                    .unwrap_or_else(|| BufferSettings::default_for(ctx));
                (cfg.enabled && now_ms.saturating_sub(p.last_at) >= cfg.idle_delay_ms)
                    .then(|| ctx.clone())
            })
            .collect();
        due.sort();
        let mut out = Vec::new();
        let mut merged = String::new();
        for ctx in due {
            let Some(pending) = self.pending.remove(&ctx) else {
                continue;
            };
            let cfg = self
                .settings
                .get(&ctx)
                .copied()
                .unwrap_or_else(|| BufferSettings::default_for(&ctx));
            let text = if cfg.phrase_extensions {
                reduce_phrase_extensions(&pending.pieces).unwrap_or(pending.text)
            } else {
                pending.text
            };
            if ctx != "merged" && cfg.merge == Some(MergeMode::OnFlush) {
                merged.push_str(&text);
            } else {
                out.push(FlushedText { context: ctx, text });
            }
        }
        if !merged.is_empty() {
            out.extend(self.push("merged", &merged, now_ms));
        }
        out
    }
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
    fn buffer_flushes_idle_per_context_and_merges_configured_sources() {
        let mut b = InputBuffer::new();
        b.configure("api", BufferSettings::default_for("api"));
        assert!(b.push("api", "a", 0).is_empty());
        assert!(b.push("api", "b", 50).is_empty());
        assert!(b.push("api", "c", 100).is_empty());
        assert!(b.flush_due(349).is_empty());
        assert_eq!(
            b.flush_due(350),
            [FlushedText {
                context: "api".into(),
                text: "abc".into()
            }]
        );
    }

    #[test]
    fn buffer_defaults_immediate_and_contexts_are_independent() {
        let mut b = InputBuffer::new();
        assert_eq!(BufferSettings::default_for("clipboard").idle_delay_ms, 0);
        assert_eq!(BufferSettings::default_for("typed").idle_delay_ms, 0);
        assert_eq!(BufferSettings::default_for("api").idle_delay_ms, 250);
        assert_eq!(BufferSettings::default_for("plugin").idle_delay_ms, 250);
        b.configure(
            "a",
            BufferSettings {
                idle_delay_ms: 10,
                ..Default::default()
            },
        );
        b.configure(
            "b",
            BufferSettings {
                idle_delay_ms: 20,
                ..Default::default()
            },
        );
        assert!(b.push("a", "A", 0).is_empty());
        assert_eq!(b.push("typed", "T", 0)[0].text, "T");
        assert!(b.push("b", "B", 0).is_empty());
        assert_eq!(b.flush_due(10)[0].context, "a");
        assert_eq!(b.flush_due(20)[0].context, "b");
    }

    #[test]
    fn disabling_context_discards_pending_and_merge_modes_are_deterministic() {
        let mut b = InputBuffer::new();
        b.configure(
            "x",
            BufferSettings {
                idle_delay_ms: 5,
                merge: Some(MergeMode::OnFlush),
                ..Default::default()
            },
        );
        b.configure(
            "y",
            BufferSettings {
                idle_delay_ms: 5,
                merge: Some(MergeMode::OnFlush),
                ..Default::default()
            },
        );
        b.configure(
            "merged",
            BufferSettings {
                idle_delay_ms: 7,
                ..Default::default()
            },
        );
        b.push("x", "X", 0);
        b.push("y", "Y", 0);
        assert!(b.flush_due(5).is_empty());
        assert!(b.push("merged", "!", 7).is_empty());
        assert_eq!(b.flush_due(14)[0].text, "XY!");
        b.configure(
            "z",
            BufferSettings {
                idle_delay_ms: 20,
                ..Default::default()
            },
        );
        b.push("z", "drop", 20);
        b.configure(
            "z",
            BufferSettings {
                enabled: false,
                ..Default::default()
            },
        );
        assert!(b.flush_due(100).is_empty());
    }

    #[test]
    fn merge_stream_routes_each_piece_through_merged_idle_buffer() {
        let mut b = InputBuffer::new();
        b.configure(
            "x",
            BufferSettings {
                merge: Some(MergeMode::Stream),
                ..Default::default()
            },
        );
        b.configure(
            "y",
            BufferSettings {
                merge: Some(MergeMode::Stream),
                ..Default::default()
            },
        );
        b.configure(
            "merged",
            BufferSettings {
                idle_delay_ms: 10,
                ..Default::default()
            },
        );
        assert!(b.push("x", "A", 0).is_empty());
        assert!(b.push("y", "B", 5).is_empty());
        assert_eq!(b.flush_due(15)[0].text, "AB");
    }

    #[test]
    fn merge_stream_ignores_source_delay_and_preserves_arrival_order() {
        let mut buffer = InputBuffer::new();
        buffer.configure(
            "x",
            BufferSettings {
                idle_delay_ms: 250,
                merge: Some(MergeMode::Stream),
                ..Default::default()
            },
        );
        buffer.configure(
            "y",
            BufferSettings {
                idle_delay_ms: 250,
                merge: Some(MergeMode::Stream),
                ..Default::default()
            },
        );
        buffer.configure(
            "merged",
            BufferSettings {
                idle_delay_ms: 10,
                ..Default::default()
            },
        );
        assert!(buffer.push("y", "first", 0).is_empty());
        assert!(buffer.push("x", "second", 5).is_empty());
        assert!(buffer.flush_due(14).is_empty());
        assert_eq!(
            buffer.flush_due(15),
            vec![FlushedText {
                context: "merged".into(),
                text: "firstsecond".into(),
            }]
        );
        assert!(buffer.flush_due(250).is_empty());
    }

    #[test]
    fn merge_on_flush_emits_immediately_with_zero_merged_delay() {
        let mut buffer = InputBuffer::new();
        buffer.configure(
            "x",
            BufferSettings {
                idle_delay_ms: 5,
                merge: Some(MergeMode::OnFlush),
                ..Default::default()
            },
        );
        buffer.configure(
            "y",
            BufferSettings {
                idle_delay_ms: 5,
                merge: Some(MergeMode::OnFlush),
                ..Default::default()
            },
        );
        buffer.push("x", "A", 0);
        buffer.push("y", "B", 0);
        assert_eq!(
            buffer.flush_due(5),
            vec![FlushedText {
                context: "merged".into(),
                text: "AB".into(),
            }]
        );
        assert!(buffer.flush_due(10).is_empty());
    }

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
    fn phrase_extension_flush_keeps_longest_piece_only_when_enabled() {
        for (pieces, enabled, expected) in [
            (
                vec!["こ", "こん", "こんに", "こんにちは"],
                true,
                "こんにちは",
            ),
            (
                vec!["こ", "こん", "こんに", "こんにちは"],
                false,
                "ここんこんにこんにちは",
            ),
            (vec!["こ", "違う"], true, "こ違う"),
        ] {
            let mut buffer = InputBuffer::new();
            buffer.configure(
                "hook:game",
                BufferSettings {
                    idle_delay_ms: 10,
                    phrase_extensions: enabled,
                    ..Default::default()
                },
            );
            for (index, piece) in pieces.iter().enumerate() {
                assert!(buffer.push("hook:game", piece, index as u64).is_empty());
            }
            let last_at = pieces.len() as u64 - 1;
            assert_eq!(buffer.flush_due(last_at + 9), Vec::<FlushedText>::new());
            assert_eq!(buffer.flush_due(last_at + 10)[0].text, expected);
        }
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
    fn phrase_repeat_runs_before_substitutions_only_when_enabled() {
        let engine = SubstitutionEngine::new(
            true,
            vec![
                ta_substitute::SubstitutionProfile::new("*")
                    .with_rules([ta_substitute::SubstitutionRule::try_new("はい", "yes").unwrap()]),
            ],
        );
        for (source, enabled, expected) in [
            ("はいはいはい", true, "yes"),
            ("はいはいはい", false, "yesyesyes"),
            ("はいそう", true, "yesそう"),
            ("あ", true, "あ"),
        ] {
            let mut filter = TextFilter::new();
            let mut history = History::default();
            let result = prepare_input(
                TextInput::new(source).unwrap(),
                ContextId::new("typed").unwrap(),
                &mut filter,
                &engine,
                &mut history,
                PipelineOptions {
                    phrase_repeat: enabled,
                    ..PipelineOptions::default()
                },
                PipelineTransforms {
                    pre_substitution: |s: &str| Ok(s.to_owned()),
                    post_substitution: |s: &str| Ok(s.to_owned()),
                },
            );
            let PreparedInput::Ready(prepared) = result else {
                panic!("expected ready")
            };
            assert_eq!(prepared.original_text, source);
            assert_eq!(prepared.processed_text, expected);
            assert_eq!(
                history.get(&prepared.history_id).unwrap().processed_text,
                expected
            );
        }
    }

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
mod clipboard_gate;
pub use clipboard_gate::ClipboardGate;
