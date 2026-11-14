//! Shared text cleanup and script detection primitives.

#![forbid(unsafe_code)]

/// Normalize CRLF and bare CR to LF and remove NUL characters.
///
/// Other characters, including whitespace and Unicode combining marks, are preserved.
pub fn normalize_input(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                output.push('\n');
            }
            '\0' => {}
            _ => output.push(ch),
        }
    }
    output
}

/// Return true if text contains at least one Japanese-script character.
///
/// Covers Hiragana, Katakana, CJK Unified Ideographs and the Japanese
/// iteration/prolonged-sound marks. This is a script-presence check, not language ID.
pub fn contains_japanese(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(ch as u32,
            0x3040..=0x30ff | 0x31f0..=0x31ff | 0x3400..=0x4dbf |
            0x4e00..=0x9fff | 0xf900..=0xfaff | 0x3005 | 0x3006
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_newlines_and_discards_nul_without_trimming() {
        assert_eq!(normalize_input(" a\r\nb\rc\0 \n"), " a\nb\nc \n");
    }

    #[test]
    fn japanese_detection_handles_common_scripts_and_markers() {
        assert!(contains_japanese("かな"));
        assert!(contains_japanese("カタカナ"));
        assert!(contains_japanese("漢字"));
        assert!(contains_japanese("ー"));
        assert!(!contains_japanese("English 123"));
        assert!(!contains_japanese("한글"));
    }
}
