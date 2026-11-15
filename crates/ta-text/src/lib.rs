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

/// Character repetition cleanup strategies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterRepeatMode {
    /// Do not modify repeated characters.
    None,
    /// Collapse repetitions of Japanese-script characters.
    Preset2,
    /// Automatically collapse constant-length repeats.
    AutoConstant,
    /// Automatically collapse variable-length repeats.
    AutoVariable,
    /// Collapse all repeated characters.
    All,
}

/// Collapse repeated characters according to the selected strategy.
pub fn normalize_repeats(input: &str, mode: CharacterRepeatMode) -> String {
    let chars: Vec<char> = input.chars().collect();
    let runs: Vec<(char, usize)> = {
        let mut runs = Vec::new();
        for ch in chars {
            if let Some((last, count)) = runs.last_mut()
                && *last == ch
            {
                *count += 1;
                continue;
            }
            runs.push((ch, 1));
        }
        runs
    };
    if mode == CharacterRepeatMode::AutoConstant
        && (runs.is_empty()
            || runs
                .iter()
                .any(|(_, count)| *count < 2 || *count != runs[0].1))
    {
        return input.to_owned();
    }
    let mut out = String::with_capacity(input.len());
    for (ch, count) in runs {
        let collapse = match mode {
            CharacterRepeatMode::None => false,
            CharacterRepeatMode::Preset2 => count == 2 && contains_japanese(&ch.to_string()),
            CharacterRepeatMode::AutoConstant
            | CharacterRepeatMode::AutoVariable
            | CharacterRepeatMode::All => count > 1,
        };
        out.extend(std::iter::repeat_n(ch, if collapse { 1 } else { count }));
    }
    out
}

/// Newline handling strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewlineMode {
    /// Remove line breaks.
    Remove,
    /// Keep all line breaks.
    Keep,
    /// Keep the specified number of initial and final line breaks.
    KeepSome(usize, usize),
}

/// Normalize CRLF/CR, trim entry edges, and apply the line-break mode.
pub fn normalize_newlines(input: &str, mode: NewlineMode) -> String {
    let normalized = normalize_input(input);
    let trimmed = normalized.trim();
    let break_count = trimmed.chars().filter(|ch| *ch == '\n').count();
    let mut output = String::with_capacity(trimmed.len());
    let mut breaks_seen = 0usize;
    let mut rest = trimmed;
    while let Some(index) = rest.find('\n') {
        let before = &rest[..index];
        let after = &rest[index + 1..];
        output.push_str(before);
        let keep = match mode {
            NewlineMode::Remove => false,
            NewlineMode::Keep => true,
            NewlineMode::KeepSome(first, last) => {
                breaks_seen < first || breaks_seen >= break_count.saturating_sub(last)
            }
        };
        breaks_seen += 1;
        if keep {
            output.push('\n');
            rest = after;
            continue;
        }
        let left = output.trim_end_matches([' ', '\t']).to_owned();
        let right = after.trim_start_matches(char::is_whitespace);
        let japanese_pair = left.chars().last().is_some_and(is_japanese_char)
            && right.chars().next().is_some_and(is_japanese_char);
        output = left;
        if !output.is_empty() && !output.ends_with('\n') && !right.is_empty() && !japanese_pair {
            output.push(' ');
        }
        rest = right;
    }
    output.push_str(rest);
    output
}

fn is_japanese_char(ch: char) -> bool {
    contains_japanese(&ch.to_string())
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
    fn repeat_modes_collapse_runs_by_mode() {
        assert_eq!(
            normalize_repeats("あああ!!", CharacterRepeatMode::Preset2),
            "あああ!!"
        );
        assert_eq!(
            normalize_repeats("あああ!!", CharacterRepeatMode::AutoConstant),
            "あああ!!"
        );
        assert_eq!(
            normalize_repeats("あああ!!", CharacterRepeatMode::AutoVariable),
            "あ!"
        );
        assert_eq!(
            normalize_repeats("あああ!!", CharacterRepeatMode::All),
            "あ!"
        );
        assert_eq!(normalize_repeats("ああ", CharacterRepeatMode::None), "ああ");
    }

    #[test]
    fn newline_modes_normalize_trim_and_filter_blank_lines() {
        assert_eq!(
            normalize_newlines("  a\r\n\r b \r", NewlineMode::Remove),
            "a b"
        );
        assert_eq!(
            normalize_newlines("  a\r\n\r b \r", NewlineMode::Keep),
            "a\n\n b"
        );
        assert_eq!(
            normalize_newlines("  a\r\n\r b \r", NewlineMode::KeepSome(1, 0)),
            "a\nb"
        );
        assert_eq!(normalize_newlines("A 日 B", NewlineMode::Keep), "A 日 B");
    }

    #[test]
    fn spec_repeat_and_newline_edge_cases() {
        assert_eq!(
            normalize_repeats("ここんんににちちはは", CharacterRepeatMode::Preset2),
            "こんにちは"
        );
        assert_eq!(
            normalize_repeats("ここんんににちちはは", CharacterRepeatMode::AutoConstant),
            "こんにちは"
        );
        assert_eq!(
            normalize_repeats("ああいいい", CharacterRepeatMode::AutoConstant),
            "ああいいい"
        );
        assert_eq!(
            normalize_repeats("ああい", CharacterRepeatMode::AutoConstant),
            "ああい"
        );
        assert_eq!(
            normalize_repeats("ここんんににちちはは", CharacterRepeatMode::AutoVariable),
            "こんにちは"
        );
        assert_eq!(normalize_newlines("a\nb", NewlineMode::Remove), "a b");
        assert_eq!(normalize_newlines("あ\nい", NewlineMode::Remove), "あい");
        assert_eq!(
            normalize_newlines("a\nb\nc\nd", NewlineMode::KeepSome(1, 1)),
            "a\nb c\nd"
        );
        assert_eq!(normalize_newlines("a  b\nc", NewlineMode::Keep), "a  b\nc");
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
