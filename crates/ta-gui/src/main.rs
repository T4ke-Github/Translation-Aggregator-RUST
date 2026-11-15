//! Headless developer console harness for deterministic input preparation.
#![forbid(unsafe_code)]

use std::{
    env, fs,
    io::{self, Read},
    path::Path,
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
        provider: Option<String>,
        source: Option<String>,
        target: Option<String>,
    },
}

fn parse_args(args: &[String]) -> Result<Mode, String> {
    if args.is_empty() {
        return Ok(Mode::Help);
    }
    let mut text = None;
    let mut stdin = false;
    let mut context = "typed".to_owned();
    let mut provider = None;
    let mut source = None;
    let mut target = None;
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
            "--provider" | "--source" | "--target" => {
                let flag = args[i].as_str();
                i += 1;
                if i == args.len() || args[i].trim().is_empty() || args[i].starts_with("--") {
                    return Err(format!("{flag} requires a value"));
                }
                let slot = match flag {
                    "--provider" => &mut provider,
                    "--source" => &mut source,
                    _ => &mut target,
                };
                if slot.replace(args[i].clone()).is_some() {
                    return Err(format!("{flag} was provided more than once"));
                }
            }
            value => return Err(format!("unknown or invalid argument: {value}")),
        }
        i += 1;
    }
    if text.is_none() && !stdin {
        return Err("provide --text TEXT or --stdin".into());
    }
    if provider.is_some() != (source.is_some() && target.is_some())
        || (provider.is_none() && (source.is_some() || target.is_some()))
    {
        return Err("--provider requires --source and --target, and vice versa".into());
    }
    Ok(Mode::Run {
        text,
        context,
        provider,
        source,
        target,
    })
}

fn prepare(text: &str, context: &str) -> Result<PreparedInput, String> {
    let input = TextInput::new(text).map_err(|e| format!("invalid input: {e:?}"))?;
    let context = ContextId::new(context).map_err(|e| format!("invalid context: {e:?}"))?;
    let mut filter = TextFilter::new();
    let mut history = History::with_capacity(100);
    let substitutions = SubstitutionEngine::new(false, vec![]);
    Ok(prepare_input(
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
    ))
}

fn run_with_provider(
    text: &str,
    context: &str,
    path: &Path,
    source: &str,
    target: &str,
) -> Result<String, String> {
    let prepared = prepare(text, context)?;
    let PreparedInput::Ready(input) = prepared else {
        return Ok(format!(
            "Original: {text}\nProcessed: <skipped>\nStatus: skipped"
        ));
    };
    let definition = fs::read_to_string(path).map_err(|e| format!("provider file: {e}"))?;
    let provider = ta_provider_web::ProviderDefinition::from_toml(&definition)
        .map_err(|e| format!("provider definition: {e}"))?;
    if !provider.definition().provider.enabled {
        return Err("provider is disabled".into());
    }
    let mut secrets = std::collections::BTreeMap::new();
    let request = loop {
        match provider.map_request_with_values(
            &input.processed_text,
            source,
            target,
            &std::collections::BTreeMap::new(),
            &secrets,
        ) {
            Ok(request) => break request,
            Err(ta_provider_web::ProviderError::NeedsSecret(name)) => {
                if secrets.contains_key(&name) || secrets.len() >= 64 {
                    return Err("Needs key".into());
                }
                let value = ta_platform::credentials::read_secret(&name)
                    .map_err(|_| "Needs key (credential store unavailable)".to_owned())?
                    .ok_or_else(|| "Needs key".to_owned())?;
                secrets.insert(name, value);
            }
            Err(error) => return Err(format!("provider request: {error}")),
        }
    };
    if ta_provider_web::requires_insecure_http_confirmation(&request.url, true) {
        return Err("refusing non-loopback HTTP provider in console mode".into());
    }
    let bytes = provider
        .send(&request)
        .map_err(|e| format!("provider send: {e}"))?;
    let translation = provider
        .extract_translation_from_bytes(&bytes)
        .map_err(|e| format!("provider reply: {e}"))?;
    Ok(format!(
        "Original: {}\nProcessed: {}\nTranslation: {translation}\nStatus: ready",
        input.original_text, input.processed_text
    ))
}

fn run(text: &str, context: &str) -> Result<String, String> {
    let prepared = prepare(text, context)?;
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
            println!(
                "Usage: translation-aggregator (--text TEXT | --stdin) [--context ID] [--provider FILE --source LANG --target LANG]"
            );
            ExitCode::SUCCESS
        }
        Ok(Mode::Run {
            text,
            context,
            provider,
            source,
            target,
        }) => {
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
            let result = match (provider, source, target) {
                (Some(path), Some(source), Some(target)) => {
                    run_with_provider(&input, &context, Path::new(&path), &source, &target)
                }
                _ => run(&input, &context),
            };
            match result {
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
            eprintln!(
                "{e}\nUsage: translation-aggregator (--text TEXT | --stdin) [--context ID] [--provider FILE --source LANG --target LANG]"
            );
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
                context: "typed".into(),
                provider: None,
                source: None,
                target: None,
            })
        );
        assert_eq!(
            parse_args(&["--stdin".into(), "--context".into(), "api:editor".into()]),
            Ok(Mode::Run {
                text: None,
                context: "api:editor".into(),
                provider: None,
                source: None,
                target: None,
            })
        );
    }
    #[test]
    fn provider_mode_requires_language_pair_and_rejects_lone_flags() {
        assert_eq!(
            parse_args(&[
                "--text".into(),
                "hello".into(),
                "--provider".into(),
                "sample.toml".into(),
                "--source".into(),
                "en".into(),
                "--target".into(),
                "fr".into(),
            ]),
            Ok(Mode::Run {
                text: Some("hello".into()),
                context: "typed".into(),
                provider: Some("sample.toml".into()),
                source: Some("en".into()),
                target: Some("fr".into()),
            })
        );
        for args in [
            vec!["--text", "hi", "--provider", "sample.toml"],
            vec!["--text", "hi", "--source", "en"],
            vec![
                "--text",
                "hi",
                "--provider",
                "sample.toml",
                "--source",
                "en",
            ],
        ] {
            assert!(parse_args(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
    }

    #[test]
    fn rejects_invalid_arguments() {
        assert!(parse_args(&["--wat".into()]).is_err());
        assert!(parse_args(&["--text".into()]).is_err());
        assert!(parse_args(&["--text".into(), "x".into(), "--stdin".into()]).is_err());
        assert_eq!(parse_args(&[]), Ok(Mode::Help));
    }
    #[test]
    fn runs_local_provider_end_to_end_and_rejects_disabled_provider() {
        use std::{
            fs,
            io::{Read, Write},
            net::TcpListener,
            thread,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let started = std::time::Instant::now();
            let mut connection = loop {
                match listener.accept() {
                    Ok((connection, _)) => break connection,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && started.elapsed() < std::time::Duration::from_secs(5) =>
                    {
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => panic!("local provider did not connect: {e}"),
                }
            };
            connection
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = [0_u8; 4096];
            let size = connection.read(&mut bytes).unwrap();
            assert!(
                String::from_utf8_lossy(&bytes[..size]).starts_with("POST /translate HTTP/1.1")
            );
            let reply = br#"{"translatedText":"bonjour"}"#;
            write!(
                connection,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                reply.len()
            )
            .unwrap();
            connection.write_all(reply).unwrap();
        });
        let path = std::env::current_dir().unwrap().join(format!(
            "target/provider-smoke-{}-{}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let definition = format!(
            "[provider]\nid='smoke'\nname='Smoke'\nenabled=true\n[request]\nmethod='POST'\nurl='http://{addr}/translate'\nbody.json={{q='{{{{text}}}}',source='{{{{src}}}}',target='{{{{dst}}}}'}}\n[response]\ntext={{jsonpath='$.translatedText'}}\n"
        );
        fs::write(&path, &definition).unwrap();
        let result = run_with_provider("hello", "typed", &path, "en", "fr");
        fs::remove_file(&path).unwrap();
        server.join().unwrap();
        assert_eq!(
            result.unwrap(),
            "Original: hello\nProcessed: hello\nTranslation: bonjour\nStatus: ready"
        );
        let disabled = definition.replace("enabled=true", "enabled=false");
        fs::write(&path, disabled).unwrap();
        let result = run_with_provider("hello", "typed", &path, "en", "fr");
        fs::remove_file(path).unwrap();
        assert!(result.unwrap_err().contains("disabled"));
    }

    #[test]
    fn missing_credential_fails_closed_before_sending() {
        use std::fs;
        let name = format!("missing-{}", std::process::id());
        let definition = format!(
            "[provider]\nid='smoke'\nname='Smoke'\nenabled=true\n[request]\nmethod='GET'\nurl='http://127.0.0.1:9/translate'\nheaders={{Authorization='Bearer {{{{secret.{name}}}}}'}}\n[response]\ntext={{jsonpath='$.translatedText'}}\n"
        );
        let path = std::env::current_dir().unwrap().join(format!(
            "target/missing-key-{}-{}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, definition).unwrap();
        let result = run_with_provider("hello", "typed", &path, "en", "fr");
        fs::remove_file(path).unwrap();
        assert!(result.unwrap_err().contains("Needs key"));
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
