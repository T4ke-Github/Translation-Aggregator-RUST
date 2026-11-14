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

/// A single literal substitution. Backslash escapes are decoded when constructed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubstitutionRule {
    matched: String,
    replacement: String,
}

impl SubstitutionRule {
    fn new(matched: &str, replacement: &str) -> Self {
        Self {
            matched: decode_escapes(matched),
            replacement: decode_escapes(replacement),
        }
    }

    /// Decode escapes and reject an empty match before storing the rule.
    pub fn try_new(matched: &str, replacement: &str) -> Result<Self, String> {
        let rule = Self::new(matched, replacement);
        if rule.matched.is_empty() {
            Err("substitution match must not be empty".into())
        } else {
            Ok(rule)
        }
    }
}

fn decode_escapes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Rules assigned to a source context, or `*` for all sources.
#[derive(Clone, Debug)]
pub struct SubstitutionProfile {
    context: String,
    rules: Vec<SubstitutionRule>,
}
impl SubstitutionProfile {
    /// Create an empty profile for a source context pattern.
    pub fn new(context: impl Into<String>) -> Self {
        Self {
            context: context.into(),
            rules: Vec::new(),
        }
    }
    /// Add validated rules to this profile.
    pub fn with_rules(mut self, rules: impl IntoIterator<Item = SubstitutionRule>) -> Self {
        self.rules = rules.into_iter().collect();
        self
    }
}

/// Apply the active global and source-specific substitution rules.
pub struct SubstitutionEngine {
    enabled: bool,
    profiles: Vec<SubstitutionProfile>,
}
impl SubstitutionEngine {
    /// Set whether substitutions are active and load the available profiles.
    pub fn new(enabled: bool, profiles: Vec<SubstitutionProfile>) -> Self {
        Self { enabled, profiles }
    }
    /// Replace left-to-right, without scanning the result of a replacement.
    pub fn apply(&self, text: &str, context: &str) -> String {
        if !self.enabled {
            return text.to_owned();
        }
        let mut rules: Vec<(bool, &SubstitutionRule)> = Vec::new();
        for profile in &self.profiles {
            if profile.context == "*" {
                rules.extend(profile.rules.iter().map(|r| (false, r)));
            }
        }
        for profile in &self.profiles {
            if profile.context != "*" && wildcard_matches(&profile.context, context) {
                rules.extend(profile.rules.iter().map(|r| (true, r)));
            }
        }
        rules.sort_by(|a, b| {
            b.1.matched
                .chars()
                .count()
                .cmp(&a.1.matched.chars().count())
                .then_with(|| b.0.cmp(&a.0))
        });
        let mut output = String::new();
        let mut rest = text;
        while !rest.is_empty() {
            if let Some((_, rule)) = rules.iter().find(|(_, r)| rest.starts_with(&r.matched)) {
                output.push_str(&rule.replacement);
                rest = &rest[rule.matched.len()..];
            } else {
                let ch = rest.chars().next().unwrap();
                output.push(ch);
                rest = &rest[ch.len_utf8()..];
            }
        }
        output
    }
}

fn wildcard_matches(pattern: &str, value: &str) -> bool {
    let (mut p, mut v, mut star, mut retry) = (0, 0, None, 0);
    let (pb, vb) = (pattern.as_bytes(), value.as_bytes());
    while v < vb.len() {
        if p < pb.len() && pb[p] == vb[v] {
            p += 1;
            v += 1;
        } else if p < pb.len() && pb[p] == b'*' {
            star = Some(p);
            p += 1;
            retry = v;
        } else if let Some(s) = star {
            retry += 1;
            v = retry;
            p = s + 1;
        } else {
            return false;
        }
    }
    while p < pb.len() && pb[p] == b'*' {
        p += 1;
    }
    p == pb.len()
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

    #[test]
    fn substitution_longest_first_non_recursive_and_escape_aware() {
        let profile = SubstitutionProfile::new("*").with_rules([
            SubstitutionRule::new("日本", "Japan"),
            SubstitutionRule::new("日本語", "Japanese"),
            SubstitutionRule::new("a", "b"),
            SubstitutionRule::new("b", "c"),
            SubstitutionRule::new("\\n", "\\t\\\\q"),
        ]);
        let engine = SubstitutionEngine::new(true, vec![profile]);
        assert_eq!(engine.apply("日本語と日本 a", "other"), "JapaneseとJapan b");
        assert_eq!(engine.apply("\n", "other"), "\t\\q");
    }

    #[test]
    fn context_rules_override_global_and_disabled_or_invalid_rules_are_safe() {
        let global =
            SubstitutionProfile::new("*").with_rules([SubstitutionRule::new("x", "global")]);
        let context = SubstitutionProfile::new("api:hook/*")
            .with_rules([SubstitutionRule::new("x", "context")]);
        let engine = SubstitutionEngine::new(true, vec![global, context]);
        assert_eq!(engine.apply("x", "api:hook/game.exe"), "context");
        assert_eq!(engine.apply("x", "api:other/game.exe"), "global");
        assert_eq!(
            SubstitutionEngine::new(false, vec![]).apply("x", "any"),
            "x"
        );
        assert!(SubstitutionRule::try_new("", "x").is_err());
        let deletion = SubstitutionProfile::new("api:hook/game.exe")
            .with_rules([SubstitutionRule::try_new("x", "").unwrap()]);
        assert_eq!(
            SubstitutionEngine::new(true, vec![deletion]).apply("axb", "api:hook/game.exe"),
            "ab"
        );
        assert_eq!(decode_escapes("\\q"), "\\q");
    }
}
