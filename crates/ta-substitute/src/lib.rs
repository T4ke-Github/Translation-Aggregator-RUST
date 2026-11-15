//! Substitution profiles and the longest-match engine.

#![forbid(unsafe_code)]

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
        let mut rules: Vec<(u8, &SubstitutionRule)> = Vec::new();
        for profile in &self.profiles {
            if profile.context == "*" {
                rules.extend(profile.rules.iter().map(|r| (0, r)));
            }
        }
        for profile in &self.profiles {
            if profile.context != "*" && wildcard_matches(&profile.context, context) {
                let priority = if profile.context == context { 2 } else { 1 };
                rules.extend(profile.rules.iter().map(|r| (priority, r)));
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
    fn rejects_empty_match() {
        assert!(SubstitutionRule::try_new("", "x").is_err());
    }
    #[test]
    fn unknown_escape_is_preserved() {
        assert_eq!(decode_escapes(r"\q"), r"\q");
    }
    #[test]
    fn longest_match_decodes_newline_tab_and_backslash() {
        let p = SubstitutionProfile::new("*").with_rules([
            SubstitutionRule::try_new("日本", "Japan").unwrap(),
            SubstitutionRule::try_new("日本語", "Japanese").unwrap(),
            SubstitutionRule::try_new(r"\n", r"\t\\").unwrap(),
        ]);
        let e = SubstitutionEngine::new(true, vec![p]);
        assert_eq!(e.apply("日本語と日本", "x"), "JapaneseとJapan");
        assert_eq!(e.apply("\n", "x"), "\t\\");
    }
    #[test]
    fn exact_and_wildcard_context_and_equal_length_precedence() {
        let g = SubstitutionProfile::new("*")
            .with_rules([SubstitutionRule::try_new("x", "global").unwrap()]);
        let w = SubstitutionProfile::new("api:*")
            .with_rules([SubstitutionRule::try_new("x", "wild").unwrap()]);
        let e = SubstitutionEngine::new(true, vec![g, w]);
        assert_eq!(e.apply("x", "api:hook/game.exe"), "wild");
        let exact = SubstitutionProfile::new("api:hook/game.exe")
            .with_rules([SubstitutionRule::try_new("x", "exact").unwrap()]);
        assert_eq!(
            SubstitutionEngine::new(
                true,
                vec![e.profiles[0].clone(), e.profiles[1].clone(), exact]
            )
            .apply("x", "api:hook/game.exe"),
            "exact"
        );
    }
    #[test]
    fn empty_replacement_deletes_match() {
        let p =
            SubstitutionProfile::new("*").with_rules([SubstitutionRule::try_new("x", "").unwrap()]);
        assert_eq!(
            SubstitutionEngine::new(true, vec![p]).apply("axb", "x"),
            "ab"
        );
    }
}
