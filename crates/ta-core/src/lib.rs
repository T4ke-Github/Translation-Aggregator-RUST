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

/// Default UTF-8 byte limit for text retained in each context.
pub const DEFAULT_CONTEXT_TEXT_HISTORY_BYTES: usize = 64 * 1024;

/// Per-context text history bounded independently by UTF-8 byte size.
#[derive(Clone, Debug)]
pub struct ContextTextHistory {
    contexts: HashMap<ContextId, (std::collections::VecDeque<String>, usize)>,
    max_bytes: usize,
}

impl Default for ContextTextHistory {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_CONTEXT_TEXT_HISTORY_BYTES)
    }
}

impl ContextTextHistory {
    /// Create per-context histories with the specified byte limit.
    pub fn with_capacity(max_bytes: usize) -> Self {
        Self {
            contexts: HashMap::new(),
            max_bytes,
        }
    }

    /// Add text, evicting oldest whole entries until it fits. Oversized text is ignored.
    pub fn push(&mut self, context: ContextId, text: impl Into<String>) {
        let text = text.into();
        let size = text.len();
        if self.max_bytes == 0 || size == 0 || size > self.max_bytes {
            return;
        }
        let (entries, bytes) = self.contexts.entry(context).or_default();
        while *bytes > self.max_bytes - size {
            if let Some(oldest) = entries.pop_front() {
                *bytes -= oldest.len();
            }
        }
        entries.push_back(text);
        *bytes += size;
    }

    /// Remove all retained text for one context.
    pub fn clear(&mut self, context: &ContextId) {
        self.contexts.remove(context);
    }

    /// Retained text in insertion order, or an empty vector for an untracked context.
    pub fn entries(&self, context: &ContextId) -> Vec<&str> {
        self.contexts
            .get(context)
            .map_or_else(Vec::new, |(entries, _)| {
                entries.iter().map(String::as_str).collect()
            })
    }

    /// Total retained UTF-8 bytes for one context.
    pub fn bytes(&self, context: &ContextId) -> usize {
        self.contexts.get(context).map_or(0, |(_, bytes)| *bytes)
    }

    /// Number of contexts currently retaining entries.
    pub fn context_count(&self) -> usize {
        self.contexts.len()
    }
}

static NEXT_HISTORY_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Opaque, typed identity for a history entry.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HistoryEntryId(u64);

impl HistoryEntryId {
    /// Allocate a process-unique history entry identifier.
    pub fn new() -> Self {
        use std::sync::atomic::Ordering;
        Self(NEXT_HISTORY_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for HistoryEntryId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for HistoryEntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
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
    /// The request was superseded before translation.
    Skipped,
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
    /// The request was skipped without a provider failure.
    Skipped,
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

/// Encode the retained history as a versioned JSON snapshot. This contains history only.
pub fn encode_history_snapshot(history: &History) -> Result<String, HistorySnapshotError> {
    let entries = history
        .entries
        .iter()
        .map(|entry| {
            let elapsed_since_epoch = entry
                .time
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map_err(|_| HistorySnapshotError::InvalidData)?;
            let providers = entry
                .providers
                .iter()
                .map(|(id, value)| {
                    Ok((
                        id.as_str(),
                        SnapshotProvider {
                            outcome: match &value.outcome {
                                ProviderOutcome::Result(text) => SnapshotOutcome::Result(text),
                                ProviderOutcome::Error(text) => SnapshotOutcome::Error(text),
                                ProviderOutcome::Skipped => SnapshotOutcome::Skipped,
                            },
                            status: value.status,
                            elapsed_nanos: u64::try_from(value.elapsed.as_nanos())
                                .map_err(|_| HistorySnapshotError::InvalidData)?,
                            cached: value.cached,
                        },
                    ))
                })
                .collect::<Result<HashMap<_, _>, HistorySnapshotError>>()?;
            Ok(SnapshotEntry {
                id: entry.id.0,
                time_secs: elapsed_since_epoch.as_secs(),
                time_nanos: elapsed_since_epoch.subsec_nanos(),
                context: entry.context.as_str(),
                original_text: &entry.original_text,
                processed_text: &entry.processed_text,
                providers,
            })
        })
        .collect::<Result<Vec<_>, HistorySnapshotError>>()?;
    serde_json::to_string(&Snapshot {
        version: 1,
        entries,
    })
    .map_err(|_| HistorySnapshotError::InvalidData)
}

/// Decode a versioned JSON snapshot, retaining the newest entries up to `capacity`.
pub fn decode_history_snapshot(
    json: &str,
    capacity: usize,
) -> Result<History, HistorySnapshotError> {
    let snapshot: SnapshotOwned =
        serde_json::from_str(json).map_err(|_| HistorySnapshotError::InvalidData)?;
    if snapshot.version != 1 {
        return Err(HistorySnapshotError::UnsupportedVersion);
    }
    let mut history = History::with_capacity(capacity);
    let mut seen = std::collections::HashSet::new();
    let mut highest_id = 0_u64;
    for item in snapshot.entries {
        if item.id == 0
            || item.id == u64::MAX
            || !seen.insert(item.id)
            || item.time_nanos >= 1_000_000_000
        {
            return Err(HistorySnapshotError::InvalidData);
        }
        highest_id = highest_id.max(item.id);
        let id = HistoryEntryId(item.id);
        let context =
            ContextId::new(item.context).map_err(|_| HistorySnapshotError::InvalidData)?;
        let time = std::time::SystemTime::UNIX_EPOCH
            .checked_add(std::time::Duration::new(item.time_secs, item.time_nanos))
            .ok_or(HistorySnapshotError::InvalidData)?;
        let mut entry =
            HistoryEntry::new(id, time, context, item.original_text, item.processed_text);
        for (provider, value) in item.providers {
            let provider =
                ProviderId::new(provider).map_err(|_| HistorySnapshotError::InvalidData)?;
            let outcome = match value.outcome {
                SnapshotOutcomeOwned::Result(s) => ProviderOutcome::Result(s),
                SnapshotOutcomeOwned::Error(s) => ProviderOutcome::Error(s),
                SnapshotOutcomeOwned::Skipped => ProviderOutcome::Skipped,
            };
            entry.providers.insert(
                provider,
                ProviderHistory {
                    outcome,
                    status: value.status,
                    elapsed: std::time::Duration::from_nanos(value.elapsed_nanos),
                    cached: value.cached,
                },
            );
        }
        history.push(entry);
    }
    NEXT_HISTORY_ID.fetch_max(highest_id + 1, std::sync::atomic::Ordering::Relaxed);
    Ok(history)
}

/// Snapshot decoding/encoding failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistorySnapshotError {
    /// Unsupported snapshot format version.
    UnsupportedVersion,
    /// Malformed or invalid snapshot data.
    InvalidData,
}

#[derive(serde::Serialize)]
struct Snapshot<'a> {
    version: u32,
    entries: Vec<SnapshotEntry<'a>>,
}
#[derive(serde::Serialize)]
struct SnapshotEntry<'a> {
    id: u64,
    time_secs: u64,
    time_nanos: u32,
    context: &'a str,
    original_text: &'a str,
    processed_text: &'a str,
    providers: HashMap<&'a str, SnapshotProvider<'a>>,
}
#[derive(serde::Serialize)]
struct SnapshotProvider<'a> {
    outcome: SnapshotOutcome<'a>,
    status: ProviderStatus,
    elapsed_nanos: u64,
    cached: bool,
}
#[derive(serde::Serialize)]
#[serde(tag = "kind", content = "text")]
enum SnapshotOutcome<'a> {
    Result(&'a str),
    Error(&'a str),
    Skipped,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotOwned {
    version: u32,
    entries: Vec<SnapshotEntryOwned>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotEntryOwned {
    id: u64,
    time_secs: u64,
    time_nanos: u32,
    context: String,
    original_text: String,
    processed_text: String,
    providers: HashMap<String, SnapshotProviderOwned>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotProviderOwned {
    outcome: SnapshotOutcomeOwned,
    status: ProviderStatus,
    elapsed_nanos: u64,
    cached: bool,
}
#[derive(serde::Deserialize)]
#[serde(tag = "kind", content = "text")]
enum SnapshotOutcomeOwned {
    Result(String),
    Error(String),
    Skipped,
}

impl serde::Serialize for ProviderStatus {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self {
            Self::Pending => "pending",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        })
    }
}
impl<'de> serde::Deserialize<'de> for ProviderStatus {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match <String as serde::Deserialize>::deserialize(deserializer)?.as_str() {
            "pending" => Ok(Self::Pending),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "skipped" => Ok(Self::Skipped),
            _ => Err(serde::de::Error::custom("invalid provider status")),
        }
    }
}

impl Default for History {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_HISTORY_CAPACITY)
    }
}

/// Deterministic in-memory history search criteria. Text matches original, processed,
/// and provider result/error strings using Unicode lowercase (not full Unicode case folding).
pub struct HistoryQuery<'a> {
    /// Optional substring search; matching is case-insensitive for lowercase mappings.
    pub search: Option<&'a str>,
    /// Restrict to this validated context.
    pub context: Option<&'a ContextId>,
    /// Restrict to entries containing this provider result.
    pub provider: Option<&'a ProviderId>,
    /// Inclusive lower time bound.
    pub from: Option<std::time::SystemTime>,
    /// Inclusive upper time bound.
    pub through: Option<std::time::SystemTime>,
    /// Number of matching entries to skip after sorting.
    pub offset: usize,
    /// Maximum entries to return.
    pub limit: usize,
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
    /// Search retained entries newest-first; entries with equal timestamps use ID order.
    /// Search is substring-based across original/processed text and every provider outcome.
    pub fn search(&self, query: &HistoryQuery<'_>) -> Vec<&HistoryEntry> {
        let needle = query.search.map(str::to_lowercase);
        let mut matches: Vec<&HistoryEntry> = self
            .entries
            .iter()
            .filter(|entry| {
                if query
                    .context
                    .is_some_and(|context| &entry.context != context)
                    || query
                        .provider
                        .is_some_and(|provider| !entry.providers.contains_key(provider))
                    || query.from.is_some_and(|from| entry.time < from)
                    || query.through.is_some_and(|through| entry.time > through)
                {
                    return false;
                }
                let Some(needle) = needle.as_deref() else {
                    return true;
                };
                let text_matches = |text: &str| text.to_lowercase().contains(needle);
                text_matches(&entry.original_text)
                    || text_matches(&entry.processed_text)
                    || entry
                        .providers
                        .values()
                        .any(|result| match &result.outcome {
                            ProviderOutcome::Result(text) | ProviderOutcome::Error(text) => {
                                text_matches(text)
                            }
                            ProviderOutcome::Skipped => false,
                        })
            })
            .collect();
        matches.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| b.id.0.cmp(&a.id.0)));
        matches
            .into_iter()
            .skip(query.offset)
            .take(query.limit)
            .collect()
    }
    /// Find an entry by its typed ID.
    pub fn get(&self, id: &HistoryEntryId) -> Option<&HistoryEntry> {
        self.entries.iter().find(|entry| &entry.id == id)
    }
    /// Store a provider completion if its entry is still retained.
    pub fn record_provider(
        &mut self,
        id: &HistoryEntryId,
        provider: ProviderId,
        result: ProviderHistory,
    ) {
        if let Some(entry) = self.entries.iter_mut().rev().find(|entry| &entry.id == id) {
            entry.providers.insert(provider, result);
        }
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
    fn history_snapshot_restores_unique_ids_for_new_entries() {
        let high = 1_000_000_000_u64;
        let mut history = History::with_capacity(2);
        history.push(HistoryEntry::new(
            HistoryEntryId(high),
            std::time::UNIX_EPOCH,
            ContextId::new("typed").unwrap(),
            "old",
            "old",
        ));
        let json = encode_history_snapshot(&history).unwrap();
        decode_history_snapshot(&json, 2).unwrap();
        assert!(HistoryEntryId::new().0 > high);

        let mut duplicate: serde_json::Value = serde_json::from_str(&json).unwrap();
        let same = duplicate["entries"][0].clone();
        duplicate["entries"].as_array_mut().unwrap().push(same);
        assert!(matches!(
            decode_history_snapshot(&duplicate.to_string(), 2),
            Err(HistorySnapshotError::InvalidData)
        ));
        let invalid = json.replace(&format!("\"id\":{high}"), "\"id\":18446744073709551615");
        assert!(matches!(
            decode_history_snapshot(&invalid, 2),
            Err(HistorySnapshotError::InvalidData)
        ));
    }

    #[test]
    fn history_snapshot_roundtrips_every_history_field_and_excludes_settings() {
        let mut history = History::with_capacity(3);
        let mut entry = HistoryEntry::new(
            HistoryEntryId(42),
            std::time::UNIX_EPOCH + std::time::Duration::new(12, 34),
            ContextId::new("api:game").unwrap(),
            "original",
            "processed",
        );
        entry.providers.insert(
            ProviderId::new("provider-a").unwrap(),
            ProviderHistory {
                outcome: ProviderOutcome::Error("failed".into()),
                status: ProviderStatus::Failed,
                elapsed: std::time::Duration::from_millis(19),
                cached: true,
            },
        );
        entry.providers.insert(
            ProviderId::new("provider-b").unwrap(),
            ProviderHistory {
                outcome: ProviderOutcome::Result("translated".into()),
                status: ProviderStatus::Succeeded,
                elapsed: std::time::Duration::from_nanos(7),
                cached: false,
            },
        );
        history.push(entry.clone());
        let json = encode_history_snapshot(&history).unwrap();
        assert!(!json.contains("credential"));
        assert!(!json.contains("settings"));
        let restored = decode_history_snapshot(&json, 3).unwrap();
        assert_eq!(restored.iter().next(), Some(&entry));
    }

    #[test]
    fn history_snapshot_enforces_capacity_and_rejects_invalid_payloads() {
        let mut history = History::with_capacity(3);
        for id in 1..=3 {
            history.push(HistoryEntry::new(
                HistoryEntryId(id),
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(id),
                ContextId::new("typed").unwrap(),
                id.to_string(),
                "p",
            ));
        }
        let json = encode_history_snapshot(&history).unwrap();
        let loaded = decode_history_snapshot(&json, 2).unwrap();
        assert_eq!(
            loaded.iter().map(|e| e.id.0).collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert!(matches!(
            decode_history_snapshot("{", 2),
            Err(HistorySnapshotError::InvalidData)
        ));
        assert!(matches!(
            decode_history_snapshot(r#"{"version":2,"entries":[]}"#, 2),
            Err(HistorySnapshotError::UnsupportedVersion)
        ));
        let invalid_id = json.replace("\"id\":1", "\"id\":0");
        assert!(matches!(
            decode_history_snapshot(&invalid_id, 2),
            Err(HistorySnapshotError::InvalidData)
        ));
    }

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
    fn history_search_matches_original_processed_provider_and_filters_pages() {
        let mut h = History::default();
        let ctx = ContextId::new("api:game").unwrap();
        let pid = ProviderId::new("web-a").unwrap();
        let t = std::time::SystemTime::UNIX_EPOCH;
        let mut a = HistoryEntry::new(HistoryEntryId::new(), t, ctx.clone(), "Fox", "trimmed");
        a.providers.insert(
            pid.clone(),
            ProviderHistory {
                outcome: ProviderOutcome::Result("Renard".into()),
                status: ProviderStatus::Succeeded,
                elapsed: std::time::Duration::ZERO,
                cached: false,
            },
        );
        h.push(a);
        let mut b = HistoryEntry::new(
            HistoryEntryId::new(),
            t + std::time::Duration::from_secs(1),
            ctx.clone(),
            "other",
            "fox processed",
        );
        b.providers.insert(
            pid.clone(),
            ProviderHistory {
                outcome: ProviderOutcome::Error("FOX failed".into()),
                status: ProviderStatus::Failed,
                elapsed: std::time::Duration::ZERO,
                cached: false,
            },
        );
        h.push(b);
        h.push(HistoryEntry::new(
            HistoryEntryId::new(),
            t + std::time::Duration::from_secs(2),
            ContextId::new("typed").unwrap(),
            "ignore",
            "none",
        ));
        let q = HistoryQuery {
            search: Some("fOx"),
            context: Some(&ctx),
            provider: Some(&pid),
            from: None,
            through: None,
            offset: 0,
            limit: 10,
        };
        let results = h.search(&q);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].original_text, "other");
        let page = h.search(&HistoryQuery {
            offset: 1,
            limit: 1,
            ..q
        });
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].original_text, "Fox");
    }

    #[test]
    fn default_history_capacity_is_ten_thousand() {
        assert_eq!(History::default().capacity(), DEFAULT_HISTORY_CAPACITY);
        assert_eq!(DEFAULT_HISTORY_CAPACITY, 10_000);
    }

    #[test]
    fn text_history_counts_utf8_bytes_evicts_whole_entries_and_clears_in_isolation() {
        let a = ContextId::new("api:a").unwrap();
        let b = ContextId::new("api:b").unwrap();
        let mut h = ContextTextHistory::with_capacity(5);
        h.push(a.clone(), "éé");
        assert_eq!(h.bytes(&a), 4);
        h.push(a.clone(), "xy");
        h.push(b.clone(), "kept");
        assert_eq!(h.entries(&a), &["xy"]);
        assert_eq!(h.bytes(&a), 2);
        assert_eq!(h.bytes(&b), 4);
        h.clear(&a);
        assert!(h.entries(&a).is_empty());
        assert_eq!(h.entries(&b), &["kept"]);
    }

    #[test]
    fn text_history_discards_oversized_entries_and_handles_repeated_pushes() {
        let context = ContextId::new("api:a").unwrap();
        let mut h = ContextTextHistory::with_capacity(3);
        h.push(context.clone(), "large");
        assert!(h.entries(&context).is_empty());
        for _ in 0..20 {
            h.push(context.clone(), "ab");
            h.push(context.clone(), "");
        }
        assert_eq!(h.entries(&context), &["ab"]);
        assert_eq!(h.bytes(&context), 2);
        assert_eq!(h.context_count(), 1);
        h.clear(&context);
        assert_eq!(h.context_count(), 0);
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
