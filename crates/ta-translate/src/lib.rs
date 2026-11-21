//! Translator capability metadata and request eligibility policy.

#![forbid(unsafe_code)]

/// Error returned when sentence-aware splitting cannot honor the chunk limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SentenceSplitError {
    /// Number of Unicode scalar values in the indivisible sentence.
    pub actual_chars: usize,
    /// Maximum number of scalar values permitted in each chunk.
    pub max_chars: usize,
}

impl std::fmt::Display for SentenceSplitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "sentence has {} characters, limit {}",
            self.actual_chars, self.max_chars
        )
    }
}

impl std::error::Error for SentenceSplitError {}

/// Split text at sentence-ending punctuation or line breaks without changing its contents.
/// Lengths count Unicode scalar values, not bytes or UTF-16 code units.
pub fn split_sentences(text: &str, max_chars: usize) -> Result<Vec<String>, SentenceSplitError> {
    if max_chars == 0 {
        return Err(SentenceSplitError {
            actual_chars: text.chars().count(),
            max_chars,
        });
    }
    if text.is_empty() {
        return Ok(Vec::new());
    }

    let mut chunks = Vec::new();
    let mut chunk_start = 0;
    let mut chunk_chars = 0;
    let mut sentence_start = 0;
    let mut sentence_chars = 0;

    for (index, ch) in text.char_indices() {
        sentence_chars += 1;
        let sentence_end = index + ch.len_utf8();
        let boundary = matches!(ch, '。' | '！' | '？' | '.' | '!' | '?' | '\n' | '\r');
        if boundary {
            if sentence_chars > max_chars {
                return Err(SentenceSplitError {
                    actual_chars: sentence_chars,
                    max_chars,
                });
            }
            if chunk_chars + sentence_chars > max_chars {
                chunks.push(text[chunk_start..sentence_start].to_owned());
                chunk_start = sentence_start;
                chunk_chars = 0;
            }
            chunk_chars += sentence_chars;
            sentence_start = sentence_end;
            sentence_chars = 0;
        }
    }
    if sentence_chars > 0 {
        if sentence_chars > max_chars {
            return Err(SentenceSplitError {
                actual_chars: sentence_chars,
                max_chars,
            });
        }
        if chunk_chars + sentence_chars > max_chars {
            chunks.push(text[chunk_start..sentence_start].to_owned());
            chunk_start = sentence_start;
        }
    }
    if chunk_start < text.len() {
        chunks.push(text[chunk_start..].to_owned());
    }
    Ok(chunks)
}

/// Default maximum input length for automatic requests, in Unicode scalar values.
pub const DEFAULT_AUTO_REQUEST_MAX_CHARS: usize = 500;

/// Whether a translator is a built-in local tool or a remote service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslatorKind {
    /// A local translator bundled with the application.
    BuiltInLocal,
    /// A translator that sends requests to a remote service.
    Remote,
}

/// How a translator's use is priced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CostHint {
    /// No usage fee.
    Free,
    /// Usage-based fee.
    Metered,
    /// Paid access is required.
    Paid,
}

/// Capability metadata used by routing and UI to describe a translator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslatorCapabilities {
    /// Stable translator identifier.
    pub id: String,
    /// Human-readable translator name.
    pub name: String,
    /// Local or remote implementation category.
    pub kind: TranslatorKind,
    /// Whether this translator can detect the source language.
    pub detects_source_language: bool,
    /// Whether this translator delivers incremental results.
    pub streaming: bool,
    /// Whether text leaves this machine.
    pub remote: bool,
    /// Indicative pricing category.
    pub cost_hint: CostHint,
    /// Initial automatic-translation preference.
    pub auto_translate_enabled_by_default: bool,
    /// Optional translator-specific scalar-value length limit.
    pub max_chars: Option<usize>,
    /// Supported source language tags; `None` means unrestricted.
    pub supported_source_languages: Option<Vec<String>>,
    /// Supported target language tags; `None` means unrestricted.
    pub supported_target_languages: Option<Vec<String>>,
}

impl TranslatorCapabilities {
    /// Create capabilities with safe default automatic-request behavior.
    pub fn new(kind: TranslatorKind) -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            kind,
            detects_source_language: false,
            streaming: false,
            remote: kind == TranslatorKind::Remote,
            cost_hint: CostHint::Free,
            auto_translate_enabled_by_default: kind == TranslatorKind::BuiltInLocal,
            max_chars: None,
            supported_source_languages: None,
            supported_target_languages: None,
        }
    }

    /// Whether the translator can serve this language pair. `None` means unrestricted.
    pub fn supports_language_pair(&self, source: &str, target: &str) -> bool {
        language_allowed(&self.supported_source_languages, source)
            && language_allowed(&self.supported_target_languages, target)
    }
}

fn language_allowed(allowed: &Option<Vec<String>>, language: &str) -> bool {
    allowed
        .as_ref()
        .is_none_or(|items| items.iter().any(|item| item == language))
}

/// Why an automatic request was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutoRequestRejection {
    /// Automatic translation is disabled for this request.
    Disabled,
    /// The source or target language is unsupported.
    UnsupportedLanguagePair,
    /// Input exceeds the effective scalar-value length cap.
    TooLong {
        /// Number of Unicode scalar values in the input.
        actual_chars: usize,
        /// Effective maximum accepted number of scalar values.
        max_chars: usize,
    },
}

/// Why a manually requested translation cannot be sent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestRejection {
    /// The source or target language is unsupported.
    UnsupportedLanguagePair,
    /// Input exceeds this translator's maximum request length.
    TooLong {
        /// Number of Unicode scalar values in the input.
        actual_chars: usize,
        /// Maximum accepted by this translator.
        max_chars: usize,
    },
}

/// Whether a request was initiated automatically or explicitly by the user.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestMode {
    /// A request initiated by automatic translation.
    Automatic,
    /// An explicit request for one translator.
    Manual,
    /// An explicit request to use every eligible translator.
    TranslateAll,
}

/// Why a translator was excluded by routing policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutingRejection {
    /// The translator is switched off.
    ProviderDisabled,
    /// Remote sending has not been permitted.
    RemotePermissionRequired,
    /// A remote credential is unavailable.
    MissingCredentials,
    /// The request violates a provider capability.
    Provider(RequestRejection),
    /// Automatic translation is switched off for the translator.
    AutoTranslateDisabled,
}

/// Inputs needed to decide if one translator can receive this request.
pub struct RoutingRequest<'a> {
    /// Translator capability metadata.
    pub capabilities: &'a TranslatorCapabilities,
    /// Whether this translator is switched on.
    pub provider_enabled: bool,
    /// Whether required credentials are available.
    pub credentials_available: bool,
    /// Whether sending text off-device is allowed.
    pub remote_permission: bool,
    /// Whether automatic translation is switched on.
    pub auto_translate_enabled: bool,
    /// Requested source language.
    pub source: &'a str,
    /// Requested target language.
    pub target: &'a str,
    /// Text to translate.
    pub text: &'a str,
    /// How the request was started.
    pub mode: RequestMode,
}

/// Check routing switches independently from provider capabilities.
pub fn routing_request_rejection(request: RoutingRequest<'_>) -> Option<RoutingRejection> {
    let RoutingRequest {
        capabilities,
        provider_enabled,
        credentials_available,
        remote_permission,
        auto_translate_enabled,
        source,
        target,
        text,
        mode,
    } = request;
    if !provider_enabled {
        return Some(RoutingRejection::ProviderDisabled);
    }
    if capabilities.remote && !remote_permission {
        return Some(RoutingRejection::RemotePermissionRequired);
    }
    if capabilities.remote && !credentials_available {
        return Some(RoutingRejection::MissingCredentials);
    }
    if mode == RequestMode::Automatic && !auto_translate_enabled {
        return Some(RoutingRejection::AutoTranslateDisabled);
    }
    request_rejection(capabilities, source, target, text).map(RoutingRejection::Provider)
}

/// Check provider constraints shared by manual and automatic requests.
pub fn request_rejection(
    capabilities: &TranslatorCapabilities,
    source: &str,
    target: &str,
    text: &str,
) -> Option<RequestRejection> {
    if !capabilities.supports_language_pair(source, target) {
        return Some(RequestRejection::UnsupportedLanguagePair);
    }
    let actual_chars = text.chars().count();
    capabilities
        .max_chars
        .filter(|max| actual_chars > *max)
        .map(|max_chars| RequestRejection::TooLong {
            actual_chars,
            max_chars,
        })
}

/// Test whether text may be sent automatically under the configured cap.
/// Length is measured as Unicode scalar values, not bytes or grapheme clusters.
pub fn auto_request_rejection(
    capabilities: &TranslatorCapabilities,
    enabled: bool,
    source: &str,
    target: &str,
    text: &str,
    global_max_chars: usize,
) -> Option<AutoRequestRejection> {
    if !enabled {
        return Some(AutoRequestRejection::Disabled);
    }
    if let Some(rejection) = request_rejection(capabilities, source, target, text) {
        return Some(match rejection {
            RequestRejection::UnsupportedLanguagePair => {
                AutoRequestRejection::UnsupportedLanguagePair
            }
            RequestRejection::TooLong {
                actual_chars,
                max_chars,
            } => AutoRequestRejection::TooLong {
                actual_chars,
                max_chars,
            },
        });
    }
    // Built-in local tools are exempt from the global auto-send cap. A provider cap
    // replaces rather than tightens against the global cap.
    if capabilities.kind == TranslatorKind::BuiltInLocal {
        return None;
    }
    let actual_chars = text.chars().count();
    let max_chars = capabilities.max_chars.unwrap_or(global_max_chars);
    (actual_chars > max_chars).then_some(AutoRequestRejection::TooLong {
        actual_chars,
        max_chars,
    })
}

/// Default global automatic-request cap.
pub const fn default_auto_request_max_chars() -> usize {
    DEFAULT_AUTO_REQUEST_MAX_CHARS
}

/// Maximum number of successful translation results retained in memory.
pub const DEFAULT_RESULT_CACHE_CAPACITY: usize = 10_000;

/// Identity of a translated request, including caller-provided settings fingerprint.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResultCacheKey {
    /// Stable provider identifier.
    pub provider_id: String,
    /// Text after request preprocessing.
    pub processed_text: String,
    /// Requested source language.
    pub source: String,
    /// Requested target language.
    pub target: String,
    /// Fingerprint covering all settings that affect provider output.
    pub settings_fingerprint: String,
}

/// Bounded in-memory least-recently-used store for complete successful outputs.
#[derive(Debug)]
pub struct ResultCache {
    capacity: usize,
    entries: Vec<(ResultCacheKey, String)>,
}

impl ResultCache {
    /// Create an empty cache with the default maximum size.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_RESULT_CACHE_CAPACITY)
    }

    /// Create an empty cache with a chosen maximum number of entries.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Vec::new(),
        }
    }

    /// Find a result and mark it most recently used, unless this provider forbids caching.
    pub fn get(&mut self, key: &ResultCacheKey, cache_allowed: bool) -> Option<&str> {
        if !cache_allowed {
            return None;
        }
        let index = self.entries.iter().position(|(entry, _)| entry == key)?;
        let item = self.entries.remove(index);
        self.entries.push(item);
        self.entries.last().map(|(_, value)| value.as_str())
    }

    /// Store a complete successful result; opting-out providers are never retained.
    /// Call only after a request has completed successfully (never for partial output).
    pub fn insert_success(&mut self, key: ResultCacheKey, output: String, cache_allowed: bool) {
        if !cache_allowed || self.capacity == 0 {
            return;
        }
        self.entries.retain(|(existing, _)| existing != &key);
        if self.entries.len() == self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, output));
    }

    /// Bypass lookup for a retry, then replace the prior result on success.
    pub fn retry_success(&mut self, key: ResultCacheKey, output: String, cache_allowed: bool) {
        self.entries.retain(|(existing, _)| existing != &key);
        self.insert_success(key, output, cache_allowed);
    }

    /// Number of retained results.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache has no retained results.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for ResultCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Maximum threads used by one dispatcher call, even if a caller asks for more.
pub const MAX_DISPATCH_WORKERS: usize = 8;

/// Selection policy for a one-shot dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchMode {
    /// Run only translators configured for automatic requests.
    Automatic,
    /// Run every enabled and unpaused translator.
    TranslateAll,
}

/// Configured translator and its dispatch-time eligibility.
pub struct DispatchTranslator {
    /// Stable translator identifier.
    pub id: String,
    /// Whether this translator may receive requests.
    pub enabled: bool,
    /// Whether this translator is temporarily paused.
    pub paused: bool,
    /// Whether automatic requests are allowed for this translator.
    pub auto_translate_enabled: bool,
    /// One translation call; errors must be sanitized by the caller.
    pub translate: Box<dyn FnOnce(String) -> Result<String, String> + Send + 'static>,
}

/// Outcome of one dispatched translator call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchStatus {
    /// The translator returned a result.
    Success,
    /// The translator returned a sanitized error.
    Error,
}

/// One translator completion delivered as soon as it finishes.
#[derive(Clone, Debug)]
pub struct DispatchCompletion {
    /// Stable translator identifier.
    pub id: String,
    /// Translation or sanitized error.
    pub result: Result<String, String>,
    /// Time spent in the translator call.
    pub elapsed: std::time::Duration,
    /// Whether this result came from a cache.
    pub cached: bool,
    /// Result status for the consumer.
    pub status: DispatchStatus,
}

/// Start a bounded one-shot fan-out; receive completions as individual calls finish.
/// At most eight threads are created. Dropping the receiver does not cancel calls.
pub fn dispatch_translators(
    translators: Vec<DispatchTranslator>,
    text: String,
    mode: DispatchMode,
    max_workers: usize,
) -> std::sync::mpsc::Receiver<DispatchCompletion> {
    use std::{
        sync::{Arc, Mutex, mpsc},
        time::Instant,
    };
    let (out_tx, out_rx) = mpsc::channel();
    let jobs: Vec<_> = translators
        .into_iter()
        .filter(|translator| {
            translator.enabled
                && !translator.paused
                && (mode == DispatchMode::TranslateAll || translator.auto_translate_enabled)
        })
        .collect();
    if jobs.is_empty() {
        return out_rx;
    }
    let count = max_workers.clamp(1, MAX_DISPATCH_WORKERS).min(jobs.len());
    let (job_tx, job_rx) = mpsc::channel();
    for job in jobs {
        let _ = job_tx.send((job, text.clone()));
    }
    drop(job_tx);
    let shared_rx = Arc::new(Mutex::new(job_rx));
    let mut spawned = 0;
    for _ in 0..count {
        let jobs = Arc::clone(&shared_rx);
        let output = out_tx.clone();
        if std::thread::Builder::new()
            .spawn(move || {
                loop {
                    let job = jobs.lock().expect("dispatch queue poisoned").recv();
                    let Ok((translator, input)) = job else {
                        break;
                    };
                    let start = Instant::now();
                    let id = translator.id;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        (translator.translate)(input)
                    }))
                    .unwrap_or_else(|_| Err("translator failed".into()));
                    let status = if result.is_ok() {
                        DispatchStatus::Success
                    } else {
                        DispatchStatus::Error
                    };
                    let _ = output.send(DispatchCompletion {
                        id,
                        result,
                        elapsed: start.elapsed(),
                        cached: false,
                        status,
                    });
                }
            })
            .is_ok()
        {
            spawned += 1;
        }
    }
    if spawned == 0 {
        let jobs = shared_rx.lock().expect("dispatch queue poisoned");
        while let Ok((translator, _)) = jobs.try_recv() {
            let _ = out_tx.send(DispatchCompletion {
                id: translator.id,
                result: Err("translator worker unavailable".into()),
                elapsed: std::time::Duration::ZERO,
                cached: false,
                status: DispatchStatus::Error,
            });
        }
    }
    drop(out_tx);
    out_rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_reports_panics_and_keeps_later_jobs() {
        use std::time::Duration;
        let panicking = DispatchTranslator {
            id: "broken".into(),
            enabled: true,
            paused: false,
            auto_translate_enabled: true,
            translate: Box::new(|_| panic!("private panic detail")),
        };
        let healthy = DispatchTranslator {
            id: "healthy".into(),
            enabled: true,
            paused: false,
            auto_translate_enabled: true,
            translate: Box::new(Ok),
        };
        let rx = dispatch_translators(
            vec![panicking, healthy],
            "hello".into(),
            DispatchMode::Automatic,
            1,
        );
        let first = rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(first.id, "broken");
        assert_eq!(first.status, DispatchStatus::Error);
        assert!(!format!("{:?}", first).contains("private panic detail"));
        let second = rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(second.id, "healthy");
        assert_eq!(second.result.unwrap(), "hello");
    }

    #[test]
    fn dispatch_delivers_fast_results_before_a_blocked_provider() {
        use std::{sync::mpsc, time::Duration};
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let blocked = DispatchTranslator {
            id: "blocked".into(),
            enabled: true,
            paused: false,
            auto_translate_enabled: true,
            translate: Box::new(move |text| {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(text)
            }),
        };
        let fast = |id: &str| DispatchTranslator {
            id: id.to_owned(),
            enabled: true,
            paused: false,
            auto_translate_enabled: true,
            translate: Box::new(Ok),
        };
        let skipped = DispatchTranslator {
            id: "disabled".into(),
            enabled: false,
            paused: false,
            auto_translate_enabled: true,
            translate: Box::new(|_| panic!("disabled translator was called")),
        };
        let rx = dispatch_translators(
            vec![blocked, fast("one"), fast("two"), skipped],
            "hello".into(),
            DispatchMode::Automatic,
            3,
        );
        started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let first = rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let second = rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let mut ids = [first.id.as_str(), second.id.as_str()];
        ids.sort();
        assert_eq!(ids, ["one", "two"]);
        assert!(first.result.is_ok() && second.result.is_ok());
        assert!(!first.cached && !second.cached);
        release_tx.send(()).unwrap();
        let last = rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(last.id, "blocked");
        assert_eq!(last.result.unwrap(), "hello");
        assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
    }

    #[test]
    fn dispatch_manual_override_keeps_disabled_and_paused_skipped() {
        use std::time::Duration;
        let translator = |id: &str, enabled, paused, auto_translate_enabled| DispatchTranslator {
            id: id.to_owned(),
            enabled,
            paused,
            auto_translate_enabled,
            translate: Box::new(Ok),
        };
        let make = || {
            vec![
                translator("manual", true, false, false),
                translator("auto", true, false, true),
                translator("paused", true, true, true),
                translator("disabled", false, false, true),
            ]
        };
        let automatic = dispatch_translators(make(), "hi".into(), DispatchMode::Automatic, 2);
        assert_eq!(
            automatic.recv_timeout(Duration::from_secs(3)).unwrap().id,
            "auto"
        );
        assert!(automatic.recv_timeout(Duration::from_millis(100)).is_err());
        let manual = dispatch_translators(make(), "hi".into(), DispatchMode::TranslateAll, 2);
        let mut ids = [
            manual.recv_timeout(Duration::from_secs(3)).unwrap().id,
            manual.recv_timeout(Duration::from_secs(3)).unwrap().id,
        ];
        ids.sort();
        assert_eq!(ids, ["auto", "manual"]);
        assert!(manual.recv_timeout(Duration::from_millis(100)).is_err());
        let empty = dispatch_translators(vec![], "hi".into(), DispatchMode::TranslateAll, 4);
        assert!(empty.recv_timeout(Duration::from_millis(100)).is_err());
    }

    #[test]
    fn dispatch_caps_its_worker_threads() {
        use std::sync::{Arc, Condvar, Mutex, mpsc};
        use std::time::Duration;
        let (started_tx, started_rx) = mpsc::channel();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let jobs = (0..12)
            .map(|id| {
                let started_tx = started_tx.clone();
                let gate = Arc::clone(&gate);
                DispatchTranslator {
                    id: id.to_string(),
                    enabled: true,
                    paused: false,
                    auto_translate_enabled: true,
                    translate: Box::new(move |_| {
                        started_tx.send(()).unwrap();
                        let (lock, ready) = &*gate;
                        let mut open = lock.lock().unwrap();
                        while !*open {
                            open = ready.wait(open).unwrap();
                        }
                        Ok("done".into())
                    }),
                }
            })
            .collect();
        let rx = dispatch_translators(jobs, "hi".into(), DispatchMode::Automatic, 1000);
        for _ in 0..MAX_DISPATCH_WORKERS {
            started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        }
        assert!(started_rx.try_recv().is_err());
        let (lock, ready) = &*gate;
        *lock.lock().unwrap() = true;
        ready.notify_all();
        for _ in 0..12 {
            assert_eq!(
                rx.recv_timeout(Duration::from_secs(3)).unwrap().status,
                DispatchStatus::Success
            );
        }
        assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
    }

    #[test]
    fn sentence_splitting_preserves_japanese_and_unicode_scalars() {
        let input = "日本語。😀é!次";
        let chunks = split_sentences(input, 6).unwrap();
        assert_eq!(chunks, ["日本語。", "😀é!次"]);
        assert_eq!(chunks.concat(), input);
        assert!(chunks.iter().all(|chunk| chunk.chars().count() <= 6));
    }

    #[test]
    fn sentence_splitting_uses_line_breaks_and_keeps_whitespace() {
        let input = "first line\n second\r\nthird";
        let chunks = split_sentences(input, 11).unwrap();
        assert_eq!(chunks.concat(), input);
        assert_eq!(chunks, ["first line\n", " second\r\n", "third"]);
    }

    #[test]
    fn sentence_splitting_rejects_indivisible_overlong_sentence_with_counts() {
        assert_eq!(
            split_sentences("😀abcd", 4),
            Err(SentenceSplitError {
                actual_chars: 5,
                max_chars: 4
            })
        );
    }

    #[test]
    fn sentence_splitting_accepts_exact_limit_and_rejects_zero_limit() {
        assert_eq!(split_sentences("abc!", 4).unwrap(), ["abc!"]);
        assert_eq!(
            split_sentences("x", 0),
            Err(SentenceSplitError {
                actual_chars: 1,
                max_chars: 0
            })
        );
        assert_eq!(
            split_sentences("", 0),
            Err(SentenceSplitError {
                actual_chars: 0,
                max_chars: 0
            })
        );
    }

    #[test]
    fn sentence_splitting_never_silently_trims_or_reorders() {
        let input = "  one.\n\ttwo?  ";
        let chunks = split_sentences(input, 7).unwrap();
        assert_eq!(chunks.concat(), input);
        assert_eq!(chunks, ["  one.\n", "\ttwo?  "]);
    }

    fn key(
        provider: &str,
        text: &str,
        source: &str,
        target: &str,
        settings: &str,
    ) -> ResultCacheKey {
        ResultCacheKey {
            provider_id: provider.into(),
            processed_text: text.into(),
            source: source.into(),
            target: target.into(),
            settings_fingerprint: settings.into(),
        }
    }

    #[test]
    fn result_cache_keys_cover_provider_text_languages_and_settings() {
        let mut cache = ResultCache::new();
        let original = key("p", "processed", "ja", "en", "model-a");
        cache.insert_success(original.clone(), "result".into(), true);
        for other in [
            key("q", "processed", "ja", "en", "model-a"),
            key("p", "other", "ja", "en", "model-a"),
            key("p", "processed", "fr", "en", "model-a"),
            key("p", "processed", "ja", "fr", "model-a"),
            key("p", "processed", "ja", "en", "model-b"),
        ] {
            assert_eq!(cache.get(&other, true), None);
        }
        assert_eq!(cache.get(&original, true), Some("result"));
        assert_eq!(cache.get(&original, false), None);
        assert_eq!(cache.get(&original, true), Some("result"));
    }

    #[test]
    fn result_cache_enforces_provider_opt_out_and_zero_capacity() {
        let mut cache = ResultCache::with_capacity(1);
        cache.insert_success(key("p", "x", "ja", "en", "s"), "no".into(), false);
        assert_eq!(cache.len(), 0);
        let mut none = ResultCache::with_capacity(0);
        none.insert_success(key("p", "x", "ja", "en", "s"), "no".into(), true);
        assert!(none.is_empty());
    }

    #[test]
    fn result_cache_evicts_true_lru_entry_and_retry_replaces() {
        let mut cache = ResultCache::with_capacity(2);
        let a = key("p", "a", "ja", "en", "s");
        let b = key("p", "b", "ja", "en", "s");
        let c = key("p", "c", "ja", "en", "s");
        cache.insert_success(a.clone(), "A1".into(), true);
        cache.insert_success(b.clone(), "B".into(), true);
        assert_eq!(cache.get(&a, true), Some("A1"));
        cache.insert_success(c.clone(), "C".into(), true);
        assert_eq!(cache.get(&b, true), None);
        assert_eq!(cache.get(&a, true), Some("A1"));
        cache.retry_success(a.clone(), "A2".into(), true);
        assert_eq!(cache.get(&a, true), Some("A2"));
        cache.retry_success(a.clone(), "ignored".into(), false);
        assert_eq!(cache.get(&a, true), None);
        assert_eq!(cache.get(&c, true), Some("C"));
    }

    #[test]
    fn unsuccessful_or_partial_outputs_are_not_cached_by_policy() {
        let mut cache = ResultCache::new();
        let key = key("p", "x", "ja", "en", "s");
        // Failed and partial requests do not invoke insert_success.
        assert_eq!(cache.get(&key, true), None);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn defaults_only_local_tools_to_automatic_translation() {
        assert!(
            TranslatorCapabilities::new(TranslatorKind::BuiltInLocal)
                .auto_translate_enabled_by_default
        );
        assert!(
            !TranslatorCapabilities::new(TranslatorKind::Remote).auto_translate_enabled_by_default
        );
    }

    #[test]
    fn applies_language_pair_cap_and_unicode_scalar_limit() {
        let mut caps = TranslatorCapabilities::new(TranslatorKind::Remote);
        caps.supported_source_languages = Some(vec!["ja".into()]);
        caps.supported_target_languages = Some(vec!["en".into()]);
        caps.max_chars = Some(3);
        assert_eq!(
            auto_request_rejection(&caps, true, "ja", "en", "漢字😀", 500),
            None
        );
        assert_eq!(
            auto_request_rejection(&caps, true, "ja", "en", "漢字😀x", 500),
            Some(AutoRequestRejection::TooLong {
                actual_chars: 4,
                max_chars: 3
            })
        );
        assert_eq!(
            auto_request_rejection(&caps, true, "fr", "en", "ok", 500),
            Some(AutoRequestRejection::UnsupportedLanguagePair)
        );
    }

    #[test]
    fn global_default_cap_accepts_exact_length_and_rejects_overage() {
        let caps = TranslatorCapabilities::new(TranslatorKind::Remote);
        assert_eq!(default_auto_request_max_chars(), 500);
        assert_eq!(
            auto_request_rejection(&caps, true, "ja", "en", &"あ".repeat(500), 500),
            None
        );
        assert!(matches!(
            auto_request_rejection(&caps, true, "ja", "en", &"あ".repeat(501), 500),
            Some(AutoRequestRejection::TooLong {
                actual_chars: 501,
                max_chars: 500
            })
        ));
    }

    #[test]
    fn provider_cap_overrides_global_and_local_tools_are_exempt() {
        let mut remote = TranslatorCapabilities::new(TranslatorKind::Remote);
        remote.max_chars = Some(700);
        assert_eq!(
            auto_request_rejection(&remote, true, "ja", "en", &"x".repeat(600), 500),
            None
        );
        remote.max_chars = Some(400);
        assert!(matches!(
            auto_request_rejection(&remote, true, "ja", "en", &"x".repeat(450), 500),
            Some(AutoRequestRejection::TooLong { max_chars: 400, .. })
        ));
        let local = TranslatorCapabilities::new(TranslatorKind::BuiltInLocal);
        assert_eq!(
            auto_request_rejection(&local, true, "ja", "en", &"x".repeat(600), 500),
            None
        );
    }

    #[test]
    fn manual_requests_obey_provider_language_and_length_constraints() {
        let mut caps = TranslatorCapabilities::new(TranslatorKind::Remote);
        caps.max_chars = Some(2);
        caps.supported_source_languages = Some(vec!["ja".into()]);
        assert_eq!(request_rejection(&caps, "ja", "en", "漢字"), None);
        assert_eq!(
            request_rejection(&caps, "ja", "en", "漢字x"),
            Some(RequestRejection::TooLong {
                actual_chars: 3,
                max_chars: 2
            })
        );
        assert_eq!(
            request_rejection(&caps, "fr", "en", "ok"),
            Some(RequestRejection::UnsupportedLanguagePair)
        );
    }

    #[test]
    fn disabled_auto_request_is_rejected() {
        let caps = TranslatorCapabilities::new(TranslatorKind::BuiltInLocal);
        assert_eq!(
            auto_request_rejection(&caps, false, "ja", "en", "x", 500),
            Some(AutoRequestRejection::Disabled)
        );
    }

    #[test]
    fn routing_policy_rejects_disabled_provider_and_remote_without_credentials() {
        let remote = TranslatorCapabilities::new(TranslatorKind::Remote);
        assert_eq!(
            routing_request_rejection(RoutingRequest {
                capabilities: &remote,
                provider_enabled: true,
                credentials_available: false,
                remote_permission: true,
                auto_translate_enabled: true,
                source: "ja",
                target: "en",
                text: "x",
                mode: RequestMode::Automatic,
            }),
            Some(RoutingRejection::MissingCredentials)
        );
        assert_eq!(
            routing_request_rejection(RoutingRequest {
                capabilities: &remote,
                provider_enabled: false,
                credentials_available: true,
                remote_permission: true,
                auto_translate_enabled: true,
                source: "ja",
                target: "en",
                text: "x",
                mode: RequestMode::Automatic,
            }),
            Some(RoutingRejection::ProviderDisabled)
        );
    }

    #[test]
    fn translate_all_overrides_auto_switch_but_not_provider_constraints() {
        let mut remote = TranslatorCapabilities::new(TranslatorKind::Remote);
        remote.supported_target_languages = Some(vec!["fr".into()]);
        assert_eq!(
            routing_request_rejection(RoutingRequest {
                capabilities: &remote,
                provider_enabled: true,
                credentials_available: true,
                remote_permission: true,
                auto_translate_enabled: false,
                source: "ja",
                target: "fr",
                text: "x",
                mode: RequestMode::TranslateAll,
            }),
            None
        );
        assert_eq!(
            routing_request_rejection(RoutingRequest {
                capabilities: &remote,
                provider_enabled: true,
                credentials_available: false,
                remote_permission: true,
                auto_translate_enabled: false,
                source: "ja",
                target: "fr",
                text: "x",
                mode: RequestMode::TranslateAll,
            }),
            Some(RoutingRejection::MissingCredentials)
        );
        assert_eq!(
            routing_request_rejection(RoutingRequest {
                capabilities: &remote,
                provider_enabled: true,
                credentials_available: true,
                remote_permission: true,
                auto_translate_enabled: false,
                source: "ja",
                target: "en",
                text: "x",
                mode: RequestMode::TranslateAll,
            }),
            Some(RoutingRejection::Provider(
                RequestRejection::UnsupportedLanguagePair
            ))
        );
    }

    #[test]
    fn remote_permission_is_separate_from_provider_enabled_state() {
        let remote = TranslatorCapabilities::new(TranslatorKind::Remote);
        assert_eq!(
            routing_request_rejection(RoutingRequest {
                capabilities: &remote,
                provider_enabled: true,
                credentials_available: true,
                remote_permission: false,
                auto_translate_enabled: true,
                source: "ja",
                target: "en",
                text: "x",
                mode: RequestMode::Manual,
            }),
            Some(RoutingRejection::RemotePermissionRequired)
        );
    }
}

/// Action produced when submitting a provider request.
#[derive(Debug, PartialEq, Eq)]
pub enum QueueAction<T> {
    /// The request should be started immediately.
    Start(T),
    /// The request is now the sole pending request.
    Queued,
    /// The previous pending request was superseded and should be marked skipped.
    Replaced(T),
}
/// Result classification for a provider request.
#[derive(Debug, PartialEq, Eq)]
pub enum RequestOutcome<T> {
    /// The provider completed with a result.
    Completed(T),
    /// The request was superseded or cancelled without error.
    Skipped,
}
/// One in-flight request and at most one latest-wins pending request.
#[derive(Debug, Default)]
pub struct ProviderQueue<T> {
    in_flight: bool,
    pending: Option<T>,
}
impl<T> ProviderQueue<T> {
    /// Create an empty provider queue.
    pub fn new() -> Self {
        Self {
            in_flight: false,
            pending: None,
        }
    }
    /// Submit a request, returning replaced pending work for skipped reporting.
    pub fn submit(&mut self, request: T) -> QueueAction<T> {
        if !self.in_flight {
            self.in_flight = true;
            QueueAction::Start(request)
        } else if let Some(old) = self.pending.replace(request) {
            QueueAction::Replaced(old)
        } else {
            QueueAction::Queued
        }
    }
    /// Complete active work and promote pending work, if any.
    pub fn complete(&mut self) -> Option<T> {
        let next = self.pending.take();
        self.in_flight = next.is_some();
        next
    }
    /// Discard work that has not started while keeping the in-flight slot occupied.
    pub fn discard_pending(&mut self) -> Option<T> {
        self.pending.take()
    }
    /// Discard pending work and reset state; a later completion cannot start it.
    pub fn reset(&mut self) -> Option<T> {
        self.in_flight = false;
        self.pending.take()
    }
}
/// Fixed-capacity sliding-window limiter; timestamps must be nondecreasing.
#[derive(Debug)]
pub struct SlidingWindowRateLimiter {
    limit: usize,
    window: u64,
    timestamps: std::collections::VecDeque<u64>,
}

/// One delayed retry, bound to the input generation that produced it.
#[derive(Debug)]
struct ScheduledRetry<T> {
    generation: u64,
    request: T,
    due_at_ms: u64,
}

/// Pure, latest-generation retry scheduler. The caller supplies monotonic time and jitter seed.
#[derive(Debug)]
pub struct RetryCoordinator<T> {
    generation: u64,
    pending: Option<ScheduledRetry<T>>,
}

impl<T> RetryCoordinator<T> {
    /// Create a scheduler for the current input generation.
    pub fn new(generation: u64) -> Self {
        Self {
            generation,
            pending: None,
        }
    }

    /// Replace current input only with a newer generation, cancelling its older retry.
    pub fn replace_generation(&mut self, generation: u64) {
        if generation > self.generation {
            self.generation = generation;
            self.pending = None;
        }
    }

    /// Schedule a delay already approved by `retry_delay`; returns false past retry limit.
    pub fn schedule(
        &mut self,
        generation: u64,
        request: T,
        retries_already_performed: u8,
        now_ms: u64,
        delay: std::time::Duration,
    ) -> bool {
        if generation != self.generation || retries_already_performed >= 2 {
            return false;
        }
        self.pending = Some(ScheduledRetry {
            generation,
            request,
            due_at_ms: now_ms.saturating_add(delay.as_millis().min(u128::from(u64::MAX)) as u64),
        });
        true
    }

    /// Take due work only if its generation is current and the limiter admits it.
    /// Rate-limited work remains pending for a later deterministic poll.
    pub fn take_due(
        &mut self,
        now_ms: u64,
        current_generation: u64,
        limiter: &mut SlidingWindowRateLimiter,
    ) -> Option<T> {
        if current_generation != self.generation {
            self.replace_generation(current_generation);
            return None;
        }
        let pending = self.pending.as_ref()?;
        if pending.generation != current_generation {
            self.pending = None;
            return None;
        }
        if now_ms < pending.due_at_ms || !limiter.allow_at(now_ms) {
            return None;
        }
        self.pending.take().map(|retry| retry.request)
    }
}

impl SlidingWindowRateLimiter {
    /// Create a limiter for at most `limit` admissions per `window` milliseconds.
    pub fn new(limit: usize, window: u64) -> Self {
        Self {
            limit,
            window,
            timestamps: Default::default(),
        }
    }
    /// Admit a request at monotonic millisecond timestamp `now` if capacity remains.
    pub fn allow_at(&mut self, now: u64) -> bool {
        while self
            .timestamps
            .front()
            .is_some_and(|t| now >= *t && now - *t >= self.window)
        {
            self.timestamps.pop_front();
        }
        if self.limit == 0 || self.timestamps.len() >= self.limit {
            return false;
        }
        self.timestamps.push_back(now);
        true
    }

    /// Return the remaining wait in milliseconds once all admission slots are used.
    pub fn delay_at(&self, now: u64) -> u64 {
        if self.limit == 0 {
            return u64::MAX;
        }
        if self.timestamps.len() < self.limit {
            return 0;
        }
        self.timestamps.front().map_or(0, |oldest| {
            oldest.saturating_add(self.window).saturating_sub(now)
        })
    }
}

#[cfg(test)]
mod primitive_tests {
    use super::*;
    #[test]
    fn queue_replaces_latest_and_completes_in_order() {
        let mut q = ProviderQueue::new();
        assert_eq!(q.submit(1), QueueAction::Start(1));
        assert_eq!(q.submit(2), QueueAction::Queued);
        assert_eq!(q.submit(3), QueueAction::Replaced(2));
        assert_eq!(q.complete(), Some(3));
        assert_eq!(q.complete(), None);
    }
    #[test]
    fn reset_discards_pending_without_starting_it() {
        let mut q = ProviderQueue::new();
        q.submit(1);
        q.submit(2);
        assert_eq!(q.reset(), Some(2));
        assert_eq!(q.complete(), None);
        assert_eq!(q.submit(3), QueueAction::Start(3));
    }
    #[test]
    fn rate_limit_expires_exactly_at_window_and_handles_regression() {
        let mut l = SlidingWindowRateLimiter::new(2, 100);
        assert!(l.allow_at(100));
        assert!(l.allow_at(150));
        assert_eq!(l.delay_at(199), 1);
        assert!(!l.allow_at(199));
        assert!(!l.allow_at(50));
        assert_eq!(l.delay_at(200), 0);
        assert!(l.allow_at(200));
    }
}

/// Failure classes considered by the provider retry policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryFailure {
    /// A request exceeded its timeout.
    Timeout,
    /// The connection failed before a usable reply.
    Connection,
    /// The provider returned an HTTP status.
    Http(u16),
    /// Non-retryable application or response error.
    Other,
}

/// Decide the delay before a retry. `retries` is the number already performed.
/// Jitter is deterministic and bounded to 250 ms from the supplied seed.
pub fn retry_delay(
    failure: RetryFailure,
    retries: u8,
    retry_after_seconds: Option<u64>,
    provider_timeout: std::time::Duration,
    seed: u64,
    generation_is_current: bool,
) -> Option<std::time::Duration> {
    use std::time::Duration;
    let transient = match failure {
        RetryFailure::Timeout | RetryFailure::Connection => true,
        RetryFailure::Http(code) => code == 429 || (500..=599).contains(&code),
        RetryFailure::Other => false,
    };
    if !transient || retries >= 2 || !generation_is_current || provider_timeout.is_zero() {
        return None;
    }
    let base = if let Some(seconds) = retry_after_seconds {
        Duration::from_secs(seconds).min(provider_timeout)
    } else {
        let millis = if retries == 0 { 1_000u64 } else { 2_000u64 };
        let mut x = seed
            ^ (u64::from(retries)
                .wrapping_add(1)
                .wrapping_mul(0x9E3779B97F4A7C15));
        x ^= x >> 30;
        x = x.wrapping_mul(0xBF58476D1CE4E5B9);
        x ^= x >> 27;
        let jitter_ms = x % 251;
        Duration::from_millis(millis.saturating_add(jitter_ms))
    };
    Some(base.min(provider_timeout))
}

#[cfg(test)]
mod retry_tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn classes_attempt_limits_stale_and_determinism() {
        for failure in [
            RetryFailure::Timeout,
            RetryFailure::Connection,
            RetryFailure::Http(429),
            RetryFailure::Http(500),
            RetryFailure::Http(599),
        ] {
            assert!(retry_delay(failure, 0, None, Duration::from_secs(15), 9, true).is_some());
        }
        for failure in [
            RetryFailure::Http(400),
            RetryFailure::Http(401),
            RetryFailure::Http(499),
            RetryFailure::Http(600),
            RetryFailure::Other,
        ] {
            assert_eq!(
                retry_delay(failure, 0, None, Duration::from_secs(15), 9, true),
                None
            );
        }
        assert_eq!(
            retry_delay(
                RetryFailure::Timeout,
                2,
                None,
                Duration::from_secs(15),
                1,
                true
            ),
            None
        );
        assert_eq!(
            retry_delay(
                RetryFailure::Timeout,
                0,
                None,
                Duration::from_secs(15),
                1,
                false
            ),
            None
        );
        assert_eq!(
            retry_delay(
                RetryFailure::Timeout,
                0,
                None,
                Duration::from_secs(15),
                1,
                true
            ),
            retry_delay(
                RetryFailure::Timeout,
                0,
                None,
                Duration::from_secs(15),
                1,
                true
            )
        );
    }
    #[test]
    fn backoffs_retry_after_and_timeout_are_bounded() {
        let a = retry_delay(
            RetryFailure::Connection,
            0,
            None,
            Duration::from_secs(15),
            0,
            true,
        )
        .unwrap();
        let b = retry_delay(
            RetryFailure::Connection,
            1,
            None,
            Duration::from_secs(15),
            0,
            true,
        )
        .unwrap();
        assert!((Duration::from_secs(1)..=Duration::from_millis(1250)).contains(&a));
        assert!((Duration::from_secs(2)..=Duration::from_millis(2250)).contains(&b));
        assert_eq!(
            retry_delay(
                RetryFailure::Http(429),
                0,
                Some(u64::MAX),
                Duration::from_secs(15),
                0,
                true
            ),
            Some(Duration::from_secs(15))
        );
        assert_eq!(
            retry_delay(
                RetryFailure::Http(429),
                0,
                Some(3),
                Duration::from_secs(15),
                0,
                true
            ),
            Some(Duration::from_secs(3))
        );
        assert_eq!(
            retry_delay(RetryFailure::Timeout, 0, None, Duration::ZERO, 0, true),
            None
        );
    }

    #[test]
    fn coordinator_schedules_due_retry_and_retains_rate_limited_work() {
        let mut coordinator = RetryCoordinator::new(7);
        let delay = retry_delay(
            RetryFailure::Timeout,
            0,
            None,
            Duration::from_secs(15),
            0,
            true,
        )
        .unwrap();
        assert!(coordinator.schedule(7, "retry", 0, 100, delay));
        let mut limiter = SlidingWindowRateLimiter::new(1, 100);
        let due_at = 100 + delay.as_millis() as u64;
        assert!(limiter.allow_at(due_at - 1));
        assert_eq!(coordinator.take_due(due_at, 7, &mut limiter), None);
        assert_eq!(
            coordinator.take_due(due_at + 100, 7, &mut limiter),
            Some("retry")
        );
    }

    #[test]
    fn coordinator_cancels_replaced_and_stale_work() {
        let mut coordinator = RetryCoordinator::new(1);
        assert!(coordinator.schedule(1, 1, 1, 0, Duration::from_millis(5)));
        coordinator.replace_generation(2);
        assert!(!coordinator.schedule(1, 9, 1, 0, Duration::ZERO));
        assert!(coordinator.schedule(2, 2, 1, 0, Duration::ZERO));
        let mut limiter = SlidingWindowRateLimiter::new(2, 100);
        assert_eq!(coordinator.take_due(0, 1, &mut limiter), None);
        assert_eq!(coordinator.take_due(0, 2, &mut limiter), Some(2));
    }
}
