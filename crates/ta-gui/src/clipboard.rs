use std::{
    collections::BTreeMap,
    io::{self, BufRead, Write},
    path::Path,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use ta_core::{
    ContextId, DEFAULT_MAX_INPUT_CHARS, History, HistoryEntryId, ProviderHistory, ProviderId,
    ProviderOutcome, ProviderStatus, TextInput,
};
use ta_ingest::{
    ClipboardGate, FlushedText, InputBuffer, PipelineOptions, PipelineTransforms, PreparedInput,
    SkipReason, TextFilter, prepare_input,
};
use ta_platform::{
    clipboard::{ClipboardListener, clipboard_sequence_number, read_clipboard_change},
    credentials::read_secret,
    load_settings, settings_paths,
};
use ta_provider_web::{
    ProviderError, ValidatedProvider, libretranslate_preset, libretranslate_request, redact_secrets,
};
use ta_substitute::{SubstitutionEngine, SubstitutionProfile};
use ta_translate::{
    ProviderQueue, QueueAction, RequestMode, RequestRejection, ResultCache, ResultCacheKey,
    RoutingRejection, RoutingRequest, TranslatorCapabilities, TranslatorKind,
    routing_request_rejection,
};

const CONTEXT: &str = "clipboard";
pub(super) const USAGE: &str = "       translation-aggregator --clipboard --source LANG --target LANG [--libretranslate-server IP:PORT] [--japanese-only] [--substitute MATCH REPLACEMENT]";

mod history;
mod input_settings;
mod options;
mod pacing;
use history::HistoryDisk;
use input_settings::InputSettings;
use options::{ClipboardOptions, Endpoint};
use pacing::{ProviderPacing, send_with_retries};

struct TranslationJob {
    id: HistoryEntryId,
    text: String,
}

struct ActiveRequest {
    id: HistoryEntryId,
    started: Instant,
    cache_key: Option<ResultCacheKey>,
    worker: JoinHandle<Result<Option<String>, String>>,
    stop_retries: SyncSender<()>,
    waits: Receiver<Duration>,
}

struct ClipboardPipeline {
    gate: ClipboardGate,
    buffer: InputBuffer,
    filter: TextFilter,
    substitutions: SubstitutionEngine,
    history: History,
    history_disk: Option<HistoryDisk>,
    history_dirty: bool,
    cache: ResultCache,
    cache_enabled: bool,
    options: PipelineOptions,
    auto_translate: bool,
    max_input_chars: usize,
    provider: ValidatedProvider,
    provider_enabled: bool,
    params: BTreeMap<String, String>,
    capabilities: TranslatorCapabilities,
    credential: Option<String>,
    read_key: fn(&str) -> io::Result<Option<String>>,
    queue: ProviderQueue<TranslationJob>,
    active: Option<ActiveRequest>,
    pacing: Arc<Mutex<ProviderPacing>>,
    source: String,
    target: String,
    started: Instant,
}

impl ClipboardPipeline {
    fn new(options: ClipboardOptions) -> Self {
        let mut provider = libretranslate_preset();
        if let Some(timeout) = options.endpoint.timeout_seconds {
            provider = provider.with_timeout_seconds(timeout);
        }
        let configured = &provider.definition().provider;
        let mut capabilities = TranslatorCapabilities::new(TranslatorKind::Remote);
        capabilities.id = configured.id.clone();
        capabilities.name = configured.name.clone();
        capabilities.remote = false;
        capabilities.detects_source_language = true;
        capabilities.max_chars = Some(configured.max_chars.min(options.endpoint.auto_max_chars));

        Self {
            gate: ClipboardGate::new(),
            buffer: InputBuffer::new(),
            filter: TextFilter::new(),
            substitutions: SubstitutionEngine::new(
                !options.rules.is_empty(),
                vec![SubstitutionProfile::new(CONTEXT).with_rules(options.rules)],
            ),
            history: History::default(),
            history_disk: None,
            history_dirty: false,
            cache: ResultCache::new(),
            // The configured credential can change independently of the settings file.
            cache_enabled: options.endpoint.cache && options.endpoint.credential.is_none(),
            options: PipelineOptions {
                japanese_only: options.japanese_only,
                suppress_repeats: true,
                ..PipelineOptions::default()
            },
            auto_translate: true,
            max_input_chars: DEFAULT_MAX_INPUT_CHARS,
            provider,
            provider_enabled: options.endpoint.server.is_some(),
            params: options
                .endpoint
                .server
                .map(|server| ("server".into(), format!("http://{server}")))
                .into_iter()
                .collect(),
            capabilities,
            credential: options.endpoint.credential,
            read_key: read_secret,
            queue: ProviderQueue::new(),
            active: None,
            pacing: Arc::new(Mutex::new(ProviderPacing::new(options.endpoint.rate_limit))),
            source: options.source,
            target: options.target,
            started: Instant::now(),
        }
    }

    fn handle_change(
        &mut self,
        sequence: u32,
        text: Option<&str>,
        output: &mut impl Write,
    ) -> io::Result<()> {
        self.handle_change_at(sequence, text, self.now_ms(), output)
    }

    fn configure_input(&mut self, mut input: InputSettings) {
        if !input.buffer.enabled {
            self.gate.disable();
        }
        self.buffer.configure(CONTEXT, input.buffer);
        input.pipeline.japanese_only |= self.options.japanese_only;
        self.options = input.pipeline;
        self.auto_translate = input.automatic;
        self.max_input_chars = input.max_chars;
    }

    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn handle_change_at(
        &mut self,
        sequence: u32,
        text: Option<&str>,
        now_ms: u64,
        output: &mut impl Write,
    ) -> io::Result<()> {
        self.flush_buffer_at(now_ms, output)?;
        let Some(text) = self.gate.accept(sequence, text.unwrap_or_default(), false) else {
            return Ok(());
        };
        let piece_chars = text.chars().count();
        if piece_chars > self.max_input_chars {
            return writeln!(
                output,
                "Status: skipped (input too long: {piece_chars} characters, limit {})",
                self.max_input_chars
            );
        }
        let chars = self
            .buffer
            .pending_chars(CONTEXT)
            .saturating_add(piece_chars);
        if chars > self.max_input_chars {
            self.buffer.discard_pending(CONTEXT);
            return writeln!(
                output,
                "Status: skipped (buffer too long: {chars} characters, limit {})",
                self.max_input_chars
            );
        }
        let flushed = self.buffer.push(CONTEXT, text, now_ms);
        self.process_flushed(flushed, output)
    }

    fn flush_buffer_at(&mut self, now_ms: u64, output: &mut impl Write) -> io::Result<()> {
        if self
            .buffer
            .next_flush_at(CONTEXT)
            .is_some_and(|deadline| deadline <= now_ms)
        {
            let flushed = self.buffer.flush_due(now_ms);
            self.process_flushed(flushed, output)?;
        }
        Ok(())
    }

    fn process_flushed(
        &mut self,
        entries: Vec<FlushedText>,
        output: &mut impl Write,
    ) -> io::Result<()> {
        for flushed in entries {
            let Ok(input) = TextInput::with_max_chars(&flushed.text, self.max_input_chars) else {
                continue;
            };
            let context =
                ContextId::new(flushed.context).expect("buffer uses the clipboard context");
            let prepared = prepare_input(
                input,
                context,
                &mut self.filter,
                &self.substitutions,
                &mut self.history,
                self.options,
                PipelineTransforms {
                    pre_substitution: |s: &str| Ok(s.to_owned()),
                    post_substitution: |s: &str| Ok(s.to_owned()),
                },
            );
            match prepared {
                PreparedInput::Ready(input) => {
                    self.history_dirty = true;
                    writeln!(
                        output,
                        "Entry: {}\nContext: {}\nOriginal: {}\nProcessed: {}",
                        input.history_id,
                        input.context.as_str(),
                        input.original_text,
                        input.processed_text
                    )?;
                    output.flush()?;
                    self.translate(input.history_id, input.processed_text, output)?;
                }
                PreparedInput::Skipped(SkipReason::EmptyAfterSubstitutions) => {
                    self.history_dirty = true;
                    writeln!(
                        output,
                        "Context: {CONTEXT}\nOriginal: {}\nStatus: skipped (empty after substitutions)",
                        flushed.text
                    )?;
                }
                PreparedInput::Skipped(_) => {}
            }
        }
        Ok(())
    }

    fn translate(
        &mut self,
        id: HistoryEntryId,
        text: String,
        output: &mut impl Write,
    ) -> io::Result<()> {
        let rejection = routing_request_rejection(RoutingRequest {
            capabilities: &self.capabilities,
            provider_enabled: self.provider_enabled,
            credentials_available: true,
            remote_permission: false,
            auto_translate_enabled: self.auto_translate,
            source: &self.source,
            target: &self.target,
            text: &text,
            mode: RequestMode::Automatic,
        });
        match rejection {
            Some(RoutingRejection::ProviderDisabled) => {
                return writeln!(output, "Entry: {id}\nStatus: Add a translator");
            }
            Some(RoutingRejection::AutoTranslateDisabled) => {
                return writeln!(output, "Entry: {id}\nStatus: Automatic translation is off");
            }
            Some(RoutingRejection::Provider(RequestRejection::TooLong {
                actual_chars,
                max_chars,
            })) => {
                return writeln!(
                    output,
                    "Entry: {id}\nStatus: skipped (Too long for automatic translation ({actual_chars} characters, limit {max_chars}))"
                );
            }
            Some(reason) => return writeln!(output, "Entry: {id}\nStatus: skipped ({reason:?})"),
            None => {}
        }
        if let Some(active) = &self.active {
            let _ = active.stop_retries.try_send(());
        }
        match self.queue.submit(TranslationJob { id, text }) {
            QueueAction::Start(job) => self.start(job, output),
            QueueAction::Queued => Ok(()),
            QueueAction::Replaced(job) => {
                self.report_skipped(job.id, "newer clipboard input", output)
            }
        }
    }

    fn start(&mut self, job: TranslationJob, output: &mut impl Write) -> io::Result<()> {
        self.start_request(job, output)?;
        if self.active.is_none() {
            let next = self.queue.complete();
            debug_assert!(next.is_none(), "a fresh start cannot have queued work");
        }
        Ok(())
    }

    fn start_request(&mut self, job: TranslationJob, output: &mut impl Write) -> io::Result<()> {
        let TranslationJob { id, text } = job;
        let started = Instant::now();
        let cache_key = self.cache_enabled.then(|| ResultCacheKey {
            provider_id: self.capabilities.id.clone(),
            processed_text: text.clone(),
            source: self.source.clone(),
            target: self.target.clone(),
            settings_fingerprint: self.params["server"].clone(),
        });
        if let Some(cached) = cache_key
            .as_ref()
            .and_then(|key| self.cache.get(key, true))
            .map(str::to_owned)
        {
            return self.report_completion(&id, Ok(cached), started.elapsed(), true, output);
        }

        let secrets = match self.credential.as_deref() {
            Some(name) => match (self.read_key)(name) {
                Ok(Some(key)) => vec![(name.to_owned(), key)],
                Ok(None) => {
                    return self.report_completion(
                        &id,
                        Err("needs key".into()),
                        started.elapsed(),
                        false,
                        output,
                    );
                }
                Err(_) => {
                    return self.report_completion(
                        &id,
                        Err("needs key (credential store unavailable)".into()),
                        started.elapsed(),
                        false,
                        output,
                    );
                }
            },
            None => Vec::new(),
        };
        let request = libretranslate_request(
            &self.provider,
            &text,
            &self.source,
            &self.target,
            &self.params,
            secrets.first().map(|(_, key)| key.as_str()),
        );
        let provider = self.provider.clone();
        let (stop_retries, newer_entry) = mpsc::sync_channel(1);
        let (wait_tx, waits) = mpsc::channel();
        let pacing = Arc::clone(&self.pacing);
        let jitter_seed = self.started.elapsed().as_nanos() as u64;
        let worker = thread::Builder::new()
            .name("libretranslate".into())
            .spawn(move || {
                request
                    .and_then(|request| {
                        send_with_retries(
                            &provider,
                            &request,
                            &newer_entry,
                            &pacing,
                            &wait_tx,
                            jitter_seed,
                        )
                    })
                    .map(|translation| translation.map(|text| redact_secrets(&text, &secrets)))
                    .map_err(|error| match error {
                        ProviderError::Timeout => format!(
                            "timed out after {} s",
                            provider.definition().provider.timeout_seconds
                        ),
                        _ => format!("error ({})", redact_secrets(&error.to_string(), &secrets)),
                    })
            });
        match worker {
            Ok(worker) => {
                self.active = Some(ActiveRequest {
                    id,
                    started,
                    cache_key,
                    worker,
                    stop_retries,
                    waits,
                });
                Ok(())
            }
            Err(_) => self.report_completion(
                &id,
                Err("error (could not start worker)".into()),
                started.elapsed(),
                false,
                output,
            ),
        }
    }

    fn poll_completion(&mut self, output: &mut impl Write) -> io::Result<()> {
        if let Some(active) = &self.active {
            for delay in active.waits.try_iter() {
                let seconds = delay.as_secs() + u64::from(delay.subsec_nanos() > 0);
                writeln!(
                    output,
                    "Entry: {}\nStatus: Waiting {seconds}s (rate limit)",
                    active.id
                )?;
            }
        }
        if !self
            .active
            .as_ref()
            .is_some_and(|active| active.worker.is_finished())
        {
            return Ok(());
        }
        let ActiveRequest {
            id,
            started,
            cache_key,
            worker,
            stop_retries: _,
            waits: _,
        } = self.active.take().expect("completed request");
        let result = worker
            .join()
            .unwrap_or_else(|_| Err("error (worker stopped)".into()));
        if let (Some(key), Ok(Some(translation))) = (cache_key, &result) {
            self.cache.insert_success(key, translation.clone(), true);
        }
        match result {
            Ok(None) => self.report_skipped(id, "automatic request withdrawn", output)?,
            Ok(Some(translation)) => {
                self.report_completion(&id, Ok(translation), started.elapsed(), false, output)?
            }
            Err(error) => {
                self.report_completion(&id, Err(error), started.elapsed(), false, output)?
            }
        }
        if let Some(next) = self.queue.complete() {
            self.start(next, output)?;
        }
        Ok(())
    }

    fn provider_id(&self) -> ProviderId {
        ProviderId::new(self.capabilities.id.as_str()).expect("preset has an ID")
    }

    fn report_skipped(
        &mut self,
        id: HistoryEntryId,
        reason: &str,
        output: &mut impl Write,
    ) -> io::Result<()> {
        self.history_dirty = true;
        self.history.record_provider(
            &id,
            self.provider_id(),
            ProviderHistory {
                outcome: ProviderOutcome::Skipped,
                status: ProviderStatus::Skipped,
                elapsed: Duration::ZERO,
                cached: false,
            },
        );
        writeln!(output, "Entry: {id}\nStatus: skipped ({reason})")
    }

    fn pause(&mut self, output: &mut impl Write) -> io::Result<()> {
        self.gate.pause();
        self.buffer.discard_pending(CONTEXT);
        if let Some(active) = &self.active {
            let _ = active.stop_retries.try_send(());
        }
        if let Some(job) = self.queue.discard_pending() {
            self.report_skipped(job.id, "automatic input paused", output)?;
        }
        writeln!(output, "Clipboard capture paused.")
    }

    fn process_commands(
        &mut self,
        commands: &Receiver<String>,
        output: &mut impl Write,
    ) -> io::Result<bool> {
        for command in commands.try_iter() {
            match command.trim() {
                "pause" => self.pause(output)?,
                "resume" => match clipboard_sequence_number() {
                    Ok(sequence) => {
                        self.gate.resume_at(sequence);
                        writeln!(output, "Clipboard capture resumed.")?;
                    }
                    Err(error) => writeln!(output, "Could not resume clipboard capture: {error}")?,
                },
                "quit" => return Ok(true),
                "" => {}
                _ => writeln!(output, "Commands: pause, resume, quit")?,
            }
        }
        Ok(false)
    }

    fn report_completion(
        &mut self,
        history_id: &HistoryEntryId,
        result: Result<String, String>,
        elapsed: Duration,
        cached: bool,
        output: &mut impl Write,
    ) -> io::Result<()> {
        let provider = self.provider_id();
        let (outcome, status, display) = match result {
            Ok(translation) => {
                let state = if cached { "cached" } else { "ready" };
                let display = writeln!(
                    output,
                    "Entry: {history_id}\nTranslation: {translation}\nStatus: {state}"
                );
                (
                    ProviderOutcome::Result(translation),
                    ProviderStatus::Succeeded,
                    display,
                )
            }
            Err(error) => {
                let display = writeln!(
                    output,
                    "Entry: {history_id}\nStatus: LibreTranslate {error}"
                );
                (
                    ProviderOutcome::Error(error),
                    ProviderStatus::Failed,
                    display,
                )
            }
        };
        self.history.record_provider(
            history_id,
            provider,
            ProviderHistory {
                outcome,
                status,
                elapsed,
                cached,
            },
        );
        self.history_dirty = true;
        display
    }

    fn open_history(&mut self, dir: &Path, keep: bool) -> io::Result<()> {
        (self.history, self.history_disk) = HistoryDisk::open(dir, keep)?;
        self.history_dirty = false;
        Ok(())
    }

    fn flush_history(&mut self) -> io::Result<()> {
        if self.history_dirty
            && let Some(disk) = &self.history_disk
            && let Err(error) = disk.save(&self.history)
        {
            self.history_disk = None;
            return Err(error);
        }
        self.history_dirty = false;
        Ok(())
    }
}

pub(super) fn run(args: &[String]) -> Result<(), String> {
    let mut history_settings = None;
    let mut input_settings = InputSettings::default();
    let options = ClipboardOptions::parse(args, || {
        let paths = settings_paths().map_err(|error| format!("settings path: {error}"))?;
        let settings =
            load_settings(&paths.settings_file).map_err(|error| format!("settings: {error}"))?;
        let keep_history = settings
            .document
            .value()
            .get("history")
            .and_then(|history| history.get("keep_between_sessions"))
            .map(|value| {
                value
                    .as_bool()
                    .ok_or("history.keep_between_sessions must be a boolean")
            })
            .transpose()?
            .unwrap_or(false);
        history_settings = Some((paths.data_dir, keep_history));
        input_settings = InputSettings::from_document(&settings.document)?;
        Endpoint::from_settings(&settings.document)
    })
    .map_err(|error| format!("{error}\n{USAGE}"))?;
    let mut pipeline = ClipboardPipeline::new(options);
    let capture_enabled = input_settings.buffer.enabled;
    pipeline.configure_input(input_settings);
    if let Some((dir, keep)) = history_settings
        && let Err(error) = pipeline.open_history(&dir, keep)
    {
        eprintln!("history storage unavailable: {error}");
    }
    if !capture_enabled {
        println!("Clipboard capture is disabled in settings.");
        return Ok(());
    }
    let listener =
        ClipboardListener::new().map_err(|error| format!("clipboard listener: {error}"))?;
    let (command_tx, commands) = mpsc::sync_channel(16);
    thread::Builder::new()
        .name("clipboard-commands".into())
        .spawn(move || {
            for line in io::stdin().lock().lines().map_while(Result::ok) {
                if command_tx.send(line).is_err() {
                    break;
                }
            }
        })
        .map_err(|error| format!("console commands: {error}"))?;
    let mut output = io::stdout().lock();
    writeln!(output, "Watching for clipboard changes (Ctrl+C to stop).")
        .and_then(|()| output.flush())
        .map_err(|error| format!("console output: {error}"))?;
    loop {
        if pipeline
            .process_commands(&commands, &mut output)
            .map_err(|error| format!("console output: {error}"))?
        {
            break;
        }
        pipeline
            .flush_buffer_at(pipeline.now_ms(), &mut output)
            .map_err(|error| format!("console output: {error}"))?;
        pipeline
            .poll_completion(&mut output)
            .map_err(|error| format!("console output: {error}"))?;
        if let Err(error) = pipeline.flush_history() {
            eprintln!("history storage unavailable: {error}");
        }
        let mut timeout = if pipeline.active.is_some() {
            Duration::from_millis(10)
        } else {
            Duration::from_millis(100)
        };
        if let Some(due) = pipeline.buffer.next_flush_at(CONTEXT) {
            timeout = timeout.min(Duration::from_millis(due.saturating_sub(pipeline.now_ms())));
        }
        let Some(sequence) = listener
            .wait_for_change(timeout)
            .map_err(|error| format!("clipboard listener: {error}"))?
        else {
            continue;
        };
        if pipeline
            .process_commands(&commands, &mut output)
            .map_err(|error| format!("console output: {error}"))?
        {
            break;
        }
        if !pipeline.gate.can_read(sequence) {
            continue;
        }
        let text = match read_clipboard_change(sequence) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("clipboard change skipped: {error}");
                continue;
            }
        };
        if pipeline
            .process_commands(&commands, &mut output)
            .map_err(|error| format!("console output: {error}"))?
        {
            break;
        }
        pipeline
            .handle_change(sequence, text.as_deref(), &mut output)
            .map_err(|error| format!("console output: {error}"))?;
    }
    pipeline
        .flush_history()
        .map_err(|error| format!("history storage: {error}"))
}

#[cfg(test)]
pub(super) mod tests;
