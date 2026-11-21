use super::*;
use std::{
    io::{BufRead, BufReader, Read},
    net::TcpListener,
    thread,
};
use ta_core::{ProviderId, ProviderOutcome, ProviderStatus};
use ta_platform::SettingsDocument;
use ta_substitute::SubstitutionRule;

#[test]
fn clipboard_pipeline_posts_only_eligible_processed_text() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let server = listener.local_addr().unwrap();
    let mut pipeline = ClipboardPipeline::new(ClipboardOptions {
        endpoint: Endpoint::keyless(server),
        source: "ja".into(),
        target: "en".into(),
        japanese_only: true,
        rules: vec![
            SubstitutionRule::try_new("猫", "犬").unwrap(),
            SubstitutionRule::try_new("削除", "").unwrap(),
        ],
    });
    let mut output = Vec::new();

    listener.set_nonblocking(true).unwrap();
    for (sequence, text) in [
        (1, None),
        (2, Some("   ")),
        (3, Some("English only")),
        (4, Some("猫".repeat(501).as_str())),
        (5, Some("削除")),
    ] {
        pipeline.handle_change(sequence, text, &mut output).unwrap();
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
    assert!(!String::from_utf8_lossy(&output).contains("English only"));
    assert!(String::from_utf8_lossy(&output).contains("Too long"));
    assert!(String::from_utf8_lossy(&output).contains("empty after substitutions"));

    let reply = mock_reply(&listener, r#"{"translatedText":"It is a dog."}"#);

    output.clear();
    pipeline
        .handle_change(6, Some("猫です"), &mut output)
        .unwrap();
    drain(&mut pipeline, &mut output);
    let body = reply.join().unwrap();
    assert!(body.contains(r#""q":"犬です""#), "{body}");
    assert!(body.contains(r#""source":"ja""#), "{body}");
    assert!(body.contains(r#""target":"en""#), "{body}");
    assert!(!body.contains("api_key"), "{body}");
    let fetched = pipeline.history.iter().next_back().unwrap();
    let id = fetched.id;
    let text = String::from_utf8(output).unwrap();
    assert!(text.starts_with(&format!(
        "Entry: {id}\nContext: clipboard\nOriginal: 猫です\nProcessed: 犬です\nEntry: {id}\nProvider: libretranslate\nElapsed: "
    )));
    assert!(text.ends_with("Cached: false\nTranslation: It is a dog.\nStatus: ready\n"));
    assert_eq!(fetched.original_text, "猫です");
    assert_eq!(fetched.processed_text, "犬です");
    let provider_id = ta_core::ProviderId::new("libretranslate").unwrap();
    let fetched_result = &fetched.providers[&provider_id];
    assert_eq!(
        fetched_result.outcome,
        ta_core::ProviderOutcome::Result("It is a dog.".into())
    );
    assert_eq!(fetched_result.status, ta_core::ProviderStatus::Succeeded);
    assert!(!fetched_result.cached);
    for sequence in [6, 7] {
        pipeline
            .handle_change(sequence, Some("猫です"), &mut Vec::new())
            .unwrap();
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
    let mut output = Vec::new();
    pipeline
        .handle_change(8, Some("犬です"), &mut output)
        .unwrap();
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert!(
        String::from_utf8(output)
            .unwrap()
            .ends_with("Translation: It is a dog.\nStatus: cached\n")
    );
    let cached = pipeline.history.iter().next_back().unwrap();
    assert_eq!(cached.original_text, "犬です");
    assert!(cached.providers[&provider_id].cached);
}

fn mock_reply(listener: &TcpListener, response: &'static str) -> thread::JoinHandle<String> {
    let listener = listener.try_clone().unwrap();
    thread::spawn(move || {
        let (mut stream, body) = accept_request(&listener);
        respond(&mut stream, response);
        body
    })
}

pub(crate) fn expect_request_and_reply(listener: &TcpListener, text: &str, response: &str) {
    let (mut connection, body) = accept_request(listener);
    assert!(body.contains(text), "{body}");
    respond(&mut connection, response);
}

fn accept_request(listener: &TcpListener) -> (std::net::TcpStream, String) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("no eligible request: {e}"),
        }
    };
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut reader = BufReader::new(&mut stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert_eq!(line, "POST /translate HTTP/1.1\r\n");
    let mut content_length = None;
    loop {
        line.clear();
        assert_ne!(reader.read_line(&mut line).unwrap(), 0);
        if line == "\r\n" {
            break;
        }
        let (header, value) = line.split_once(':').unwrap();
        if header.eq_ignore_ascii_case("content-length") {
            content_length = Some(value.trim().parse().unwrap());
        }
    }
    let mut body = vec![0; content_length.unwrap()];
    reader.read_exact(&mut body).unwrap();
    let body = String::from_utf8(body).unwrap();
    (stream, body)
}

fn respond(stream: &mut std::net::TcpStream, response: &str) {
    respond_status(stream, "200 OK", "", response);
}

fn respond_status(stream: &mut std::net::TcpStream, status: &str, headers: &str, response: &str) {
    write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{response}", response.len()).unwrap();
}

#[test]
fn clipboard_requires_an_explicit_loopback_server_and_languages() {
    let args = |values: &[&str]| values.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let absent = || Err("settings not configured".into());
    assert!(ClipboardOptions::parse(&[], absent).is_err());
    for server in ["192.0.2.5:5000", "127.0.0.1:0"] {
        assert!(
            ClipboardOptions::parse(
                &args(&[
                    "--libretranslate-server",
                    server,
                    "--source",
                    "ja",
                    "--target",
                    "en"
                ]),
                absent
            )
            .is_err()
        );
    }
    assert!(
        ClipboardOptions::parse(
            &args(&[
                "--libretranslate-server",
                "[::1]:5000",
                "--source",
                "ja",
                "--target",
                "en",
                "--substitute",
                "猫",
                "犬"
            ]),
            || panic!("the explicit server must ignore settings and credentials")
        )
        .is_ok()
    );
}

#[test]
fn clipboard_reads_settings_and_fails_closed_for_a_missing_key() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = listener.local_addr().unwrap();
    let settings = SettingsDocument::parse(&format!(
        "[providers.libretranslate]\nserver = '{server}'\ncredential = 'libre-key'"
    ))
    .unwrap();
    let args = ["--source", "ja", "--target", "en"].map(str::to_owned);
    let options = ClipboardOptions::parse(&args, || Endpoint::from_settings(&settings)).unwrap();
    assert_eq!(options.endpoint.server, Some(server));
    let mut pipeline = ClipboardPipeline::new(options);
    let history_id = pipeline.history.push(ta_core::HistoryEntry::new(
        HistoryEntryId::new(),
        std::time::SystemTime::now(),
        ContextId::new(CONTEXT).unwrap(),
        "こんにちは",
        "こんにちは",
    ));
    let mut output = Vec::new();
    pipeline.providers[0].read_key = |name| {
        assert_eq!(name, "libre-key");
        Ok(None)
    };
    pipeline
        .translate(history_id, "こんにちは".into(), &mut output)
        .unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.starts_with(&format!("Entry: {history_id}\nProvider: libretranslate\n")));
    assert!(text.ends_with("Status: LibreTranslate needs key\n"));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    let reply = mock_reply(&listener, r#"{"error":"Rejected canary-api-key"}"#);
    let mut output = Vec::new();
    pipeline.providers[0].read_key = |_| Ok(Some("canary-api-key".into()));
    pipeline
        .translate(history_id, "こんにちは".into(), &mut output)
        .unwrap();
    drain(&mut pipeline, &mut output);
    assert!(
        reply
            .join()
            .unwrap()
            .contains(r#""api_key":"canary-api-key""#)
    );
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("LibreTranslate error"), "{output}");
    assert!(!output.contains("canary-api-key"), "{output}");
    let result = &pipeline.history.get(&history_id).unwrap().providers
        [&ProviderId::new("libretranslate").unwrap()];
    let ProviderOutcome::Error(message) = &result.outcome else {
        panic!("expected a stored provider error");
    };
    assert!(message.contains("Rejected"));
    assert!(!message.contains("canary-api-key"));
    assert_eq!(result.status, ProviderStatus::Failed);
    assert!(!result.cached);
}

#[test]
fn settings_control_the_automatic_cap_and_cache() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = listener.local_addr().unwrap();
    let settings = SettingsDocument::parse(&format!(
        "[providers.libretranslate]\nserver = '{server}'\nauto_max_chars = 2\ncache = false\ntimeout_seconds = 2"
    ))
    .unwrap();
    let args = ["--source", "ja", "--target", "en"].map(str::to_owned);
    let options = ClipboardOptions::parse(&args, || Endpoint::from_settings(&settings)).unwrap();
    let mut pipeline = ClipboardPipeline::new(options);
    let mut output = Vec::new();
    pipeline
        .handle_change(1, Some("猫𠮷犬"), &mut output)
        .unwrap();
    assert_eq!(
        pipeline.providers[0]
            .provider
            .definition()
            .provider
            .timeout_seconds,
        2
    );
    assert!(
        String::from_utf8(output)
            .unwrap()
            .contains("3 characters, limit 2")
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    for (sequence, text) in [(2, "猫𠮷"), (3, "犬𠮷"), (4, "猫𠮷")] {
        let reply = mock_reply(&listener, r#"{"translatedText":"OK"}"#);
        let mut output = Vec::new();
        pipeline
            .handle_change(sequence, Some(text), &mut output)
            .unwrap();
        drain(&mut pipeline, &mut output);
        assert!(reply.join().unwrap().contains(text));
        assert!(
            String::from_utf8(output)
                .unwrap()
                .ends_with("Status: ready\n")
        );
    }
    assert!(pipeline.providers[0].cache.is_empty());
}

#[test]
fn busy_provider_finishes_first_and_translates_only_the_newest_waiting_entry() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut pipeline = ClipboardPipeline::new(ClipboardOptions {
        endpoint: Endpoint::keyless(listener.local_addr().unwrap()),
        source: "ja".into(),
        target: "en".into(),
        japanese_only: false,
        rules: vec![],
    });
    let mut output = Vec::new();
    pipeline
        .handle_change(1, Some("first"), &mut output)
        .unwrap();
    let (mut first, body) = accept_request(&listener);
    assert!(body.contains(r#""q":"first""#));

    for sequence in 2..=20 {
        let text = format!("line {sequence}");
        pipeline
            .handle_change(sequence, Some(&text), &mut output)
            .unwrap();
    }
    assert!(String::from_utf8_lossy(&output).contains("Original: line 20"));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    respond(&mut first, r#"{"translatedText":"first result"}"#);
    let newest = mock_reply(&listener, r#"{"translatedText":"newest result"}"#);
    drain(&mut pipeline, &mut output);
    assert!(newest.join().unwrap().contains(r#""q":"line 20""#));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );

    let provider = ProviderId::new("libretranslate").unwrap();
    let results: Vec<_> = pipeline
        .history
        .iter()
        .map(|e| &e.providers[&provider])
        .collect();
    assert_eq!(results.len(), 20);
    assert_eq!(
        results[0].outcome,
        ProviderOutcome::Result("first result".into())
    );
    assert!(
        results[1..19]
            .iter()
            .all(|r| r.status == ProviderStatus::Skipped && !r.cached)
    );
    assert_eq!(
        results[19].outcome,
        ProviderOutcome::Result("newest result".into())
    );
    let snapshot = ta_core::encode_history_snapshot(&pipeline.history).unwrap();
    let restored = ta_core::decode_history_snapshot(&snapshot, 20).unwrap();
    assert_eq!(
        restored.iter().nth(1).unwrap().providers[&provider].outcome,
        ProviderOutcome::Skipped
    );
}

#[test]
fn libretranslate_retries_transient_service_errors_but_not_authentication_errors() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut pipeline = ClipboardPipeline::new(ClipboardOptions {
        endpoint: Endpoint::keyless(listener.local_addr().unwrap()),
        source: "ja".into(),
        target: "en".into(),
        japanese_only: false,
        rules: vec![],
    });
    let mut output = Vec::new();
    pipeline
        .handle_change(1, Some("retry me"), &mut output)
        .unwrap();
    for _ in 0..2 {
        let (mut stream, body) = accept_request(&listener);
        assert!(body.contains("retry me"));
        respond_status(
            &mut stream,
            "503 Service Unavailable",
            "Retry-After: 0\r\n",
            r#"{"error":"busy"}"#,
        );
    }
    let (mut stream, body) = accept_request(&listener);
    assert!(body.contains("retry me"));
    respond(&mut stream, r#"{"translatedText":"recovered"}"#);
    drain(&mut pipeline, &mut output);
    assert!(String::from_utf8_lossy(&output).contains("Translation: recovered"));

    pipeline
        .handle_change(2, Some("unauthorized"), &mut output)
        .unwrap();
    let (mut stream, _) = accept_request(&listener);
    respond_status(
        &mut stream,
        "401 Unauthorized",
        "Retry-After: 0\r\n",
        r#"{"error":"key rejected"}"#,
    );
    drain(&mut pipeline, &mut output);
    assert!(String::from_utf8_lossy(&output).contains("key rejected"));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

#[test]
fn newer_text_interrupts_a_retry_after_wait() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut pipeline = ClipboardPipeline::new(ClipboardOptions {
        endpoint: Endpoint::keyless(listener.local_addr().unwrap()),
        source: "ja".into(),
        target: "en".into(),
        japanese_only: false,
        rules: vec![],
    });
    let mut output = Vec::new();
    pipeline.handle_change(1, Some("old"), &mut output).unwrap();
    let (mut stream, _) = accept_request(&listener);
    respond_status(
        &mut stream,
        "503 Service Unavailable",
        "Retry-After: 10\r\n",
        r#"{"error":"limited"}"#,
    );
    pipeline.handle_change(2, Some("new"), &mut output).unwrap();
    let newest = mock_reply(&listener, r#"{"translatedText":"new result"}"#);
    drain(&mut pipeline, &mut output);
    assert!(newest.join().unwrap().contains(r#""q":"new""#));
    assert!(String::from_utf8_lossy(&output).contains("Translation: new result"));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

fn drain(pipeline: &mut ClipboardPipeline, output: &mut Vec<u8>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while pipeline.has_active_request() {
        pipeline.poll_completion(output).unwrap();
        assert!(Instant::now() < deadline, "provider did not finish");
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn configured_rate_limit_waits_and_replaces_an_unsent_entry() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let settings = SettingsDocument::parse(&format!(
        "[providers.libretranslate]\nserver = '{}'\nrate_limit = {{ requests = 1, per_seconds = 1 }}",
        listener.local_addr().unwrap()
    )).unwrap();
    let args = ["--source", "ja", "--target", "en"].map(str::to_owned);
    let mut pipeline = ClipboardPipeline::new(
        ClipboardOptions::parse(&args, || Endpoint::from_settings(&settings)).unwrap(),
    );
    let mut output = Vec::new();
    pipeline
        .handle_change(1, Some("first"), &mut output)
        .unwrap();
    let (mut first, _) = accept_request(&listener);
    respond(&mut first, r#"{"translatedText":"first result"}"#);
    drain(&mut pipeline, &mut output);

    pipeline
        .handle_change(2, Some("waiting"), &mut output)
        .unwrap();
    let deadline = Instant::now() + Duration::from_millis(500);
    while !String::from_utf8_lossy(&output).contains("rate limit") {
        pipeline.poll_completion(&mut output).unwrap();
        assert!(Instant::now() < deadline, "the second entry should wait");
        thread::sleep(Duration::from_millis(5));
    }
    pipeline
        .handle_change(3, Some("newest"), &mut output)
        .unwrap();
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    let newest = mock_reply(&listener, r#"{"translatedText":"last result"}"#);
    drain(&mut pipeline, &mut output);
    assert!(newest.join().unwrap().contains(r#""q":"newest""#));
    let provider = ProviderId::new("libretranslate").unwrap();
    assert_eq!(
        pipeline.history.iter().nth(1).unwrap().providers[&provider].status,
        ProviderStatus::Skipped
    );
}

#[test]
fn retry_after_cooldown_also_blocks_new_entries_after_http_429() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut pipeline = ClipboardPipeline::new(ClipboardOptions {
        endpoint: Endpoint::keyless(listener.local_addr().unwrap()),
        source: "ja".into(),
        target: "en".into(),
        japanese_only: false,
        rules: vec![],
    });
    let mut output = Vec::new();
    pipeline.handle_change(1, Some("old"), &mut output).unwrap();
    let (mut connection, _) = accept_request(&listener);
    let since = Instant::now();
    respond_status(
        &mut connection,
        "429 Too Many Requests",
        "Retry-After: 1\r\n",
        r#"{"error":"limited"}"#,
    );
    pipeline.handle_change(2, Some("new"), &mut output).unwrap();
    let newest = mock_reply(&listener, r#"{"translatedText":"new result"}"#);
    drain(&mut pipeline, &mut output);
    assert!(newest.join().unwrap().contains(r#""q":"new""#));
    assert!(since.elapsed() >= Duration::from_secs(1));
    assert!(String::from_utf8_lossy(&output).contains("rate limit"));
}

#[test]
fn clipboard_history_is_restored_only_when_opted_in() {
    let dir = std::env::temp_dir().join(format!(
        "ta-clipboard-history-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = dir.join("history.json");
    let mut pipeline = ClipboardPipeline::new(ClipboardOptions {
        endpoint: Endpoint::keyless("127.0.0.1:9".parse().unwrap()),
        source: "ja".into(),
        target: "en".into(),
        japanese_only: false,
        rules: vec![SubstitutionRule::try_new("record only", "").unwrap()],
    });
    let mut output = Vec::new();

    pipeline.open_history(&dir, false).unwrap();
    pipeline
        .handle_change(1, Some("record only"), &mut output)
        .unwrap();
    pipeline.flush_history().unwrap();
    assert!(!dir.exists());

    pipeline.open_history(&dir, true).unwrap();
    pipeline
        .handle_change(2, Some(" record only "), &mut output)
        .unwrap();
    pipeline.flush_history().unwrap();
    assert!(path.is_file());
    assert!(
        pipeline.open_history(&dir, true).is_err(),
        "a second writer should fail"
    );
    drop(pipeline);

    let (history, storage) = HistoryDisk::open(&dir, true).unwrap();
    assert_eq!(
        history.iter().next().unwrap().original_text,
        " record only "
    );
    assert_eq!(history.len(), 1);
    drop(storage);
    assert!(HistoryDisk::open(&dir, false).unwrap().0.is_empty());
    assert!(!path.exists());
    assert!(!dir.join("history.json.bak").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn without_a_provider_the_console_stores_original_text_and_offers_setup() {
    let document = SettingsDocument::parse("").unwrap();
    let args = ["--source", "ja", "--target", "en"].map(str::to_owned);
    let options = ClipboardOptions::parse(&args, || Endpoint::from_settings(&document)).unwrap();
    let mut pipeline = ClipboardPipeline::new(options);
    let mut output = Vec::new();
    pipeline
        .handle_change(1, Some("こんにちは"), &mut output)
        .unwrap();
    assert!(!pipeline.has_active_request());
    assert_eq!(
        pipeline.history.iter().next().unwrap().original_text,
        "こんにちは"
    );
    assert!(
        String::from_utf8(output)
            .unwrap()
            .ends_with("Status: Add a translator\n")
    );
}

#[test]
fn pause_discards_waiting_translation_and_resume_ignores_the_old_clipboard() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut pipeline = ClipboardPipeline::new(ClipboardOptions {
        endpoint: Endpoint::keyless(listener.local_addr().unwrap()),
        source: "ja".into(),
        target: "en".into(),
        japanese_only: false,
        rules: vec![],
    });
    let mut output = Vec::new();
    pipeline
        .handle_change(1, Some("first"), &mut output)
        .unwrap();
    let (mut first, _) = accept_request(&listener);
    pipeline
        .handle_change(2, Some("waiting"), &mut output)
        .unwrap();
    pipeline.pause(&mut output).unwrap();
    assert!(!pipeline.gate.can_read(3));
    pipeline
        .handle_change(3, Some("private"), &mut output)
        .unwrap();
    respond(&mut first, r#"{"translatedText":"first result"}"#);
    drain(&mut pipeline, &mut output);
    let provider = ProviderId::new("libretranslate").unwrap();
    assert_eq!(pipeline.history.len(), 2);
    assert_eq!(
        pipeline.history.iter().nth(1).unwrap().providers[&provider].status,
        ProviderStatus::Skipped
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    pipeline.gate.resume_at(3);
    assert!(!pipeline.gate.can_read(3));
    assert!(pipeline.gate.can_read(4));
    assert!(!String::from_utf8(output).unwrap().contains("private"));
}

#[test]
fn input_settings_flush_cleaned_text_after_idle_without_sending_when_disabled() {
    let document = SettingsDocument::parse(
        r#"
[inputs.clipboard]
idle_delay_ms = 40
phrase_extensions = true
repeat_characters = "preset_2"
line_breaks = "remove"
auto_translate = false
max_input_chars = 24
"#,
    )
    .unwrap();
    let mut pipeline = ClipboardPipeline::new(ClipboardOptions {
        endpoint: Endpoint::keyless("127.0.0.1:9".parse().unwrap()),
        source: "ja".into(),
        target: "en".into(),
        japanese_only: false,
        rules: vec![],
    });
    pipeline.configure_input(InputSettings::from_document(&document).unwrap());
    let mut output = Vec::new();
    pipeline
        .handle_change_at(1, Some("ここ"), 0, &mut output)
        .unwrap();
    pipeline
        .handle_change_at(2, Some("ここんん\nにに"), 20, &mut output)
        .unwrap();
    pipeline.flush_buffer_at(59, &mut output).unwrap();
    assert!(pipeline.history.is_empty());
    pipeline.flush_buffer_at(60, &mut output).unwrap();
    assert_eq!(
        pipeline.history.iter().next().unwrap().processed_text,
        "こんに"
    );
    assert!(!pipeline.has_active_request());
    pipeline
        .handle_change_at(3, Some(&"𠮷".repeat(25)), 70, &mut output)
        .unwrap();
    assert!(
        String::from_utf8(output)
            .unwrap()
            .contains("25 characters, limit 24")
    );
}

#[test]
fn configured_providers_run_concurrently_and_manual_all_includes_nonautomatic_provider() {
    let dir = std::env::temp_dir().join(format!(
        "ta-clipboard-providers-{}-{}",
        std::process::id(),
        HistoryEntryId::new(),
    ));
    std::fs::create_dir(&dir).unwrap();
    struct RemoveTestDirectory(std::path::PathBuf);
    impl Drop for RemoveTestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = RemoveTestDirectory(dir.clone());

    let providers = ["slow", "fast-a", "fast-b", "manual-only", "disabled"];
    let listeners = providers.map(|id| {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let enabled = id != "disabled";
        let automatic = id != "manual-only";
        std::fs::write(
            dir.join(format!("{id}.toml")),
            format!(
                r#"[provider]
id = "{id}"
name = "{id}"
enabled = {enabled}
auto_translate = {automatic}
timeout_seconds = 4
[request]
method = "POST"
url = "http://{address}/translate"
body.json = {{ q = "{{{{text}}}}" }}
[response]
text = {{ jsonpath = "$.translatedText" }}
"#,
            ),
        )
        .unwrap();
        listener
    });

    let mut pipeline = ClipboardPipeline::new(ClipboardOptions {
        endpoint: Endpoint::default(),
        source: "ja".into(),
        target: "en".into(),
        japanese_only: false,
        rules: Vec::new(),
    });
    pipeline.load_providers(&dir);
    let mut output = Vec::new();
    pipeline
        .handle_change(1, Some("並列確認"), &mut output)
        .unwrap();
    let (mut slow, body) = accept_request(&listeners[0]);
    assert!(body.contains("並列確認"));
    expect_request_and_reply(&listeners[1], "並列確認", r#"{"translatedText":"fast A"}"#);
    expect_request_and_reply(&listeners[2], "並列確認", r#"{"translatedText":"fast B"}"#);
    let deadline = Instant::now() + Duration::from_secs(2);
    while !(String::from_utf8_lossy(&output).contains("Translation: fast A")
        && String::from_utf8_lossy(&output).contains("Translation: fast B"))
    {
        assert!(
            Instant::now() < deadline,
            "fast providers were held by the slow provider"
        );
        pipeline.poll_completion(&mut output).unwrap();
        thread::sleep(Duration::from_millis(5));
    }
    assert!(!String::from_utf8_lossy(&output).contains("Translation: slow"));
    for listener in &listeners[3..] {
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
    respond(&mut slow, r#"{"translatedText":"slow"}"#);
    drain(&mut pipeline, &mut output);
    let entry = pipeline.history.iter().next_back().unwrap();
    assert_eq!(entry.providers.len(), 3);
    for id in &providers[..3] {
        let result = &entry.providers[&ProviderId::new(*id).unwrap()];
        assert_eq!(result.status, ProviderStatus::Succeeded);
        assert!(!result.cached);
        assert!(result.elapsed > Duration::ZERO);
    }
    let console = String::from_utf8_lossy(&output);
    assert!(console.contains("Provider: fast-a\nElapsed: "));
    assert!(console.contains("Cached: false\nTranslation: fast A\nStatus: ready"));

    pipeline.pause(&mut output).unwrap();
    let (commands, receiver) = mpsc::channel();
    commands.send("translate-all".into()).unwrap();
    assert!(!pipeline.process_commands(&receiver, &mut output).unwrap());
    for listener in &listeners[..4] {
        expect_request_and_reply(listener, "並列確認", r#"{"translatedText":"manual run"}"#);
    }
    drain(&mut pipeline, &mut output);
    assert_eq!(
        listeners[4].accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(
        pipeline.history.iter().next_back().unwrap().providers.len(),
        4
    );
}

#[test]
fn automatic_custom_provider_does_not_send_clipboard_text_off_device() {
    let mut pipeline = ClipboardPipeline::new(ClipboardOptions {
        endpoint: Endpoint::default(),
        source: "ja".into(),
        target: "en".into(),
        japanese_only: false,
        rules: Vec::new(),
    });
    let definition = ProviderDefinition::from_toml(
        r#"
[provider]
id = "off-device"
name = "Off-device test"
enabled = true
auto_translate = true
timeout_seconds = 1
[request]
method = "POST"
url = "https://provider.invalid/translate"
body.json = { q = "{{text}}" }
[response]
text = { jsonpath = "$.translatedText" }
"#,
    )
    .unwrap();
    pipeline
        .providers
        .push(ClipboardProvider::custom(definition, "ja", "en"));
    let mut output = Vec::new();
    pipeline
        .handle_change(1, Some("確認"), &mut output)
        .unwrap();
    drain(&mut pipeline, &mut output);
    let entry = pipeline.history.iter().next_back().unwrap();
    let result = &entry.providers[&ProviderId::new("off-device").unwrap()];
    assert_eq!(result.status, ProviderStatus::Skipped);
    assert!(
        String::from_utf8_lossy(&output)
            .contains("automatic off-device sending needs confirmation")
    );
}

#[test]
fn remote_auto_consent_is_persisted_and_bound_to_provider_and_origin() {
    let dir = std::env::temp_dir().join(format!(
        "ta-remote-auto-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let provider = |id: &str, host: &str, automatic: bool| {
        let definition = ProviderDefinition::from_toml(&format!(
            r#"
[provider]
id = "{id}"
name = "Remote translator"
enabled = true
auto_translate = {automatic}
[request]
method = "POST"
url = "https://{host}/translate?q={{{{text}}}}"
[response]
text = {{ jsonpath = "$.translatedText" }}
"#,
        ))
        .unwrap();
        ClipboardProvider::custom(definition, "ja", "en")
    };
    let mut providers = [provider("remote", "example.invalid", true)];
    consent::configure(&mut providers, &dir, |name, id, origin| {
        assert_eq!(
            (name, id, origin),
            ("Remote translator", "remote", "https://example.invalid")
        );
        Ok(false)
    })
    .unwrap();
    assert!(!provider::permits_automatic_url(
        "https://example.invalid/translate",
        providers[0].approved_origin.as_deref()
    ));

    consent::configure(&mut providers, &dir, |_, _, _| Ok(true)).unwrap();
    assert!(provider::permits_automatic_url(
        "https://EXAMPLE.invalid:443/translate",
        providers[0].approved_origin.as_deref()
    ));
    assert!(!provider::permits_automatic_url(
        "https://other.invalid/translate",
        providers[0].approved_origin.as_deref()
    ));
    assert!(!provider::permits_automatic_url(
        "http://example.invalid/translate",
        providers[0].approved_origin.as_deref()
    ));

    let mut providers = [
        provider("remote", "example.invalid", true),
        provider("other", "example.invalid", true),
    ];
    consent::configure(&mut providers, &dir, |_, id, _| {
        assert_eq!(id, "other");
        Ok(false)
    })
    .unwrap();
    assert!(provider::permits_automatic_url(
        "https://example.invalid/other",
        providers[0].approved_origin.as_deref()
    ));
    assert!(!provider::permits_automatic_url(
        "https://example.invalid/other",
        providers[1].approved_origin.as_deref()
    ));

    let mut providers = [provider("remote", "example.invalid", false)];
    consent::configure(&mut providers, &dir, |_, _, _| {
        panic!("a disabled provider must not prompt")
    })
    .unwrap();
    assert!(providers[0].approved_origin.is_none());

    let mut providers = [provider("remote", "example.invalid", true)];
    let mut asked = false;
    consent::configure(&mut providers, &dir, |_, id, origin| {
        assert_eq!((id, origin), ("remote", "https://example.invalid"));
        asked = true;
        Ok(true)
    })
    .unwrap();
    assert!(
        asked,
        "disabling the provider must remove its previous approval"
    );

    let mut providers = [provider("remote", "different.invalid", true)];
    consent::configure(&mut providers, &dir, |_, _, origin| {
        assert_eq!(origin, "https://different.invalid");
        Ok(false)
    })
    .unwrap();
    assert!(!provider::permits_automatic_url(
        "https://different.invalid/translate",
        providers[0].approved_origin.as_deref()
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn remote_auto_prompt_requires_an_explicit_yes_and_warns_about_text_and_cost() {
    for (line, expected) in [
        (b"yes\n".as_slice(), true),
        (b"\n", false),
        (b"", false),
        (b"y\n", false),
    ] {
        let mut output = Vec::new();
        assert_eq!(
            consent::prompt(
                &mut &line[..],
                &mut output,
                "Example\nTranslator",
                "example",
                "https://example.invalid"
            )
            .unwrap(),
            expected
        );
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains(r"Example\nTranslator"));
        assert!(output.contains("https://example.invalid"));
        assert!(output.contains("Clipboard text leaves this machine"));
        assert!(output.contains("may cost money"));
    }
}
