//! Headless developer console harness for deterministic input preparation.
#![forbid(unsafe_code)]

use std::{
    env,
    io::{self, Read},
    process::ExitCode,
};
use ta_core::{ContextId, History, TextInput};
use ta_ingest::{PipelineOptions, PipelineTransforms, PreparedInput, TextFilter, prepare_input};
use ta_substitute::SubstitutionEngine;

#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Help,
    Run {
        text: Option<String>,
        context: String,
    },
}

fn parse_args(args: &[String]) -> Result<Mode, String> {
    if args.is_empty() {
        return Ok(Mode::Help);
    }
    let mut text = None;
    let mut stdin = false;
    let mut context = "typed".to_owned();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" if args.len() == 1 => return Ok(Mode::Help),
            "--text" => {
                i += 1;
                if i == args.len() || text.is_some() || stdin {
                    return Err(
                        "--text requires one value and cannot be combined with --stdin".into(),
                    );
                }
                text = Some(args[i].clone());
            }
            "--stdin" if !stdin && text.is_none() => stdin = true,
            "--context" => {
                i += 1;
                if i == args.len() {
                    return Err("--context requires an ID".into());
                }
                context = args[i].clone();
            }
            value => return Err(format!("unknown or invalid argument: {value}")),
        }
        i += 1;
    }
    if text.is_none() && !stdin {
        return Err("provide --text TEXT or --stdin".into());
    }
    Ok(Mode::Run { text, context })
}

fn run(text: &str, context: &str) -> Result<String, String> {
    let input = TextInput::new(text).map_err(|e| format!("invalid input: {e:?}"))?;
    let context = ContextId::new(context).map_err(|e| format!("invalid context: {e:?}"))?;
    let mut filter = TextFilter::new();
    let mut history = History::with_capacity(100);
    let substitutions = SubstitutionEngine::new(false, vec![]);
    let prepared = prepare_input(
        input,
        context,
        &mut filter,
        &substitutions,
        &mut history,
        PipelineOptions::default(),
        PipelineTransforms {
            pre_substitution: |s: &str| Ok::<String, String>(s.to_owned()),
            post_substitution: |s: &str| Ok::<String, String>(s.to_owned()),
        },
    );
    Ok(match prepared {
        PreparedInput::Ready(p) => format!(
            "Original: {}\nProcessed: {}\nStatus: ready",
            p.original_text, p.processed_text
        ),
        PreparedInput::Skipped(reason) => {
            format!("Original: {text}\nProcessed: <skipped>\nStatus: skipped ({reason:?})")
        }
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match parse_args(&args) {
        Ok(Mode::Help) => {
            println!("Usage: translation-aggregator (--text TEXT | --stdin) [--context ID]");
            ExitCode::SUCCESS
        }
        Ok(Mode::Run { text, context }) => {
            let input = match text {
                Some(s) => s,
                None => {
                    let mut s = String::new();
                    if let Err(e) = io::stdin().read_to_string(&mut s) {
                        eprintln!("stdin error: {e}");
                        return ExitCode::FAILURE;
                    }
                    s
                }
            };
            match run(&input, &context) {
                Ok(out) => {
                    println!("{out}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Err(e) => {
            eprintln!("{e}\nUsage: translation-aggregator (--text TEXT | --stdin) [--context ID]");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_text_and_stdin_modes() {
        assert_eq!(
            parse_args(&["--text".into(), "hello".into()]),
            Ok(Mode::Run {
                text: Some("hello".into()),
                context: "typed".into()
            })
        );
        assert_eq!(
            parse_args(&["--stdin".into(), "--context".into(), "api:editor".into()]),
            Ok(Mode::Run {
                text: None,
                context: "api:editor".into()
            })
        );
    }
    #[test]
    fn rejects_invalid_arguments() {
        assert!(parse_args(&["--wat".into()]).is_err());
        assert!(parse_args(&["--text".into()]).is_err());
        assert!(parse_args(&["--text".into(), "x".into(), "--stdin".into()]).is_err());
        assert_eq!(parse_args(&[]), Ok(Mode::Help));
    }
    #[test]
    fn runs_deterministic_pipeline_and_reports_skips() {
        assert_eq!(
            run("hello", "typed").unwrap(),
            "Original: hello\nProcessed: hello\nStatus: ready"
        );
        assert!(run("   ", "typed").is_err());
    }
}
