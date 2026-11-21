use ta_core::DEFAULT_MAX_INPUT_CHARS;
use ta_ingest::{BufferSettings, PipelineOptions};
use ta_platform::SettingsDocument;
use ta_text::{CharacterRepeatMode, NewlineMode};

pub(super) struct InputSettings {
    pub(super) buffer: BufferSettings,
    pub(super) pipeline: PipelineOptions,
    pub(super) automatic: bool,
    pub(super) max_chars: usize,
}

impl Default for InputSettings {
    fn default() -> Self {
        Self {
            buffer: BufferSettings::default_for(super::CONTEXT),
            pipeline: PipelineOptions {
                suppress_repeats: true,
                ..PipelineOptions::default()
            },
            automatic: true,
            max_chars: DEFAULT_MAX_INPUT_CHARS,
        }
    }
}

impl InputSettings {
    pub(super) fn from_document(document: &SettingsDocument) -> Result<Self, String> {
        let Some(value) = document
            .value()
            .get("inputs")
            .and_then(|inputs| inputs.get("clipboard"))
        else {
            return Ok(Self::default());
        };
        let table = value.as_table().ok_or("inputs.clipboard must be a table")?;
        let boolean = |name: &str, default| {
            table.get(name).map_or(Ok(default), |value| {
                value
                    .as_bool()
                    .ok_or_else(|| format!("inputs.clipboard.{name} must be a boolean"))
            })
        };
        let number = |name: &str, default| {
            table.get(name).map_or(Ok(default), |value| {
                value
                    .as_integer()
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or_else(|| {
                        format!("inputs.clipboard.{name} must be a non-negative integer")
                    })
            })
        };
        let repeat_mode = match table.get("repeat_characters").map(|value| value.as_str()) {
            None | Some(Some("none")) => CharacterRepeatMode::None,
            Some(Some("preset_2")) => CharacterRepeatMode::Preset2,
            Some(Some("auto_constant")) => CharacterRepeatMode::AutoConstant,
            Some(Some("auto_variable")) => CharacterRepeatMode::AutoVariable,
            Some(Some("all")) => CharacterRepeatMode::All,
            _ => return Err("inputs.clipboard.repeat_characters must be none, preset_2, auto_constant, auto_variable, or all".into()),
        };
        let newline_mode = match table.get("line_breaks").map(|value| value.as_str()) {
            None | Some(Some("keep")) => NewlineMode::Keep,
            Some(Some("remove")) => NewlineMode::Remove,
            Some(Some("keep_some")) => {
                NewlineMode::KeepSome(number("keep_first", 1)?, number("keep_last", 0)?)
            }
            _ => {
                return Err(
                    "inputs.clipboard.line_breaks must be keep, remove, or keep_some".into(),
                );
            }
        };
        let max_chars = number("max_input_chars", DEFAULT_MAX_INPUT_CHARS)?;
        if max_chars == 0 {
            return Err("inputs.clipboard.max_input_chars must be positive".into());
        }
        Ok(Self {
            buffer: BufferSettings {
                enabled: boolean("enabled", true)?,
                idle_delay_ms: number("idle_delay_ms", 0)? as u64,
                phrase_extensions: boolean("phrase_extensions", false)?,
                ..BufferSettings::default_for(super::CONTEXT)
            },
            pipeline: PipelineOptions {
                japanese_only: boolean("japanese_only", false)?,
                suppress_repeats: boolean("suppress_repeats", true)?,
                detect_loops: boolean("detect_loops", false)?,
                phrase_repeat: boolean("phrase_repeat", false)?,
                repeat_mode,
                newline_mode,
                ..PipelineOptions::default()
            },
            automatic: boolean("auto_translate", true)?,
            max_chars,
        })
    }
}
