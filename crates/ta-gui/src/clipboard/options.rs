use std::{net::SocketAddr, num::NonZeroU64};
use ta_platform::{SettingsDocument, credentials::credential_name};
use ta_substitute::SubstitutionRule;
use ta_translate::{SlidingWindowRateLimiter, default_auto_request_max_chars};

pub(super) struct Endpoint {
    pub(super) server: Option<SocketAddr>,
    pub(super) credential: Option<String>,
    pub(super) cache: bool,
    pub(super) auto_max_chars: usize,
    pub(super) rate_limit: Option<SlidingWindowRateLimiter>,
    pub(super) timeout_seconds: Option<NonZeroU64>,
}

impl Default for Endpoint {
    fn default() -> Self {
        Self {
            server: None,
            credential: None,
            cache: true,
            auto_max_chars: default_auto_request_max_chars(),
            rate_limit: None,
            timeout_seconds: None,
        }
    }
}

impl Endpoint {
    pub(super) fn keyless(server: SocketAddr) -> Self {
        Self {
            server: Some(server),
            ..Self::default()
        }
    }

    fn parse_address(address: &str) -> Result<SocketAddr, String> {
        address
            .parse::<SocketAddr>()
            .ok()
            .filter(|s| s.ip().is_loopback() && s.port() != 0)
            .ok_or_else(|| {
                "LibreTranslate requires a loopback IP and port (e.g. 127.0.0.1:5000)".into()
            })
    }

    pub(super) fn from_settings(document: &SettingsDocument) -> Result<Self, String> {
        let Some(settings) = document
            .value()
            .get("providers")
            .and_then(|providers| providers.get("libretranslate"))
        else {
            return Ok(Self::default());
        };
        let settings = settings
            .as_table()
            .ok_or("providers.libretranslate must be a table")?;
        let server = settings
            .get("server")
            .map(|value| {
                let address = value
                    .as_str()
                    .ok_or("providers.libretranslate.server must be a loopback IP and port")?;
                Self::parse_address(address)
            })
            .transpose()?;
        let credential = settings
            .get("credential")
            .map(|value| {
                let name = value
                    .as_str()
                    .ok_or("providers.libretranslate.credential must be a credential name")?;
                credential_name(name).map_err(|error| error.to_string())?;
                Ok::<_, String>(name.to_owned())
            })
            .transpose()?;
        let mut endpoint = Self {
            server,
            credential,
            ..Self::default()
        };
        if let Some(value) = settings.get("cache") {
            endpoint.cache = value
                .as_bool()
                .ok_or("providers.libretranslate.cache must be a boolean")?;
        }
        if let Some(value) = settings.get("auto_max_chars") {
            endpoint.auto_max_chars = value
                .as_integer()
                .and_then(|limit| usize::try_from(limit).ok())
                .filter(|limit| *limit > 0)
                .ok_or("providers.libretranslate.auto_max_chars must be a positive integer")?;
        }
        if let Some(value) = settings.get("rate_limit") {
            let requests = value
                .get("requests")
                .and_then(|count| count.as_integer())
                .and_then(|count| usize::try_from(count).ok())
                .filter(|count| *count > 0)
                .ok_or("providers.libretranslate.rate_limit.requests must be a positive integer")?;
            let window = value
                .get("per_seconds")
                .and_then(|seconds| seconds.as_integer())
                .and_then(|seconds| u64::try_from(seconds).ok())
                .filter(|seconds| *seconds > 0)
                .and_then(|seconds| seconds.checked_mul(1000))
                .ok_or(
                    "providers.libretranslate.rate_limit.per_seconds must be a positive duration",
                )?;
            endpoint.rate_limit = Some(SlidingWindowRateLimiter::new(requests, window));
        }
        if let Some(value) = settings.get("timeout_seconds") {
            endpoint.timeout_seconds = Some(
                value
                    .as_integer()
                    .and_then(|seconds| u64::try_from(seconds).ok())
                    .and_then(NonZeroU64::new)
                    .ok_or("providers.libretranslate.timeout_seconds must be a positive integer")?,
            );
        }
        Ok(endpoint)
    }
}

pub(super) struct ClipboardOptions {
    pub(super) endpoint: Endpoint,
    pub(super) source: String,
    pub(super) target: String,
    pub(super) japanese_only: bool,
    pub(super) rules: Vec<SubstitutionRule>,
}

impl ClipboardOptions {
    pub(super) fn parse(
        args: &[String],
        load_endpoint: impl FnOnce() -> Result<Endpoint, String>,
    ) -> Result<Self, String> {
        let mut args = args.iter().map(String::as_str);
        let mut server = None;
        let mut source = None;
        let mut target = None;
        let mut japanese_only = false;
        let mut rules = Vec::new();
        while let Some(flag) = args.next() {
            let slot = match flag {
                "--libretranslate-server" => &mut server,
                "--source" => &mut source,
                "--target" => &mut target,
                "--japanese-only" => {
                    japanese_only = true;
                    continue;
                }
                "--substitute" => {
                    let (Some(matched), Some(replacement)) = (args.next(), args.next()) else {
                        return Err("--substitute requires a match and a replacement".into());
                    };
                    rules.push(SubstitutionRule::try_new(matched, replacement)?);
                    continue;
                }
                _ => return Err(format!("unknown clipboard argument: {flag}")),
            };
            let Some(value) = args
                .next()
                .filter(|s| !s.trim().is_empty() && !s.starts_with("--"))
            else {
                return Err(format!("{flag} requires a value"));
            };
            if slot.replace(value).is_some() {
                return Err(format!("{flag} was provided more than once"));
            }
        }

        let source = source.ok_or("--source is required")?.into();
        let target = target.ok_or("--target is required")?.into();
        let endpoint = match server {
            Some(address) => Endpoint::keyless(Endpoint::parse_address(address)?),
            None => load_endpoint()?,
        };
        Ok(Self {
            endpoint,
            source,
            target,
            japanese_only,
            rules,
        })
    }
}
