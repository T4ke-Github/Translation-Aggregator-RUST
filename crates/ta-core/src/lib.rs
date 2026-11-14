//! Core text-input value types and validation.

#![forbid(unsafe_code)]

use std::fmt;

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

#[cfg(test)]
mod tests {
    use super::*;

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
