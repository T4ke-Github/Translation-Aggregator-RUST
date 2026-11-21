//! Replays observations from the published legacy application through the rewrite's core.

use serde_json::{Value, json};
use ta_core::{ContextId, History, TextInput};
use ta_ingest::{
    BufferSettings, InputBuffer, MergeMode, PipelineOptions, PipelineTransforms, PreparedInput,
    TextFilter, prepare_input,
};
use ta_substitute::{SubstitutionEngine, SubstitutionProfile, SubstitutionRule};
use ta_text::{CharacterRepeatMode, NewlineMode};

const CONTEXT_CASES: &str = include_str!("../../../fixtures/legacy/context-filters/cases.jsonl");
const SUBSTITUTION_CASES: &str = include_str!("../../../fixtures/legacy/substitutions/cases.jsonl");

fn cases(data: &str) -> impl Iterator<Item = Value> + '_ {
    data.lines().map(|line| serde_json::from_str(line).unwrap())
}

#[test]
fn context_filters_match_legacy_or_documented_rewrite_behavior() {
    for case in cases(CONTEXT_CASES) {
        let expected = if let Some(rewrite) = case.get("rewrite_output") {
            assert!(case["criterion"].is_string());
            assert_ne!(*rewrite, case["output"], "{}", case["id"]);
            rewrite
        } else {
            &case["output"]
        };
        assert_eq!(replay_filter(&case), *expected, "{}", case["id"]);
    }
}

fn replay_filter(case: &Value) -> Value {
    let settings = &case["settings"];
    if let Some(input) = case["input"].as_str() {
        return replay_clipboard_filter(input, settings);
    }
    let pieces = case["input"].as_array().unwrap();
    if settings["detect_loops"] == true {
        let mut filter = TextFilter::new();
        let mut displayed = String::new();
        let observations: Vec<_> = pieces
            .iter()
            .map(|piece| {
                if let Some(accepted) =
                    filter.accept_with_loop_detection("fixture", piece.as_str().unwrap(), true)
                {
                    displayed = accepted;
                }
                displayed.clone()
            })
            .collect();
        return json!(observations);
    }

    let spacing = settings["spacing_ms"].as_u64().unwrap();
    let mut buffer = InputBuffer::new();
    if settings["phrase_extensions"] == "basic" {
        buffer.configure(
            "fixture",
            BufferSettings {
                idle_delay_ms: 250,
                phrase_extensions: true,
                ..BufferSettings::default()
            },
        );
        for (index, piece) in pieces.iter().enumerate() {
            assert!(
                buffer
                    .push("fixture", piece.as_str().unwrap(), index as u64 * spacing)
                    .is_empty()
            );
        }
        let result = buffer.flush_due(pieces.len() as u64 * spacing + 250);
        assert_eq!(result.len(), 1);
        return json!(result[0].text);
    }

    let merge_mode = if settings["merge_stream"] == true {
        MergeMode::Stream
    } else {
        assert_eq!(settings["merge_on_flush"], true);
        MergeMode::OnFlush
    };
    buffer.configure(
        "merged",
        BufferSettings {
            idle_delay_ms: 250,
            ..BufferSettings::default()
        },
    );
    for (index, piece) in pieces.iter().enumerate() {
        let context = piece[0].as_str().unwrap();
        buffer.configure(
            context,
            BufferSettings {
                idle_delay_ms: 250,
                merge: Some(merge_mode),
                ..BufferSettings::default()
            },
        );
        assert!(
            buffer
                .push(context, piece[1].as_str().unwrap(), index as u64 * spacing)
                .is_empty()
        );
    }
    let last_input = (pieces.len() as u64 - 1) * spacing;
    let mut results = buffer.flush_due(last_input + 250);
    results.extend(buffer.flush_due(last_input + 500));
    assert!(results.iter().all(|result| result.context == "merged"));
    json!(
        results
            .into_iter()
            .map(|result| result.text)
            .collect::<Vec<_>>()
    )
}

fn replay_clipboard_filter(input: &str, settings: &Value) -> Value {
    let repeat_mode = match settings["repeat_characters"].as_str() {
        None | Some("none") => CharacterRepeatMode::None,
        Some("preset_2") => CharacterRepeatMode::Preset2,
        Some("auto_constant") => CharacterRepeatMode::AutoConstant,
        Some("auto_variable") => CharacterRepeatMode::AutoVariable,
        Some("all") => CharacterRepeatMode::All,
        mode => panic!("unrecognized repeat mode {mode:?}"),
    };
    let newline_mode = if settings["line_breaks"]["mode"] == "keep_some" {
        NewlineMode::KeepSome(
            settings["line_breaks"]["first"].as_u64().unwrap() as usize,
            settings["line_breaks"]["last"].as_u64().unwrap() as usize,
        )
    } else if settings["line_breaks"] == "remove" {
        NewlineMode::Remove
    } else {
        assert!(settings["line_breaks"].is_null());
        NewlineMode::Keep
    };
    let phrase_repeat = !settings["phrase_repeat"].is_null();
    if phrase_repeat {
        assert_eq!(settings["phrase_repeat"], json!({"min": 2, "max": 40}));
    }

    match prepare_input(
        TextInput::new(input).unwrap(),
        ContextId::new("clipboard").unwrap(),
        &mut TextFilter::new(),
        &SubstitutionEngine::new(false, vec![]),
        &mut History::with_capacity(1),
        PipelineOptions {
            japanese_only: settings["japanese_only"] == true,
            repeat_mode,
            newline_mode,
            phrase_repeat,
            ..PipelineOptions::default()
        },
        PipelineTransforms {
            pre_substitution: |text: &str| {
                assert!(
                    settings["auto_romaji_to_hiragana"] != true,
                    "Japanese-only input filtering should run before transforms"
                );
                Ok(text.to_owned())
            },
            post_substitution: |text: &str| Ok(text.to_owned()),
        },
    ) {
        PreparedInput::Ready(entry) => json!(entry.processed_text),
        PreparedInput::Skipped(_) => Value::Null,
    }
}

#[test]
fn substitutions_match_legacy_captures() {
    const SCOPED_CONTEXT: &str = "api:parity-demo";
    // These are the authored rules in the legacy UserDict fixture. Legacy process
    // activation maps to the selected source context in the rewrite.
    let rules = [
        ("日本", "Japan"),
        ("日本語", "Japanese"),
        ("a", "b"),
        ("b", "c"),
        (r"行\n末", r"行\t終"),
        (r"逆\\斜", "Backslash"),
    ]
    .map(|(matched, replacement)| SubstitutionRule::try_new(matched, replacement).unwrap());
    let engine = SubstitutionEngine::new(
        true,
        vec![
            SubstitutionProfile::new("*").with_rules(rules),
            SubstitutionProfile::new(SCOPED_CONTEXT)
                .with_rules([SubstitutionRule::try_new("星印", "Scoped").unwrap()]),
        ],
    );
    for case in cases(SUBSTITUTION_CASES) {
        let context = match case["settings"]["profile"].as_str().unwrap() {
            "*" => "clipboard",
            r"debug\translation-aggregator.exe" => SCOPED_CONTEXT,
            profile => panic!("unrecognized legacy profile {profile:?}"),
        };
        assert_eq!(
            engine.apply(case["input"].as_str().unwrap(), context),
            case["output"].as_str().unwrap(),
            "{}",
            case["id"]
        );
    }
}
