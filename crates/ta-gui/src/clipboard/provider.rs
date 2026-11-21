use std::{
    collections::BTreeMap,
    io::{self, Write},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use ta_core::{
    History, HistoryEntryId, ProviderHistory, ProviderId, ProviderOutcome, ProviderStatus,
};
use ta_platform::credentials::read_secret;
use ta_provider_web::{
    ProviderError, ValidatedProvider, automatic_https_origin, automatic_https_origin_from_template,
    is_loopback_url, libretranslate_preset, libretranslate_request, redact_secrets,
};
use ta_translate::{
    ProviderQueue, QueueAction, RequestMode, RequestRejection, ResultCache, ResultCacheKey,
    RoutingRejection, RoutingRequest, TranslatorCapabilities, TranslatorKind,
    default_auto_request_max_chars, routing_request_rejection,
};

use super::{
    options::Endpoint,
    pacing::{ProviderPacing, send_with_retries},
};
use crate::provider_request::map_custom_request;

struct TranslationJob {
    id: HistoryEntryId,
    text: String,
    mode: RequestMode,
}

struct ActiveRequest {
    id: HistoryEntryId,
    mode: RequestMode,
    started: Instant,
    cache_key: Option<ResultCacheKey>,
    worker: JoinHandle<Result<Option<String>, String>>,
    stop_retries: SyncSender<()>,
    waits: Receiver<Duration>,
}

pub(super) struct ClipboardProvider {
    pub(super) provider: ValidatedProvider,
    pub(super) enabled: bool,
    automatic: bool,
    configured_origin: Option<String>,
    pub(super) approved_origin: Option<String>,
    capabilities: TranslatorCapabilities,
    // Only the keyless built-in preset caches results. Credentials can rotate independently.
    local_endpoint: Option<String>,
    credential: Option<String>,
    cache_enabled: bool,
    pub(super) cache: ResultCache,
    pub(super) read_key: fn(&str) -> io::Result<Option<String>>,
    queue: ProviderQueue<TranslationJob>,
    active: Option<ActiveRequest>,
    pacing: Arc<Mutex<ProviderPacing>>,
    source: String,
    target: String,
}

impl ClipboardProvider {
    pub(super) fn libretranslate(endpoint: Endpoint, source: &str, target: &str) -> Self {
        let mut definition = libretranslate_preset();
        if let Some(timeout) = endpoint.timeout_seconds {
            definition = definition.with_timeout_seconds(timeout);
        }
        let mut provider = Self::new(definition, source, target, endpoint.auto_max_chars);
        provider.enabled = endpoint.server.is_some();
        provider.automatic = true;
        provider.local_endpoint = endpoint.server.map(|server| format!("http://{server}"));
        provider.cache_enabled = endpoint.cache && endpoint.credential.is_none();
        provider.credential = endpoint.credential;
        provider.pacing = Arc::new(Mutex::new(ProviderPacing::new(endpoint.rate_limit)));
        provider
    }

    pub(super) fn custom(definition: ValidatedProvider, source: &str, target: &str) -> Self {
        Self::new(definition, source, target, default_auto_request_max_chars())
    }

    fn new(
        definition: ValidatedProvider,
        source: &str,
        target: &str,
        auto_max_chars: usize,
    ) -> Self {
        let configured = &definition.definition().provider;
        let languages = &definition.definition().languages;
        let url_template = &definition.definition().request.url;
        let configured_origin = (!is_loopback_url(url_template))
            .then(|| automatic_https_origin_from_template(url_template))
            .flatten();
        let mut capabilities = TranslatorCapabilities::new(TranslatorKind::Remote);
        capabilities.id = configured.id.clone();
        capabilities.name = configured.name.clone();
        // Only the mapped URL determines whether a request can leave the machine.
        capabilities.remote = false;
        capabilities.detects_source_language = languages.auto.is_some();
        capabilities.max_chars = Some(configured.max_chars.min(auto_max_chars));
        if !languages.only.is_empty() {
            let mut sources = languages.only.clone();
            if capabilities.detects_source_language {
                sources.push("auto".into());
            }
            capabilities.supported_source_languages = Some(sources);
            capabilities.supported_target_languages = Some(languages.only.clone());
        }
        Self {
            enabled: configured.enabled,
            automatic: configured.auto_translate,
            configured_origin,
            approved_origin: None,
            provider: definition,
            capabilities,
            local_endpoint: None,
            credential: None,
            cache_enabled: false,
            cache: ResultCache::new(),
            read_key: read_secret,
            queue: ProviderQueue::new(),
            active: None,
            pacing: Arc::new(Mutex::new(ProviderPacing::new(None))),
            source: source.to_owned(),
            target: target.to_owned(),
        }
    }

    pub(super) fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub(super) fn consent_origin(&self) -> Option<&str> {
        if self.enabled && self.automatic {
            self.configured_origin.as_deref()
        } else {
            None
        }
    }

    pub(super) fn translate(
        &mut self,
        id: HistoryEntryId,
        text: &str,
        mode: RequestMode,
        history: &mut History,
        output: &mut impl Write,
    ) -> io::Result<()> {
        if !self.enabled || (mode == RequestMode::Automatic && !self.automatic) {
            return Ok(());
        }
        // Manual requests are split into legal chunks before any of them are sent.
        let mut capabilities = self.capabilities.clone();
        if mode == RequestMode::Manual {
            capabilities.max_chars = None;
        }
        let rejection = routing_request_rejection(RoutingRequest {
            capabilities: &capabilities,
            provider_enabled: self.enabled,
            credentials_available: true,
            remote_permission: false,
            auto_translate_enabled: self.automatic,
            source: &self.source,
            target: &self.target,
            text,
            mode,
        });
        if let Some(reason) = rejection {
            let detail = match reason {
                RoutingRejection::Provider(RequestRejection::TooLong {
                    actual_chars,
                    max_chars,
                }) => {
                    format!(
                        "Too long for automatic translation ({actual_chars} characters, limit {max_chars})"
                    )
                }
                _ => format!("{reason:?}"),
            };
            return self.report_skipped(id, &detail, history, output);
        }
        if mode == RequestMode::Automatic
            && self.consent_origin().is_some()
            && self.approved_origin.is_none()
        {
            return self.report_skipped(
                id,
                "automatic off-device sending needs confirmation",
                history,
                output,
            );
        }
        if let Some(active) = &self.active
            && active.mode == RequestMode::Automatic
        {
            let _ = active.stop_retries.try_send(());
        }
        match self.queue.submit(TranslationJob {
            id,
            text: text.into(),
            mode,
        }) {
            QueueAction::Start(job) => self.start(job, history, output),
            QueueAction::Queued => Ok(()),
            QueueAction::Replaced(job) => {
                self.report_skipped(job.id, "newer clipboard input", history, output)
            }
        }
    }

    fn start(
        &mut self,
        job: TranslationJob,
        history: &mut History,
        output: &mut impl Write,
    ) -> io::Result<()> {
        self.start_request(job, history, output)?;
        if self.active.is_none() {
            let next = self.queue.complete();
            debug_assert!(next.is_none(), "a fresh start cannot have queued work");
        }
        Ok(())
    }

    fn start_request(
        &mut self,
        job: TranslationJob,
        history: &mut History,
        output: &mut impl Write,
    ) -> io::Result<()> {
        let TranslationJob { id, text, mode } = job;
        let started = Instant::now();
        let cache_key = self.cache_enabled.then(|| ResultCacheKey {
            provider_id: self.capabilities.id.clone(),
            processed_text: text.clone(),
            source: self.source.clone(),
            target: self.target.clone(),
            settings_fingerprint: self
                .local_endpoint
                .clone()
                .expect("keyless server is configured"),
        });
        if mode == RequestMode::Automatic
            && let Some(cached) = cache_key
                .as_ref()
                .and_then(|key| self.cache.get(key, true))
                .map(str::to_owned)
        {
            return self.report_completion(
                &id,
                Ok(cached),
                started.elapsed(),
                true,
                history,
                output,
            );
        }

        let mut secrets = BTreeMap::new();
        if let Some(name) = &self.credential {
            match (self.read_key)(name) {
                Ok(Some(key)) => {
                    secrets.insert(name.clone(), key);
                }
                result => {
                    let status = if result.is_err() {
                        "needs key (credential store unavailable)"
                    } else {
                        "needs key"
                    };
                    return self.report_completion(
                        &id,
                        Err(status.into()),
                        started.elapsed(),
                        false,
                        history,
                        output,
                    );
                }
            }
        }

        let (stop_retries, newer_entry) = mpsc::sync_channel(1);
        let (wait_tx, waits) = mpsc::channel();
        let pacing = Arc::clone(&self.pacing);
        let provider = self.provider.clone();
        let languages = (self.source.clone(), self.target.clone());
        let endpoint = self.local_endpoint.clone();
        let approved_origin = self.approved_origin.clone();
        let read_key = self.read_key;
        let worker = thread::Builder::new()
            .name(self.capabilities.id.clone())
            .spawn(move || {
                let result = (|| {
                    let chunks = if mode == RequestMode::Manual {
                        provider
                            .manual_chunks(&text)
                            .map_err(|error| format!("error ({error})"))?
                    } else {
                        vec![text]
                    };
                    let mut requests = Vec::with_capacity(chunks.len());
                    let params = endpoint
                        .map(|server| ("server".to_owned(), server))
                        .into_iter()
                        .collect::<BTreeMap<_, _>>();
                    for chunk in chunks {
                        let request = if params.is_empty() {
                            map_custom_request(
                                &provider,
                                &chunk,
                                (&languages.0, &languages.1),
                                &params,
                                &mut secrets,
                                read_key,
                            )?
                        } else {
                            libretranslate_request(
                                &provider,
                                &chunk,
                                &languages.0,
                                &languages.1,
                                &params,
                                secrets.values().next().map(String::as_str),
                            )
                            .map_err(|error| format!("error ({error})"))?
                        };
                        if mode == RequestMode::Automatic
                            && !permits_automatic_url(&request.url, approved_origin.as_deref())
                        {
                            return Err("automatic off-device sending needs confirmation for a fixed HTTPS origin".into());
                        }
                        requests.push(request);
                    }
                    let mut text = String::new();
                    for request in requests {
                        match send_with_retries(
                            &provider,
                            &request,
                            &newer_entry,
                            &pacing,
                            &wait_tx,
                            started.elapsed().as_nanos() as u64,
                        ) {
                            Ok(Some(result)) => text.push_str(&result),
                            Ok(None) => return Ok(None),
                            Err(ProviderError::Timeout) => {
                                return Err(format!(
                                    "timed out after {} s",
                                    provider.definition().provider.timeout_seconds
                                ));
                            }
                            Err(error) => return Err(format!("error ({error})")),
                        }
                    }
                    Ok(Some(text))
                })();
                let secrets: Vec<_> = secrets.into_iter().collect();
                result
                    .map(|translation| translation.map(|text| redact_secrets(&text, &secrets)))
                    .map_err(|error: String| redact_secrets(&error, &secrets))
            });
        match worker {
            Ok(worker) => {
                self.active = Some(ActiveRequest {
                    id,
                    mode,
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
                history,
                output,
            ),
        }
    }

    pub(super) fn poll_completion(
        &mut self,
        history: &mut History,
        output: &mut impl Write,
    ) -> io::Result<bool> {
        if let Some(active) = &self.active {
            for delay in active.waits.try_iter() {
                let seconds = delay.as_secs() + u64::from(delay.subsec_nanos() > 0);
                writeln!(
                    output,
                    "Entry: {}\nProvider: {}\nStatus: Waiting {seconds}s (rate limit)",
                    active.id, self.capabilities.id
                )?;
            }
        }
        if !self
            .active
            .as_ref()
            .is_some_and(|active| active.worker.is_finished())
        {
            return Ok(false);
        }
        let ActiveRequest {
            id,
            started,
            cache_key,
            worker,
            ..
        } = self.active.take().expect("completed request");
        let result = worker
            .join()
            .unwrap_or_else(|_| Err("error (worker stopped)".into()));
        if let (Some(key), Ok(Some(translation))) = (cache_key, &result) {
            self.cache.insert_success(key, translation.clone(), true);
        }
        match result {
            Ok(None) => self.report_skipped(id, "automatic request withdrawn", history, output)?,
            Ok(Some(translation)) => self.report_completion(
                &id,
                Ok(translation),
                started.elapsed(),
                false,
                history,
                output,
            )?,
            Err(error) => {
                self.report_completion(&id, Err(error), started.elapsed(), false, history, output)?
            }
        }
        if let Some(next) = self.queue.complete() {
            self.start(next, history, output)?;
        }
        Ok(true)
    }

    pub(super) fn pause(
        &mut self,
        history: &mut History,
        output: &mut impl Write,
    ) -> io::Result<()> {
        if let Some(active) = &self.active
            && active.mode == RequestMode::Automatic
        {
            let _ = active.stop_retries.try_send(());
        }
        if let Some(job) = self.queue.discard_pending() {
            if job.mode == RequestMode::Manual {
                let action = self.queue.submit(job);
                debug_assert!(matches!(action, QueueAction::Queued));
            } else {
                self.report_skipped(job.id, "automatic input paused", history, output)?;
            }
        }
        Ok(())
    }

    fn provider_id(&self) -> ProviderId {
        ProviderId::new(self.capabilities.id.as_str()).expect("validated provider has an ID")
    }

    fn report_skipped(
        &self,
        id: HistoryEntryId,
        reason: &str,
        history: &mut History,
        output: &mut impl Write,
    ) -> io::Result<()> {
        history.record_provider(
            &id,
            self.provider_id(),
            ProviderHistory {
                outcome: ProviderOutcome::Skipped,
                status: ProviderStatus::Skipped,
                elapsed: Duration::ZERO,
                cached: false,
            },
        );
        writeln!(
            output,
            "Entry: {id}\nProvider: {}\nElapsed: 0 ms\nCached: false\nStatus: skipped ({reason})",
            self.capabilities.id
        )
    }

    fn report_completion(
        &self,
        id: &HistoryEntryId,
        result: Result<String, String>,
        elapsed: Duration,
        cached: bool,
        history: &mut History,
        output: &mut impl Write,
    ) -> io::Result<()> {
        let (outcome, status) = match result {
            Ok(translation) => (
                ProviderOutcome::Result(translation),
                ProviderStatus::Succeeded,
            ),
            Err(error) => (ProviderOutcome::Error(error), ProviderStatus::Failed),
        };
        writeln!(
            output,
            "Entry: {id}\nProvider: {}\nElapsed: {} ms\nCached: {cached}",
            self.capabilities.id,
            elapsed.as_millis()
        )?;
        match &outcome {
            ProviderOutcome::Result(translation) => {
                let state = if cached { "cached" } else { "ready" };
                writeln!(output, "Translation: {translation}\nStatus: {state}")?;
            }
            ProviderOutcome::Error(error) => {
                writeln!(output, "Status: {} {error}", self.capabilities.name)?
            }
            ProviderOutcome::Skipped => unreachable!("completion is not a skip"),
        }
        history.record_provider(
            id,
            self.provider_id(),
            ProviderHistory {
                outcome,
                status,
                elapsed,
                cached,
            },
        );
        Ok(())
    }
}

pub(super) fn permits_automatic_url(url: &str, approved_origin: Option<&str>) -> bool {
    is_loopback_url(url)
        || approved_origin
            .is_some_and(|approved| automatic_https_origin(url).as_deref() == Some(approved))
}
