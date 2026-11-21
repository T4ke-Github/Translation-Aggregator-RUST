use std::{
    io::{self, BufRead, Write},
    path::Path,
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};
use ta_core::{ContextId, DEFAULT_MAX_INPUT_CHARS, History, HistoryEntryId, TextInput};
use ta_ingest::{
    ClipboardGate, FlushedText, InputBuffer, PipelineOptions, PipelineTransforms, PreparedInput,
    SkipReason, TextFilter, prepare_input,
};
use ta_platform::{
    clipboard::{ClipboardListener, clipboard_sequence_number, read_clipboard_change},
    load_settings, settings_paths,
};
use ta_provider_web::ProviderDefinition;
use ta_substitute::SubstitutionEngine;
use ta_translate::RequestMode;

const CONTEXT: &str = "clipboard";
pub(super) const USAGE: &str = "       translation-aggregator --clipboard --source LANG --target LANG [--libretranslate-server IP:PORT] [--japanese-only] [--substitute MATCH REPLACEMENT]";

mod history;
mod input_settings;
mod options;
mod pacing;
mod provider;
use crate::substitution_settings::SubstitutionSettings;
use history::HistoryDisk;
use input_settings::InputSettings;
use options::{ClipboardOptions, Endpoint};
use provider::ClipboardProvider;

struct ClipboardPipeline {
    gate: ClipboardGate,
    buffer: InputBuffer,
    filter: TextFilter,
    substitutions: SubstitutionEngine,
    history: History,
    history_disk: Option<HistoryDisk>,
    history_dirty: bool,
    options: PipelineOptions,
    auto_translate: bool,
    max_input_chars: usize,
    providers: Vec<ClipboardProvider>,
    source: String,
    target: String,
    started: Instant,
}

impl ClipboardPipeline {
    #[cfg(test)]
    fn new(options: ClipboardOptions) -> Self {
        Self::with_substitutions(options, SubstitutionSettings::default())
    }

    fn with_substitutions(options: ClipboardOptions, substitutions: SubstitutionSettings) -> Self {
        let preset =
            ClipboardProvider::libretranslate(options.endpoint, &options.source, &options.target);
        Self {
            gate: ClipboardGate::new(),
            buffer: InputBuffer::new(),
            filter: TextFilter::new(),
            substitutions: substitutions
                .with_rules(CONTEXT, options.rules)
                .into_engine(),
            history: History::default(),
            history_disk: None,
            history_dirty: false,
            options: PipelineOptions {
                japanese_only: options.japanese_only,
                suppress_repeats: true,
                ..PipelineOptions::default()
            },
            auto_translate: true,
            max_input_chars: DEFAULT_MAX_INPUT_CHARS,
            providers: vec![preset],
            source: options.source,
            target: options.target,
            started: Instant::now(),
        }
    }

    fn load_providers(&mut self, path: &Path) {
        match path.try_exists() {
            Ok(true) => {}
            Ok(false) => return,
            Err(error) => {
                eprintln!("providers folder: {error}");
                return;
            }
        }
        let report = ProviderDefinition::load_folder(path);
        for error in report.errors {
            eprintln!("provider: {error}");
        }
        for provider in report.providers {
            if provider.definition().provider.id == "libretranslate" {
                eprintln!("provider: libretranslate is reserved for the built-in preset");
                continue;
            }
            self.providers.push(ClipboardProvider::custom(
                provider,
                &self.source,
                &self.target,
            ));
        }
    }

    fn has_active_request(&self) -> bool {
        self.providers.iter().any(ClipboardProvider::is_active)
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
        self.route(id, &text, RequestMode::Automatic, output)
    }

    fn route(
        &mut self,
        id: HistoryEntryId,
        text: &str,
        mode: RequestMode,
        output: &mut impl Write,
    ) -> io::Result<()> {
        if !self.providers.iter().any(|provider| provider.enabled) {
            return writeln!(output, "Entry: {id}\nStatus: Add a translator");
        }
        if mode == RequestMode::Automatic && !self.auto_translate {
            return writeln!(output, "Entry: {id}\nStatus: Automatic translation is off");
        }
        self.history_dirty = true;
        for provider in &mut self.providers {
            provider.translate(id, text, mode, &mut self.history, output)?;
        }
        Ok(())
    }

    fn poll_completion(&mut self, output: &mut impl Write) -> io::Result<()> {
        for provider in &mut self.providers {
            self.history_dirty |= provider.poll_completion(&mut self.history, output)?;
        }
        Ok(())
    }

    fn pause(&mut self, output: &mut impl Write) -> io::Result<()> {
        self.gate.pause();
        self.buffer.discard_pending(CONTEXT);
        self.history_dirty = true;
        for provider in &mut self.providers {
            provider.pause(&mut self.history, output)?;
        }
        writeln!(output, "Clipboard capture paused.")
    }

    fn translate_all(&mut self, output: &mut impl Write) -> io::Result<()> {
        let Some((id, text)) = self
            .history
            .iter()
            .next_back()
            .map(|entry| (entry.id, entry.processed_text.clone()))
        else {
            return writeln!(output, "Status: No captured text to translate");
        };
        if text.is_empty() {
            return writeln!(output, "Status: No captured text to translate");
        }
        self.route(id, &text, RequestMode::Manual, output)
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
                "translate-all" => self.translate_all(output)?,
                "quit" => return Ok(true),
                "" => {}
                _ => writeln!(output, "Commands: pause, resume, translate-all, quit")?,
            }
        }
        Ok(false)
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
    let mut providers_folder = None;
    let mut input_settings = InputSettings::default();
    let mut substitutions = SubstitutionSettings::default();
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
        providers_folder = paths
            .settings_file
            .parent()
            .map(|dir| dir.join("providers"));
        history_settings = Some((paths.data_dir, keep_history));
        input_settings = InputSettings::from_document(&settings.document)?;
        substitutions = SubstitutionSettings::from_document(&settings.document)?;
        Endpoint::from_settings(&settings.document)
    })
    .map_err(|error| format!("{error}\n{USAGE}"))?;
    let mut pipeline = ClipboardPipeline::with_substitutions(options, substitutions);
    if let Some(path) = providers_folder {
        pipeline.load_providers(&path);
    }
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
        let mut timeout = if pipeline.has_active_request() {
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
