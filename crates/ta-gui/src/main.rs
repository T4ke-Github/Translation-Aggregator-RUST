//! Headless developer console harness for deterministic input preparation.
#![forbid(unsafe_code)]
mod clipboard;
mod provider_request;
mod substitution_settings;

use std::{
    env, fs,
    io::{self, IsTerminal, Read},
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

fn prepare(
    text: &str,
    context: &str,
    substitutions: &SubstitutionEngine,
) -> Result<PreparedInput, String> {
    let input = TextInput::new(text).map_err(|e| format!("invalid input: {e:?}"))?;
    let context = ContextId::new(context).map_err(|e| format!("invalid context: {e:?}"))?;
    let mut filter = TextFilter::new();
    let mut history = History::with_capacity(100);
    Ok(prepare_input(
        input,
        context,
        &mut filter,
        substitutions,
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
    substitutions: &SubstitutionEngine,
) -> Result<String, String> {
    let prepared = prepare(text, context, substitutions)?;
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
    let chunks = provider
        .manual_chunks(&input.processed_text)
        .map_err(|error| format!("provider request: {error}"))?;
    let mut requests = Vec::with_capacity(chunks.len());
    let mut secrets = std::collections::BTreeMap::new();
    let params = std::collections::BTreeMap::new();
    for chunk in &chunks {
        let request = provider_request::map_custom_request(
            &provider,
            chunk,
            (source, target),
            &params,
            &mut secrets,
            ta_platform::credentials::read_secret,
        )?;
        requests.push(request);
    }
    let secrets: Vec<_> = secrets.into_iter().collect();
    let mut translation = String::new();
    for request in &requests {
        let result = provider.send_translation(request).map_err(|error| {
            format!(
                "provider send: {}",
                ta_provider_web::redact_secrets(&error.to_string(), &secrets)
            )
        })?;
        translation.push_str(&result);
    }
    let translation = ta_provider_web::redact_secrets(&translation, &secrets);
    Ok(format!(
        "Original: {}\nProcessed: {}\nTranslation: {translation}\nStatus: ready",
        input.original_text, input.processed_text
    ))
}

fn run(text: &str, context: &str, substitutions: &SubstitutionEngine) -> Result<String, String> {
    let prepared = prepare(text, context, substitutions)?;
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
    if let [flag, rest @ ..] = args.as_slice()
        && flag == "--store-credential"
    {
        return match rest {
            [name] => {
                let stdin = io::stdin();
                if stdin.is_terminal() {
                    eprintln!("credential: pipe the secret on stdin");
                    return ExitCode::FAILURE;
                }
                let mut secret = String::new();
                let result = stdin
                    .take(256 * 1024 + 3)
                    .read_to_string(&mut secret)
                    .and_then(|length| {
                        if length >= 256 * 1024 + 3 {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "credential input is too long",
                            ));
                        }
                        ta_platform::credentials::save_secret(
                            name,
                            secret.trim_end_matches(['\r', '\n']),
                        )
                    });
                match result {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(error) => {
                        eprintln!("credential: {error}");
                        ExitCode::FAILURE
                    }
                }
            }
            _ => {
                eprintln!("Usage: translation-aggregator --store-credential NAME (reads stdin)");
                ExitCode::FAILURE
            }
        };
    }
    if let [flag, rest @ ..] = args.as_slice()
        && flag == "--clipboard"
    {
        return match clipboard::run(rest) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }
    match parse_args(&args) {
        Ok(Mode::Help) => {
            println!(
                "Usage: translation-aggregator (--text TEXT | --stdin) [--context ID] [--provider FILE --source LANG --target LANG]"
            );
            println!("{}", clipboard::USAGE);
            println!("       translation-aggregator --store-credential NAME (reads stdin)");
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
            let substitutions = match substitution_settings::SubstitutionSettings::load() {
                Ok(settings) => settings.into_engine(),
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::FAILURE;
                }
            };
            let result = match (provider, source, target) {
                (Some(path), Some(source), Some(target)) => run_with_provider(
                    &input,
                    &context,
                    Path::new(&path),
                    &source,
                    &target,
                    &substitutions,
                ),
                _ => run(&input, &context, &substitutions),
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
    fn no_substitutions() -> super::SubstitutionEngine {
        super::SubstitutionEngine::new(false, Vec::new())
    }
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
        let result = run_with_provider("hello", "typed", &path, "en", "fr", &no_substitutions());
        fs::remove_file(&path).unwrap();
        server.join().unwrap();
        assert_eq!(
            result.unwrap(),
            "Original: hello\nProcessed: hello\nTranslation: bonjour\nStatus: ready"
        );
        let disabled = definition.replace("enabled=true", "enabled=false");
        fs::write(&path, disabled).unwrap();
        let result = run_with_provider("hello", "typed", &path, "en", "fr", &no_substitutions());
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
        let result = run_with_provider("hello", "typed", &path, "en", "fr", &no_substitutions());
        fs::remove_file(path).unwrap();
        assert!(result.unwrap_err().contains("Needs key"));
    }

    #[test]
    fn manual_console_chunks_at_sentence_boundaries_and_rejects_indivisible_text() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let path = std::env::temp_dir().join(format!(
            "ta-manual-chunks-{}-{}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(
            &path,
            format!("[provider]\nid='manual'\nname='Manual'\nenabled=true\nmax_chars=2\n[request]\nmethod='POST'\nurl='http://{addr}/translate'\nbody.json={{q='{{{{text}}}}'}}\n[response]\ntext={{jsonpath='$.translatedText'}}\n"),
        ).unwrap();
        let error = run_with_provider("犬。abc", "typed", &path, "ja", "en", &no_substitutions())
            .unwrap_err();
        assert!(error.contains("3 characters, limit 2"), "{error}");
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );

        let mock_listener = listener.try_clone().unwrap();
        let server = std::thread::spawn(move || {
            clipboard::tests::expect_request_and_reply(
                &mock_listener,
                "犬。",
                r#"{"translatedText":"Dog. "}"#,
            );
            clipboard::tests::expect_request_and_reply(
                &mock_listener,
                "猫。",
                r#"{"translatedText":"Cat."}"#,
            );
        });
        let output = run_with_provider("犬。猫。", "typed", &path, "ja", "en", &no_substitutions());
        fs::remove_file(&path).unwrap();
        server.join().unwrap();
        assert!(output.unwrap().contains("Translation: Dog. Cat."));
    }

    #[test]
    fn runs_deterministic_pipeline_and_reports_skips() {
        assert_eq!(
            run("hello", "typed", &no_substitutions()).unwrap(),
            "Original: hello\nProcessed: hello\nStatus: ready"
        );
        assert!(run("   ", "typed", &no_substitutions()).is_err());
    }
}
