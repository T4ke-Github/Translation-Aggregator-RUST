//! Translator capability metadata and request eligibility policy.

#![forbid(unsafe_code)]

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
