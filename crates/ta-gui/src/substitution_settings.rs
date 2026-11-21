use ta_platform::{SettingsDocument, load_settings, settings_paths};
use ta_substitute::{SubstitutionEngine, SubstitutionProfile, SubstitutionRule};

pub(crate) struct SubstitutionSettings {
    enabled: bool,
    profiles: Vec<SubstitutionProfile>,
}

impl Default for SubstitutionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            profiles: Vec::new(),
        }
    }
}

impl SubstitutionSettings {
    pub(crate) fn load() -> Result<Self, String> {
        let Ok(paths) = settings_paths() else {
            return Ok(Self::default());
        };
        let settings =
            load_settings(&paths.settings_file).map_err(|error| format!("settings: {error}"))?;
        Self::from_document(&settings.document)
    }

    pub(crate) fn from_document(document: &SettingsDocument) -> Result<Self, String> {
        let Some(value) = document.value().get("substitutions") else {
            return Ok(Self::default());
        };
        let settings = value.as_table().ok_or("substitutions must be a table")?;
        let enabled = settings
            .get("enabled")
            .map(|value| {
                value
                    .as_bool()
                    .ok_or("substitutions.enabled must be a boolean")
            })
            .transpose()?
            .unwrap_or(true);
        let mut profiles = Vec::new();
        if let Some(value) = settings.get("profiles") {
            let values = value
                .as_array()
                .ok_or("substitutions.profiles must be an array")?;
            for (index, value) in values.iter().enumerate() {
                let path = format!("substitutions.profiles[{}]", index + 1);
                let context = value
                    .get("context")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| format!("{path}.context must be a string"))?;
                if context.is_empty()
                    || context.chars().count() > 128
                    || context
                        .chars()
                        .any(|character| character.is_control() || character.is_whitespace())
                {
                    return Err(format!(
                        "{path}.context must be a nonempty context pattern of up to 128 characters"
                    ));
                }
                let rules = value
                    .get("rules")
                    .and_then(|value| value.as_array())
                    .ok_or_else(|| format!("{path}.rules must be an array"))?;
                let mut parsed = Vec::with_capacity(rules.len());
                for (index, rule) in rules.iter().enumerate() {
                    let path = format!("{path}.rules[{}]", index + 1);
                    let matched = rule
                        .get("match")
                        .and_then(|value| value.as_str())
                        .ok_or_else(|| format!("{path}.match must be a string"))?;
                    let replacement = rule
                        .get("replacement")
                        .and_then(|value| value.as_str())
                        .ok_or_else(|| format!("{path}.replacement must be a string"))?;
                    parsed.push(
                        SubstitutionRule::try_new(matched, replacement)
                            .map_err(|error| format!("{path}: {error}"))?,
                    );
                }
                profiles.push(SubstitutionProfile::new(context).with_rules(parsed));
            }
        }
        Ok(Self { enabled, profiles })
    }

    pub(crate) fn with_rules(mut self, context: &str, rules: Vec<SubstitutionRule>) -> Self {
        if !rules.is_empty() {
            self.profiles
                .insert(0, SubstitutionProfile::new(context).with_rules(rules));
        }
        self
    }

    pub(crate) fn into_engine(self) -> SubstitutionEngine {
        SubstitutionEngine::new(self.enabled && !self.profiles.is_empty(), self.profiles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILES: &str = r#"
[substitutions]
enabled = true
[[substitutions.profiles]]
context = "*"
rules = [{ match = "猫", replacement = "Cat" }, { match = "犬", replacement = "Dog" }]
[[substitutions.profiles]]
context = "api:hook/*"
rules = [{ match = "猫", replacement = "Kitten" }, { match = "削除", replacement = "" }]
[[substitutions.profiles]]
context = "api:hook/game.exe"
rules = [{ match = "猫", replacement = "GameCat" }]
"#;

    #[test]
    fn saved_profiles_use_global_wildcard_and_exact_rules() {
        let settings = SettingsDocument::parse(PROFILES).unwrap();
        let engine = SubstitutionSettings::from_document(&settings)
            .unwrap()
            .into_engine();
        assert_eq!(
            engine.apply("猫と犬削除", "api:hook/game.exe"),
            "GameCatとDog"
        );
        assert_eq!(
            engine.apply("猫と犬削除", "api:hook/another.exe"),
            "KittenとDog"
        );
        assert_eq!(engine.apply("猫と犬削除", "typed"), "CatとDog削除");
    }

    #[test]
    fn saved_switch_disables_rules_and_empty_matches_are_rejected() {
        let disabled =
            SettingsDocument::parse(&PROFILES.replace("enabled = true", "enabled = false"))
                .unwrap();
        let engine = SubstitutionSettings::from_document(&disabled)
            .unwrap()
            .into_engine();
        assert_eq!(engine.apply("猫と犬", "api:hook/game.exe"), "猫と犬");

        let invalid =
            SettingsDocument::parse(&PROFILES.replace("match = \"猫\"", "match = \"\"")).unwrap();
        let Err(error) = SubstitutionSettings::from_document(&invalid) else {
            panic!("an empty substitution match was accepted");
        };
        assert!(error.contains("substitutions.profiles[1].rules[1]"));
        assert!(error.contains("must not be empty"));
    }
}
