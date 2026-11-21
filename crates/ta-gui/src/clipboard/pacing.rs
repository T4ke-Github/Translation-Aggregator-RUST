use std::{
    sync::{
        Mutex,
        mpsc::{self, Receiver, RecvTimeoutError, TryRecvError},
    },
    time::{Duration, Instant},
};
use ta_provider_web::{MappedRequest, ProviderError, ValidatedProvider};
use ta_translate::{RetryFailure, SlidingWindowRateLimiter, retry_delay};

pub(super) struct ProviderPacing {
    started: Instant,
    limiter: Option<SlidingWindowRateLimiter>,
    blocked_until: Option<Instant>,
}

impl ProviderPacing {
    pub(super) fn new(limiter: Option<SlidingWindowRateLimiter>) -> Self {
        Self {
            started: Instant::now(),
            limiter,
            blocked_until: None,
        }
    }

    fn delay_or_admit(&mut self) -> Duration {
        let now = Instant::now();
        if let Some(deadline) = self.blocked_until
            && deadline > now
        {
            return deadline - now;
        }
        let now_ms = now.duration_since(self.started).as_millis() as u64;
        if let Some(limiter) = &mut self.limiter {
            let delay = limiter.delay_at(now_ms);
            if delay > 0 {
                return Duration::from_millis(delay.saturating_add(1));
            }
            let admitted = limiter.allow_at(now_ms);
            debug_assert!(admitted, "configured rate limit has available capacity");
        }
        Duration::ZERO
    }
}

pub(super) fn send_with_retries(
    provider: &ValidatedProvider,
    request: &MappedRequest,
    newer_entry: &Receiver<()>,
    pacing: &Mutex<ProviderPacing>,
    waits: &mpsc::Sender<Duration>,
    jitter_seed: u64,
) -> Result<Option<String>, ProviderError> {
    let timeout = Duration::from_secs(provider.definition().provider.timeout_seconds.max(1));
    let mut retries = 0;
    let mut last_error = None;
    loop {
        let mut waited = retries > 0;
        loop {
            if waited && newer_entry.try_recv() != Err(TryRecvError::Empty) {
                return last_error.map_or(Ok(None), Err);
            }
            let wait = pacing
                .lock()
                .expect("provider rate state poisoned")
                .delay_or_admit();
            if wait.is_zero() {
                break;
            }
            let _ = waits.send(wait);
            if newer_entry.recv_timeout(wait) != Err(RecvTimeoutError::Timeout) {
                return last_error.map_or(Ok(None), Err);
            }
            waited = true;
        }
        let failure = match provider.send_translation_attempt(request) {
            Ok(translation) => return Ok(Some(translation)),
            Err(failure) => failure,
        };
        if failure.retry == RetryFailure::Http(429)
            && let Some(seconds) = failure.retry_after_seconds
        {
            let wait = Duration::from_secs(seconds).min(timeout);
            if !wait.is_zero() {
                pacing
                    .lock()
                    .expect("provider rate state poisoned")
                    .blocked_until = Instant::now().checked_add(wait);
                let _ = waits.send(wait);
            }
        }
        let delay = retry_delay(
            failure.retry,
            retries,
            failure.retry_after_seconds,
            timeout,
            jitter_seed,
            true,
        );
        match delay {
            Some(delay) if newer_entry.recv_timeout(delay) == Err(RecvTimeoutError::Timeout) => {
                last_error = Some(failure.error);
                retries += 1;
            }
            _ => return Err(failure.error),
        }
    }
}
