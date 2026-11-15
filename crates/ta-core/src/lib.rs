//! Core text-input value types and validation.

#![forbid(unsafe_code)]

use std::{collections::HashMap, fmt};

/// Maximum context identifier length in Unicode scalar values.
pub const MAX_CONTEXT_ID_CHARS: usize = 128;

/// Validated identity for one input stream.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContextId(String);

impl ContextId {
    /// Validate a context identifier and preserve its spelling exactly.
    pub fn new(value: impl Into<String>) -> Result<Self, ContextIdError> {
        let value = value.into();
        let chars = value.chars().count();
        if chars == 0 {
            return Err(ContextIdError::Empty);
        }
        if chars > MAX_CONTEXT_ID_CHARS {
            return Err(ContextIdError::TooLong {
                actual_chars: chars,
                max_chars: MAX_CONTEXT_ID_CHARS,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(ContextIdError::ControlCharacter);
        }
        let valid = match value.as_str() {
            "clipboard" | "typed" => true,
            _ if value.starts_with("api:") => !value[4..].is_empty(),
            _ if value.starts_with("plugin:") => value[7..]
                .split_once('/')
                .is_some_and(|(plugin, source)| !plugin.is_empty() && !source.is_empty()),
            _ => false,
        };
        if !valid {
            return Err(ContextIdError::InvalidFormat);
        }
        Ok(Self(value))
    }

    /// Borrow the identifier string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Context identifier validation failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextIdError {
    /// The identifier is empty.
    Empty,
    /// The identifier exceeds the scalar-value limit.
    TooLong {
        /// Number of Unicode scalar values supplied.
        actual_chars: usize,
        /// Maximum accepted number of Unicode scalar values.
        max_chars: usize,
    },
    /// The identifier contains a Unicode control character.
    ControlCharacter,
    /// The identifier does not follow a supported context form.
    InvalidFormat,
}

impl fmt::Display for ContextIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("context id must not be empty"),
            Self::TooLong {
                actual_chars,
                max_chars,
            } => write!(
                f,
                "context id is {actual_chars} characters; maximum is {max_chars}"
            ),
            Self::ControlCharacter => f.write_str("context id must not contain control characters"),
            Self::InvalidFormat => f.write_str(
                "context id must be clipboard, typed, api:<source>, or plugin:<plugin-id>/<source>",
            ),
        }
    }
}
impl std::error::Error for ContextIdError {}

/// User-configurable options shared as the new-context template.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextOptions {
    /// Milliseconds of inactivity before buffered input is flushed.
    pub idle_delay_ms: u64,
}

/// One context and its independent option values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputContext {
    id: ContextId,
    options: ContextOptions,
}
impl InputContext {
    /// Context identity.
    pub fn id(&self) -> &ContextId {
        &self.id
    }
    /// Context-specific options.
    pub fn options(&self) -> &ContextOptions {
        &self.options
    }
    /// Mutably access context-specific options.
    pub fn options_mut(&mut self) -> &mut ContextOptions {
        &mut self.options
    }
}

/// In-memory registry of contexts. Persistence and UI are intentionally separate.
#[derive(Clone, Debug, Default)]
pub struct ContextRegistry {
    contexts: HashMap<ContextId, InputContext>,
}
impl ContextRegistry {
    /// Return an existing context or create one from the template and source defaults.
    pub fn get_or_create(&mut self, id: ContextId, template: &ContextOptions) -> &mut InputContext {
        self.contexts.entry(id.clone()).or_insert_with(|| {
            let mut options = template.clone();
            if id.as_str() == "clipboard" || id.as_str() == "typed" {
                options.idle_delay_ms = 0;
            } else {
                options.idle_delay_ms = 250;
            }
            InputContext { id, options }
        })
    }
    /// Find a registered context without creating it.
    pub fn get(&self, id: &ContextId) -> Option<&InputContext> {
        self.contexts.get(id)
    }
    /// Number of registered contexts.
    pub fn len(&self) -> usize {
        self.contexts.len()
    }
    /// Whether no contexts have been registered.
    pub fn is_empty(&self) -> bool {
        self.contexts.is_empty()
    }
}

/// Default maximum accepted input length, measured in Unicode scalar values.
pub const DEFAULT_MAX_INPUT_CHARS: usize = 20_000;

/// Validated user-provided text in the canonical newline representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextInput(String);

impl TextInput {
    /// Construct an input with the default limit, normalizing line endings and rejecting empty or oversized text.
    pub fn new(value: &str) -> Result<Self, TextInputError> {
        Self::with_max_chars(value, DEFAULT_MAX_INPUT_CHARS)
    }

    /// Construct an input with a user-supplied maximum Unicode scalar-value count.
    pub fn with_max_chars(value: &str, max_chars: usize) -> Result<Self, TextInputError> {
        let actual_chars = value.chars().count();
        if actual_chars > max_chars {
            return Err(TextInputError::TooLarge {
                actual_chars,
                max_chars,
            });
        }
        let normalized = ta_text::normalize_input(value);
        if normalized.trim().is_empty() {
            return Err(TextInputError::Empty);
        }
        Ok(Self(normalized))
    }

    /// Return the normalized text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the input contains Japanese-script characters.
    pub fn contains_japanese(&self) -> bool {
        ta_text::contains_japanese(&self.0)
    }

    /// Consume this value and return the normalized text.
    pub fn into_string(self) -> String {
        self.0
    }
}

/// Reasons input construction can fail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextInputError {
    /// Input contains only whitespace (or no characters).
    Empty,
    /// Unicode scalar-value count exceeds the configured limit.
    TooLarge {
        /// Number of Unicode scalar values in the supplied input.
        actual_chars: usize,
        /// Maximum number of accepted Unicode scalar values.
        max_chars: usize,
    },
}

impl fmt::Display for TextInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("text input must not be empty"),
            Self::TooLarge {
                actual_chars,
                max_chars,
            } => write!(
                f,
                "text input is {actual_chars} characters; maximum is {max_chars}"
            ),
        }
    }
}

impl std::error::Error for TextInputError {}

/// Default maximum number of in-memory history entries.
pub const DEFAULT_HISTORY_CAPACITY: usize = 10_000;

/// Opaque, typed identity for a history entry.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HistoryEntryId(u64);

impl HistoryEntryId {
    /// Allocate a process-unique history entry identifier.
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for HistoryEntryId {
    fn default() -> Self {
        Self::new()
    }
}

/// Typed provider identity.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProviderId(String);

impl ProviderId {
    /// Create a provider identity, rejecting empty/whitespace identifiers.
    pub fn new(value: impl Into<String>) -> Result<Self, ProviderIdError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ProviderIdError);
        }
        Ok(Self(value))
    }
    /// Borrow the identifier string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Invalid provider identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderIdError;
impl fmt::Display for ProviderIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("provider id must not be empty")
    }
}
impl std::error::Error for ProviderIdError {}

/// Provider request result or failure, preserving its message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderOutcome {
    /// Successful processed provider text.
    Result(String),
    /// Provider error message.
    Error(String),
}

/// Lifecycle state of a provider result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderStatus {
    /// Request is still in progress.
    Pending,
    /// Request completed with a result.
    Succeeded,
    /// Request completed with an error.
    Failed,
}

/// Per-provider outcome and request metadata for a history entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderHistory {
    /// Provider text or error.
    pub outcome: ProviderOutcome,
    /// Provider lifecycle status.
    pub status: ProviderStatus,
    /// Request duration.
    pub elapsed: std::time::Duration,
    /// Whether this result came from cache.
    pub cached: bool,
}

/// One processed input and all provider outcomes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryEntry {
    /// Stable typed entry identity.
    pub id: HistoryEntryId,
    /// Time this entry was stored.
    pub time: std::time::SystemTime,
    /// Input context.
    pub context: ContextId,
    /// Submitted text.
    pub original_text: String,
    /// Text after preprocessing.
    pub processed_text: String,
    /// Per-provider result/error and metadata.
    pub providers: HashMap<ProviderId, ProviderHistory>,
}

impl HistoryEntry {
    /// Create a history entry with no provider outcomes yet.
    pub fn new(
        id: HistoryEntryId,
        time: std::time::SystemTime,
        context: ContextId,
        original_text: impl Into<String>,
        processed_text: impl Into<String>,
    ) -> Self {
        Self {
            id,
            time,
            context,
            original_text: original_text.into(),
            processed_text: processed_text.into(),
            providers: HashMap::new(),
        }
    }
}

/// Bounded, in-memory history. Persistence and navigation are separate concerns.
#[derive(Clone, Debug)]
pub struct History {
    entries: std::collections::VecDeque<HistoryEntry>,
    capacity: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_HISTORY_CAPACITY)
    }
}

impl History {
    /// Create a history with a configured maximum number of entries.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: std::collections::VecDeque::new(),
            capacity,
        }
    }
    /// Insert an entry, evicting oldest entries as needed; returns its ID.
    pub fn push(&mut self, entry: HistoryEntry) -> HistoryEntryId {
        let id = entry.id;
        if self.capacity == 0 {
            return id;
        }
        while self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
        id
    }
    /// Find an entry by its typed ID.
    pub fn get(&self, id: &HistoryEntryId) -> Option<&HistoryEntry> {
        self.entries.iter().find(|entry| &entry.id == id)
    }
    /// Number of retained entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    /// Whether no entries are retained.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// Maximum number of retained entries.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    /// Iterate over entries from oldest to newest.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &HistoryEntry> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_stores_provider_metadata_and_evicts_oldest_entry() {
        let context = ContextId::new("api:game").unwrap();
        let mut history = History::with_capacity(2);
        let first_id = HistoryEntryId::new();
        let first = HistoryEntry::new(
            first_id,
            std::time::SystemTime::UNIX_EPOCH,
            context.clone(),
            "original",
            "processed",
        );
        let second_id = HistoryEntryId::new();
        let mut second = HistoryEntry::new(
            second_id,
            std::time::SystemTime::UNIX_EPOCH,
            context.clone(),
            "two",
            "two",
        );
        second.providers.insert(
            ProviderId::new("provider-a").unwrap(),
            ProviderHistory {
                outcome: ProviderOutcome::Result("translated".into()),
                status: ProviderStatus::Succeeded,
                elapsed: std::time::Duration::from_millis(23),
                cached: true,
            },
        );
        history.push(first);
        history.push(second);
        let third_id = HistoryEntryId::new();
        history.push(HistoryEntry::new(
            third_id,
            std::time::SystemTime::UNIX_EPOCH,
            context,
            "three",
            "three",
        ));

        assert_eq!(history.capacity(), 2);
        assert_eq!(history.len(), 2);
        assert!(history.get(&first_id).is_none());
        assert!(history.get(&second_id).is_some());
        let stored = history.get(&third_id).unwrap();
        assert_eq!(stored.original_text, "three");
        let earlier = history.get(&second_id).unwrap();
        assert_eq!(
            earlier.providers[&ProviderId::new("provider-a").unwrap()].outcome,
            ProviderOutcome::Result("translated".into())
        );
        assert_eq!(
            earlier.providers[&ProviderId::new("provider-a").unwrap()].status,
            ProviderStatus::Succeeded
        );
        assert_eq!(
            earlier.providers[&ProviderId::new("provider-a").unwrap()].elapsed,
            std::time::Duration::from_millis(23)
        );
        assert!(earlier.providers[&ProviderId::new("provider-a").unwrap()].cached);
    }

    #[test]
    fn default_history_capacity_is_ten_thousand() {
        assert_eq!(History::default().capacity(), DEFAULT_HISTORY_CAPACITY);
        assert_eq!(DEFAULT_HISTORY_CAPACITY, 10_000);
    }

    #[test]
    fn context_ids_validate_known_and_namespaced_sources() {
        for value in ["clipboard", "typed", "api:client", "plugin:mod/source"] {
            assert_eq!(ContextId::new(value).unwrap().as_str(), value);
        }
        assert!(ContextId::new("").is_err());
        assert!(ContextId::new("x".repeat(129)).is_err());
        assert!(ContextId::new("api:bad\nsource").is_err());
        assert!(ContextId::new("api:").is_err());
        assert!(ContextId::new("plugin:id/").is_err());
        assert!(ContextId::new("other").is_err());
        assert!(ContextId::new(format!("api:{}", "あ".repeat(124))).is_ok());
    }

    #[test]
    fn registry_creates_contexts_with_kind_specific_idle_defaults() {
        let mut registry = ContextRegistry::default();
        let template = ContextOptions { idle_delay_ms: 777 };
        for (id, idle) in [
            ("clipboard", 0),
            ("typed", 0),
            ("api:game", 250),
            ("plugin:x/src", 250),
        ] {
            let context = registry.get_or_create(ContextId::new(id).unwrap(), &template);
            assert_eq!(context.options().idle_delay_ms, idle);
            assert_eq!(
                registry
                    .get_or_create(ContextId::new(id).unwrap(), &template)
                    .options()
                    .idle_delay_ms,
                idle
            );
        }
        assert_eq!(registry.len(), 4);
    }

    #[test]
    fn unknown_context_copies_template_options() {
        let mut registry = ContextRegistry::default();
        let template = ContextOptions { idle_delay_ms: 42 };
        let context = registry.get_or_create(ContextId::new("api:custom").unwrap(), &template);
        assert_eq!(context.options().idle_delay_ms, 250);
    }

    #[test]
    fn stores_normalized_input_and_detects_japanese() {
        let input = TextInput::new("  こんにちは\r\n世界  ").unwrap();
        assert_eq!(input.as_str(), "  こんにちは\n世界  ");
        assert!(input.contains_japanese());
    }

    #[test]
    fn rejects_blank_input_but_preserves_nonblank_whitespace() {
        assert_eq!(TextInput::new(" \t\r\n"), Err(TextInputError::Empty));
        assert_eq!(TextInput::new(" x ").unwrap().as_str(), " x ");
    }

    #[test]
    fn enforces_default_limit_by_unicode_scalar_values() {
        let accepted = "あ".repeat(DEFAULT_MAX_INPUT_CHARS);
        let oversized = "あ".repeat(DEFAULT_MAX_INPUT_CHARS + 1);
        assert!(TextInput::new(&accepted).is_ok());
        assert!(matches!(
            TextInput::new(&oversized),
            Err(TextInputError::TooLarge {
                actual_chars: 20_001,
                max_chars: 20_000
            })
        ));
    }

    #[test]
    fn accepts_configured_limit_and_counts_multibyte_scalar_values() {
        assert!(TextInput::with_max_chars("漢字", 2).is_ok());
        assert_eq!(
            TextInput::with_max_chars("漢字語", 2),
            Err(TextInputError::TooLarge {
                actual_chars: 3,
                max_chars: 2
            })
        );
    }

    #[test]
    fn nul_only_is_empty_after_normalization() {
        assert_eq!(TextInput::new("\0\0"), Err(TextInputError::Empty));
    }
}
