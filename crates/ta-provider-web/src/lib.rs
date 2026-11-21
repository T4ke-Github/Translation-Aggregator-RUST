//! Declarative web translation provider schema, mapping, and HTTP transport.
//! Unsupported here: sessions, OS credential storage, retries, auth flows, and advanced JSONPath.
#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
/// A declarative web translation provider configuration.
/// Provider identity and capabilities.
pub struct ProviderDefinition {
    #[serde(default)]
    /// Non-secret parameter declarations and defaults.
    pub params: BTreeMap<String, ParamDefinition>,
    #[serde(default)]
    /// Provider identity and limits.
    pub provider: Provider,
    #[serde(default)]
    /// Language support and provider-specific language codes.
    pub languages: Languages,
    #[serde(default)]
    /// Request template and mapping configuration.
    pub request: Request,
    #[serde(default)]
    /// Response fields used to extract translation results.
    pub response: Response,
}

/// A non-secret user parameter declared by a provider preset.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct ParamDefinition {
    /// Label displayed when the user sets this parameter.
    #[serde(default)]
    pub label: String,
    /// Kind of input accepted by settings (for example, text or choice).
    #[serde(default, rename = "type")]
    pub kind: String,
    /// Optional non-secret default; absent values still need user setup.
    #[serde(default)]
    pub default: Option<String>,
    /// Choices for a selection field, if any.
    #[serde(default)]
    pub choices: Option<toml::Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
/// Provider identity and basic input limits.
pub struct Provider {
    /// Stable provider identifier.
    pub id: String,
    /// Name displayed to the user.
    pub name: String,
    #[serde(default)]
    /// Whether the provider is enabled.
    pub enabled: bool,
    #[serde(default)]
    /// Whether to display the unofficial-service warning.
    pub unofficial: bool,
    #[serde(default)]
    /// Whether incoming text is translated automatically.
    pub auto_translate: bool,
    #[serde(default = "default_max_chars")]
    /// Maximum text length per request.
    pub max_chars: usize,
    #[serde(default = "default_timeout_seconds")]
    /// Request timeout in seconds.
    pub timeout_seconds: u64,
}
fn default_max_chars() -> usize {
    1000
}
fn default_timeout_seconds() -> u64 {
    15
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
/// Language support configuration.
pub struct Languages {
    #[serde(default)]
    /// Provider code for automatic source-language detection.
    pub auto: Option<String>,
    #[serde(default)]
    /// Mapping from application language codes to provider codes.
    pub map: BTreeMap<String, String>,
    #[serde(default)]
    /// Languages supported by this provider.
    pub only: Vec<String>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
/// HTTP request template; execution is outside this crate.
pub struct Request {
    #[serde(default)]
    /// Uppercase HTTP method.
    pub method: String,
    #[serde(default)]
    /// Request URL template.
    pub url: String,
    #[serde(default)]
    /// Query parameter templates.
    pub query: BTreeMap<String, String>,
    #[serde(default)]
    /// Header value templates.
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    /// Optional JSON request body template.
    pub body: Option<Body>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
/// JSON request body configuration.
pub struct Body {
    #[serde(default)]
    /// Content type associated with the body.
    pub content_type: String,
    #[serde(default)]
    /// JSON value with supported placeholders.
    pub json: Value,
}
impl Default for Body {
    fn default() -> Self {
        Self {
            content_type: String::new(),
            json: Value::Null,
        }
    }
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
/// Response extraction configuration.
pub struct Response {
    #[serde(default)]
    /// Whether extracted text should be HTML-decoded.
    pub html_decode: bool,
    /// Selector for the translated text.
    pub text: Selector,
    #[serde(default)]
    /// Optional selector for provider error information.
    pub error: Option<Selector>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
/// A supported, restricted JSONPath selector.
pub struct Selector {
    /// Path to the selected response value.
    #[serde(default)]
    pub jsonpath: String,
    /// Regex for HTML or text responses; group 1 is extracted.
    #[serde(default)]
    pub regex: Option<String>,
    /// Separator used when the selector yields multiple values.
    #[serde(default)]
    pub join: Option<String>,
}
#[derive(Clone, Debug)]
/// A provider definition that passed schema validation.
pub struct ValidatedProvider(ProviderDefinition, Vec<String>);
#[derive(Clone, Debug, PartialEq, Eq)]
/// Errors raised while parsing, validating, mapping, or extracting.
pub enum ProviderError {
    /// TOML parsing failed.
    Parse(String),
    /// The configuration is invalid.
    Invalid(String),
    /// A placeholder is unsupported or malformed.
    Template(String),
    /// A secret required by the request was not injected.
    NeedsSecret(String),
    /// The response did not contain a valid translation.
    Response(String),
    /// HTTP transport failed.
    Transport(String),
    /// Request exceeded its configured timeout.
    Timeout,
    /// Server returned a non-success status; the body is intentionally omitted.
    HttpStatus(u16),
    /// Response exceeded the maximum permitted size.
    ResponseTooLarge,
}
impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpStatus(code) => write!(f, "HTTP {code} {}", status_reason(*code)),
            _ => write!(f, "{:?}", self),
        }
    }
}
impl std::error::Error for ProviderError {}
struct HttpResponse {
    status: reqwest::StatusCode,
    charset: Option<String>,
    body: Vec<u8>,
}

fn status_reason(code: u16) -> &'static str {
    match code {
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "HTTP Error",
    }
}

fn charset_from_content_type(content_type: &str) -> Option<String> {
    content_type.split(';').skip(1).find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        (key.trim().eq_ignore_ascii_case("charset"))
            .then(|| value.trim().trim_matches(['"', '\'']).to_owned())
    })
}

fn decode_response_body(bytes: &[u8], charset: Option<&str>) -> String {
    charset
        .and_then(|label| encoding_rs::Encoding::for_label(label.as_bytes()))
        .map(|encoding| encoding.decode(bytes).0.into_owned())
        .unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned())
}
#[derive(Clone, PartialEq)]
/// Request data after applying translation placeholders.
pub struct MappedRequest {
    /// HTTP method.
    pub method: String,
    /// Mapped request URL.
    pub url: String,
    /// Mapped query parameters.
    pub query: BTreeMap<String, String>,
    /// Mapped headers.
    pub headers: BTreeMap<String, String>,
    /// Mapped JSON body, if configured.
    pub body: Option<Value>,
}
impl fmt::Debug for MappedRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MappedRequest")
            .field("method", &self.method)
            .field("url", &"[redacted]")
            .field("query", &"[redacted]")
            .field("headers", &"[redacted]")
            .field("body", &"[redacted]")
            .finish()
    }
}

fn prune_unknown(value: &mut toml::Value, path: &str, warnings: &mut Vec<String>) {
    let Some(table) = value.as_table_mut() else {
        return;
    };
    let schema: &[&str] = match path {
        "" => &["provider", "languages", "request", "response", "params"],
        "provider" => &[
            "id",
            "name",
            "enabled",
            "unofficial",
            "auto_translate",
            "max_chars",
            "timeout_seconds",
        ],
        "languages" => &["auto", "map", "only"],
        "request" => &["method", "url", "query", "headers", "body"],
        "request.body" => &["content_type", "json"],
        "response" => &["html_decode", "text", "error"],
        "response.text" | "response.error" => &["jsonpath", "regex", "join"],
        _ => return,
    };
    let keys: Vec<String> = table.keys().cloned().collect();
    for key in keys {
        let full = if path.is_empty() {
            key.clone()
        } else {
            format!("{path}.{key}")
        };
        if !schema.contains(&key.as_str()) {
            warnings.push(full);
            table.remove(&key);
        } else if !matches!(
            full.as_str(),
            "languages.map" | "request.query" | "request.headers" | "request.body.json"
        ) && let Some(child) = table.get_mut(&key)
        {
            prune_unknown(child, &full, warnings);
        }
    }
    warnings.sort();
}

impl ProviderDefinition {
    /// Loads TOML provider definitions in filename order, keeping errors isolated per file.
    pub fn load_folder(path: impl AsRef<std::path::Path>) -> FolderLoadReport {
        let path = path.as_ref();
        let mut report = FolderLoadReport::default();
        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) => {
                report.errors.push(FolderLoadError {
                    path: path.to_owned(),
                    message: error.to_string(),
                });
                return report;
            }
        };
        let mut files = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) if entry.path().extension().is_some_and(|ext| ext == "toml") => {
                    files.push(entry.path());
                }
                Ok(_) => {}
                Err(error) => report.errors.push(FolderLoadError {
                    path: path.to_owned(),
                    message: error.to_string(),
                }),
            }
        }
        files.sort();
        let mut ids = std::collections::BTreeMap::<String, std::path::PathBuf>::new();
        for file in files {
            let parsed = std::fs::read_to_string(&file)
                .map_err(|e| e.to_string())
                .and_then(|text| Self::from_toml(&text).map_err(|e| e.to_string()));
            match parsed {
                Ok(provider) => {
                    let id = provider.definition().provider.id.clone();
                    if let Some(first) = ids.get(&id) {
                        report.errors.push(FolderLoadError {
                            path: file,
                            message: format!(
                                "duplicate provider.id {id:?}; also defined in {}",
                                first.display()
                            ),
                        });
                    } else {
                        ids.insert(id, file);
                        report.providers.push(provider);
                    }
                }
                Err(message) => report.errors.push(FolderLoadError {
                    path: file,
                    message,
                }),
            }
        }
        report
    }
    /// Parses and validates a TOML provider definition.
    pub fn from_toml(s: &str) -> Result<ValidatedProvider, ProviderError> {
        let mut value: toml::Value = toml::from_str(s).map_err(|error: toml::de::Error| {
            let position = error.span().map(|span| {
                let before = &s[..span.start];
                let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
                let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
                format!("line {line}, column {column}: ")
            });
            ProviderError::Parse(format!(
                "{}{}",
                position.unwrap_or_default(),
                error.message()
            ))
        })?;
        let mut warnings = Vec::new();
        prune_unknown(&mut value, "", &mut warnings);
        let d: Self = value
            .try_into()
            .map_err(|_| ProviderError::Parse("invalid provider definition".into()))?;
        let mut validated = d.validate()?;
        let literal_warnings = literal_credential_warnings(
            &validated.0.request.headers,
            validated.0.request.body.as_ref().map(|body| &body.json),
        );
        let secret_in_request = !literal_warnings.is_empty()
            || validated.0.request.url.contains("{{secret.")
            || validated
                .0
                .request
                .query
                .values()
                .any(|value| value.contains("{{secret."))
            || validated
                .0
                .request
                .headers
                .values()
                .any(|value| value.contains("{{secret."))
            || validated
                .0
                .request
                .body
                .as_ref()
                .is_some_and(|body| body.json.to_string().contains("{{secret."));
        if requires_insecure_http_confirmation(&validated.0.request.url, secret_in_request) {
            warnings.push("HTTP with a secret requires explicit user confirmation".into());
        }
        warnings.extend(literal_warnings);
        validated.1 = warnings;
        Ok(validated)
    }
    /// Checks configuration constraints and returns a validated provider.
    pub fn validate(self) -> Result<ValidatedProvider, ProviderError> {
        if self.provider.id.trim().is_empty() {
            return Err(ProviderError::Invalid("provider.id is required".into()));
        }
        if self.provider.name.trim().is_empty() {
            return Err(ProviderError::Invalid("provider.name is required".into()));
        }
        if self.request.url.trim().is_empty() {
            return Err(ProviderError::Invalid("request.url is required".into()));
        }
        if !matches!(
            self.request.method.as_str(),
            "GET" | "POST" | "PUT" | "PATCH" | "DELETE"
        ) {
            return Err(ProviderError::Invalid(
                "request.method must be an uppercase supported HTTP method".into(),
            ));
        }
        let path = self.response.text.jsonpath.as_str();
        if self.response.text.regex.is_none() && path.is_empty() {
            return Err(ProviderError::Invalid(
                "response.text.jsonpath or regex is required".into(),
            ));
        }
        if !path.is_empty() {
            segments(path)?;
        }
        if let Some(pattern) = &self.response.text.regex {
            regex::Regex::new(pattern)
                .map_err(|e| ProviderError::Invalid(format!("invalid response regex: {e}")))?;
        }
        if let Some(error) = &self.response.error {
            if !error.jsonpath.is_empty() {
                segments(&error.jsonpath)?;
            }
            if let Some(pattern) = &error.regex {
                regex::Regex::new(pattern)
                    .map_err(|e| ProviderError::Invalid(format!("invalid error regex: {e}")))?;
            }
        }
        for s in std::iter::once(self.request.url.as_str())
            .chain(self.request.query.values().map(String::as_str))
            .chain(self.request.headers.values().map(String::as_str))
        {
            validate_template(s)?;
        }
        if let Some(body) = &self.request.body {
            validate_json(&body.json)?;
        }
        Ok(ValidatedProvider(self, Vec::new()))
    }
}
/// Results from loading a directory of provider files.
#[derive(Clone, Debug, Default)]
pub struct FolderLoadReport {
    /// Successfully loaded providers in filename order.
    pub providers: Vec<ValidatedProvider>,
    /// File-specific parse, validation, and duplicate-ID errors.
    pub errors: Vec<FolderLoadError>,
}
/// An error associated with a provider file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderLoadError {
    /// Path of the file that failed.
    pub path: std::path::PathBuf,
    /// Diagnostic message.
    pub message: String,
}
impl fmt::Display for FolderLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}
fn query_has_credential(request: &MappedRequest) -> bool {
    let sensitive_name = |name: &str| {
        let name = name.to_ascii_lowercase();
        ["key", "token", "secret", "pass", "auth", "credential"]
            .iter()
            .any(|term| name.contains(term))
    };
    request.query.keys().any(|name| sensitive_name(name))
        || reqwest::Url::parse(&request.url).is_ok_and(|url| {
            !url.username().is_empty()
                || url.password().is_some()
                || url.query_pairs().any(|(name, _)| sensitive_name(&name))
        })
}

impl ValidatedProvider {
    fn configured_error(
        &self,
        body: &str,
        request: &MappedRequest,
    ) -> Result<Option<String>, ProviderError> {
        let Some(selector) = &self.0.response.error else {
            return Ok(None);
        };
        let regex_message = if let Some(pattern) = &selector.regex {
            let re = regex::Regex::new(pattern)
                .map_err(|_| ProviderError::Invalid("invalid error regex".into()))?;
            re.captures(body)
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str().to_owned())
                .filter(|value| !value.is_empty())
        } else {
            None
        };
        let json_message = if regex_message.is_none() && !selector.jsonpath.is_empty() {
            serde_json::from_str::<Value>(body).ok().and_then(|json| {
                select_json_many(&json, &selector.jsonpath)
                    .ok()?
                    .into_iter()
                    .find_map(|value| match value {
                        Value::String(text) if !text.is_empty() => Some(text.clone()),
                        Value::Number(number) => Some(number.to_string()),
                        Value::Bool(true) => Some("true".into()),
                        _ => None,
                    })
            })
        } else {
            None
        };
        let Some(message) = regex_message.or(json_message) else {
            return Ok(None);
        };
        let template = &self.0.request;
        let has_secret = query_has_credential(request)
            || template.url.contains("{{secret.")
            || template
                .query
                .values()
                .any(|value| value.contains("{{secret."))
            || template
                .headers
                .values()
                .any(|value| value.contains("{{secret."))
            || template
                .body
                .as_ref()
                .is_some_and(|body| body.json.to_string().contains("{{secret."))
            || !literal_credential_warnings(
                &template.headers,
                template.body.as_ref().map(|body| &body.json),
            )
            .is_empty();
        let message = if has_secret {
            "service error"
        } else {
            &message
        };
        Ok(Some(format!("{}: {message}", self.0.provider.name)))
    }
    /// Translate manual text by splitting it into sentence-aware provider-sized requests.
    /// All chunks are validated and mapped before `transport` is invoked.
    pub fn send_manual_with<F>(
        &self,
        text: &str,
        source: &str,
        target: &str,
        mut transport: F,
    ) -> Result<String, ProviderError>
    where
        F: FnMut(&MappedRequest) -> Result<Vec<u8>, ProviderError>,
    {
        let chunks =
            ta_translate::split_sentences(text, self.0.provider.max_chars).map_err(|e| {
                ProviderError::Invalid(format!(
                    "Text too long for {} ({} characters, limit {})",
                    self.0.provider.name, e.actual_chars, e.max_chars
                ))
            })?;
        let requests = chunks
            .iter()
            .map(|chunk| self.map_request(chunk, source, target))
            .collect::<Result<Vec<_>, _>>()?;
        let mut translations = Vec::with_capacity(requests.len());
        for request in &requests {
            let bytes = transport(request)?;
            translations.push(self.extract_translation_from_bytes(&bytes)?);
        }
        Ok(translations.join(""))
    }

    /// Translate manual text using the live HTTP transport.
    pub fn send_manual(
        &self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<String, ProviderError> {
        self.send_manual_with(text, source, target, |request| self.send(request))
    }

    /// Sends and processes a response, checking configured service errors before status and text.
    pub fn send_translation(&self, request: &MappedRequest) -> Result<String, ProviderError> {
        let response = self.send_response(request, true)?;
        let text = decode_response_body(&response.body, response.charset.as_deref());
        if let Some(error) = self.configured_error(&text, request)? {
            return Err(ProviderError::Response(error));
        }
        if !response.status.is_success() {
            return Err(ProviderError::HttpStatus(response.status.as_u16()));
        }
        self.extract_translation_from_bytes(text.as_bytes())
    }

    /// Returns the validated provider definition.
    pub fn definition(&self) -> &ProviderDefinition {
        &self.0
    }
    /// Unknown configuration field paths, sorted for deterministic diagnostics.
    pub fn warnings(&self) -> &[String] {
        &self.1
    }
    /// Sends a mapped request with the provider timeout and an 8 MiB response limit.
    pub fn send(&self, request: &MappedRequest) -> Result<Vec<u8>, ProviderError> {
        let response = self.send_response(request, false)?;
        if !response.status.is_success() {
            return Err(ProviderError::HttpStatus(response.status.as_u16()));
        }
        Ok(response.body)
    }

    fn send_response(
        &self,
        request: &MappedRequest,
        inspect_service_error: bool,
    ) -> Result<HttpResponse, ProviderError> {
        use std::io::Read;
        const MAX: u64 = 8 * 1024 * 1024;
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(
                self.0.provider.timeout_seconds.max(1),
            ))
            .build()
            .map_err(|_| ProviderError::Transport("HTTP client setup failed".into()))?;
        let mut outgoing = client.request(
            request
                .method
                .parse::<reqwest::Method>()
                .map_err(|_| ProviderError::Transport("invalid HTTP method".into()))?,
            &request.url,
        );
        for (name, value) in &request.query {
            outgoing = outgoing.query(&[(name, value)]);
        }
        for (name, value) in &request.headers {
            outgoing = outgoing.header(name, value);
        }
        if let Some(body) = &request.body {
            let content_type = self
                .0
                .request
                .body
                .as_ref()
                .map(|b| b.content_type.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("application/json");
            outgoing = outgoing
                .header(reqwest::header::CONTENT_TYPE, content_type)
                .json(body);
        }
        let mut response = outgoing.send().map_err(|error| {
            if error.is_timeout() {
                ProviderError::Timeout
            } else {
                ProviderError::Transport("HTTP request failed".into())
            }
        })?;
        let status = response.status();
        if !status.is_success() && (!inspect_service_error || self.0.response.error.is_none()) {
            return Err(ProviderError::HttpStatus(status.as_u16()));
        }
        let charset = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(charset_from_content_type);
        if response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .is_some_and(|n| n > MAX)
        {
            return Err(if status.is_success() {
                ProviderError::ResponseTooLarge
            } else {
                ProviderError::HttpStatus(status.as_u16())
            });
        }
        let mut bytes = Vec::new();
        response
            .by_ref()
            .take(MAX + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| {
                if status.is_success() {
                    ProviderError::Transport(e.kind().to_string())
                } else {
                    ProviderError::HttpStatus(status.as_u16())
                }
            })?;
        if bytes.len() as u64 > MAX {
            return Err(if status.is_success() {
                ProviderError::ResponseTooLarge
            } else {
                ProviderError::HttpStatus(status.as_u16())
            });
        }
        Ok(HttpResponse {
            status,
            charset,
            body: bytes,
        })
    }
    /// Applies placeholders to construct a request.
    pub fn map_request(
        &self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<MappedRequest, ProviderError> {
        self.map_request_with_values(text, source, target, &BTreeMap::new(), &BTreeMap::new())
    }
    /// Maps a request with externally supplied parameter and secret values.
    pub fn map_request_with_values(
        &self,
        text: &str,
        source: &str,
        target: &str,
        params: &BTreeMap<String, String>,
        secrets: &BTreeMap<String, String>,
    ) -> Result<MappedRequest, ProviderError> {
        if text.chars().count() > self.0.provider.max_chars {
            return Err(ProviderError::Invalid(format!(
                "text exceeds max_chars ({})",
                self.0.provider.max_chars
            )));
        }
        let allowed = &self.0.languages.only;
        if !allowed.is_empty()
            && ((!allowed.iter().any(|v| v == source) && source != "auto")
                || !allowed.iter().any(|v| v == target))
        {
            return Err(ProviderError::Invalid(
                "unsupported source or target language".into(),
            ));
        }
        let r = &self.0.request;
        let mut values: BTreeMap<String, String> = self
            .0
            .params
            .iter()
            .filter_map(|(name, definition)| {
                definition
                    .default
                    .as_ref()
                    .map(|value| (name.clone(), value.clone()))
            })
            .collect();
        values.extend(params.iter().map(|(k, v)| (k.clone(), v.clone())));
        for key in template_keys(r) {
            if let Some((kind, name)) = key.split_once('.') {
                if kind == "secret" && secrets.get(name).is_none_or(|value| value.is_empty()) {
                    return Err(ProviderError::NeedsSecret(name.to_owned()));
                }
                if kind == "param" && !values.contains_key(name) {
                    return Err(ProviderError::Template(format!(
                        "Needs setup: parameter {name}"
                    )));
                }
            }
        }
        if let Some((name, _)) = r
            .url
            .strip_prefix("{{param.")
            .and_then(|url| url.split_once("}}"))
        {
            let server = values.get(name).expect("checked parameter");
            let valid = reqwest::Url::parse(server).is_ok_and(|url| {
                matches!(url.scheme(), "http" | "https")
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.path() == "/"
                    && url.query().is_none()
                    && url.fragment().is_none()
            });
            if !valid {
                return Err(ProviderError::Template(format!(
                    "Needs setup: parameter {name} must be a server URL"
                )));
            }
        }
        let languages = &self.0.languages;
        let omit_source = source == "auto" && languages.auto.as_deref() == Some("");
        let source = if source == "auto" {
            languages.auto.as_deref().unwrap_or(source)
        } else {
            languages
                .map
                .get(source)
                .map(String::as_str)
                .unwrap_or(source)
        };
        let target = languages
            .map
            .get(target)
            .map(String::as_str)
            .unwrap_or(target);
        if omit_source
            && (r.url.contains("{{src}}")
                || r.headers.values().any(|v| v.contains("{{src}}"))
                || r.query
                    .values()
                    .any(|v| v != "{{src}}" && v.contains("{{src}}")))
        {
            return Err(ProviderError::Template(
                "auto source cannot be omitted from a mixed template".into(),
            ));
        }
        let mapped = MappedRequest {
            method: r.method.clone(),
            url: substitute_values(&r.url, text, source, target, Context::Url, &values, secrets),
            query: r
                .query
                .iter()
                .filter(|(_, v)| !omit_source || v.as_str() != "{{src}}")
                .map(|(k, v)| {
                    (
                        k.clone(),
                        substitute_values(v, text, source, target, Context::Raw, &values, secrets),
                    )
                })
                .collect(),
            headers: r
                .headers
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        substitute_values(v, text, source, target, Context::Raw, &values, secrets),
                    )
                })
                .collect(),
            body: r
                .body
                .as_ref()
                .map(|b| {
                    if omit_source {
                        map_json_without_source(&b.json, text, target, &values, secrets)
                    } else {
                        Ok(map_json_with_values(
                            &b.json, text, source, target, &values, secrets,
                        ))
                    }
                })
                .transpose()?,
        };
        let has_secret = template_keys(r)
            .iter()
            .any(|key| key.starts_with("secret."))
            || !literal_credential_warnings(&r.headers, r.body.as_ref().map(|body| &body.json))
                .is_empty();
        if requires_insecure_http_confirmation(&mapped.url, has_secret) {
            return Err(ProviderError::Invalid(
                "HTTP with a secret requires explicit user confirmation".into(),
            ));
        }
        Ok(mapped)
    }
    /// Extracts a translation string from a JSON response.
    pub fn extract_translation(&self, response: &Value) -> Result<String, ProviderError> {
        let selector = &self.0.response.text;
        self.extract_json_matches(response, selector)
            .map(|(text, _)| text)
    }

    /// Extracts text and reports how many non-null values matched for the Test view.
    pub fn extract_translation_with_match_count(
        &self,
        response: &Value,
    ) -> Result<(String, usize), ProviderError> {
        self.extract_json_matches(response, &self.0.response.text)
    }

    fn extract_json_matches(
        &self,
        response: &Value,
        selector: &Selector,
    ) -> Result<(String, usize), ProviderError> {
        let matches = select_json_many(response, &selector.jsonpath)?;
        let values = matches
            .into_iter()
            .filter(|value| !value.is_null())
            .map(|value| match value {
                Value::String(text) => Ok(text.clone()),
                Value::Number(number) => Ok(number.to_string()),
                _ => Err(ProviderError::Response(
                    "selected value is not text or a number".into(),
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if values.is_empty() {
            return Err(ProviderError::Response(format!(
                "path {} did not match",
                selector.jsonpath
            )));
        }
        let count = values.len();
        let text = match &selector.join {
            Some(separator) => values.join(separator),
            None => values.into_iter().next().expect("nonempty matches"),
        };
        Ok((text, count))
    }

    /// Checks configured provider errors before extracting translation text from raw bytes.
    pub fn extract_translation_from_bytes(&self, bytes: &[u8]) -> Result<String, ProviderError> {
        let raw = String::from_utf8_lossy(bytes);
        let selector = &self.0.response.text;
        if let Some(error_selector) = &self.0.response.error
            && let Some(pattern) = &error_selector.regex
        {
            let re = regex::Regex::new(pattern)
                .map_err(|e| ProviderError::Invalid(format!("invalid error regex: {e}")))?;
            if let Some(message) = re
                .captures(&raw)
                .and_then(|c| c.get(1))
                .filter(|m| !m.as_str().is_empty())
            {
                return Err(ProviderError::Response(format!(
                    "{}: {}",
                    self.0.provider.name,
                    message.as_str()
                )));
            }
        }
        let mut result = if let Some(pattern) = &selector.regex {
            let re = regex::Regex::new(pattern)
                .map_err(|e| ProviderError::Invalid(format!("invalid response regex: {e}")))?;
            let matches = re
                .captures_iter(&raw)
                .filter_map(|c| c.get(1).map(|m| m.as_str().to_owned()))
                .collect::<Vec<_>>();
            if matches.is_empty() {
                return Err(ProviderError::Response(
                    "translation not found in reply".into(),
                ));
            }
            match &selector.join {
                Some(separator) => matches.join(separator),
                None => matches.into_iter().next().expect("matches is nonempty"),
            }
        } else {
            let json: Value = serde_json::from_str(&raw)
                .map_err(|_| ProviderError::Response("Reply was not JSON".into()))?;
            if let Some(error_selector) = &self.0.response.error
                && !error_selector.jsonpath.is_empty()
                && let Ok(error) = select_json_many(&json, &error_selector.jsonpath)
            {
                let message = error
                    .into_iter()
                    .find_map(|value| match value {
                        Value::String(text) if !text.is_empty() => Some(text.clone()),
                        Value::Number(number) => Some(number.to_string()),
                        _ => None,
                    })
                    .unwrap_or_default();
                if !message.is_empty() {
                    return Err(ProviderError::Response(format!(
                        "{}: {message}",
                        self.0.provider.name
                    )));
                }
            }
            let (text, count) = self
                .extract_json_matches(&json, selector)
                .map_err(|error| match error {
                    ProviderError::Response(message) if message.starts_with("path ") => {
                        ProviderError::Response(format!(
                            "Translation not found in reply (path `{}` matched nothing)",
                            selector.jsonpath
                        ))
                    }
                    other => other,
                })?;
            let _match_count_for_test_view = count;
            text
        };
        if self.0.response.html_decode {
            result = decode_html_entities(&result);
        }
        Ok(result)
    }
}
fn select_json_many<'a>(response: &'a Value, path: &str) -> Result<Vec<&'a Value>, ProviderError> {
    let mut current = vec![response];
    for segment in segments(path)? {
        current = current
            .into_iter()
            .flat_map(|value| match &segment {
                Segment::Key(key) => value.get(key).into_iter().collect::<Vec<_>>(),
                Segment::Index(index) => value.get(*index).into_iter().collect::<Vec<_>>(),
                Segment::Wildcard => value
                    .as_array()
                    .map(|a| a.iter().collect())
                    .unwrap_or_default(),
            })
            .collect();
        if current.is_empty() {
            return Err(ProviderError::Response(format!(
                "path {path} did not match"
            )));
        }
    }
    Ok(current)
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

#[derive(Clone, Copy)]
enum Context {
    Raw,
    Url,
}

fn substitute_values(
    template: &str,
    text: &str,
    source: &str,
    target: &str,
    context: Context,
    params: &BTreeMap<String, String>,
    secrets: &BTreeMap<String, String>,
) -> String {
    let mut out = String::new();
    let mut offset = 0;
    while offset < template.len() {
        if template[offset..].starts_with("{{")
            && let Some(end) = template[offset + 2..].find("}}")
        {
            let name = &template[offset + 2..offset + 2 + end];
            let value = match name {
                "text" => Some(text),
                "src" => Some(source),
                "dst" => Some(target),
                _ => name
                    .strip_prefix("param.")
                    .and_then(|key| params.get(key).map(String::as_str))
                    .or_else(|| {
                        name.strip_prefix("secret.")
                            .and_then(|key| secrets.get(key).map(String::as_str))
                    }),
            };
            if let Some(value) = value {
                match context {
                    Context::Raw => out.push_str(value),
                    Context::Url
                        if offset == 0
                            && name.starts_with("param.")
                            && (value.starts_with("http://") || value.starts_with("https://")) =>
                    {
                        out.push_str(value.trim_end_matches('/'));
                    }
                    Context::Url => out.push_str(&percent_encode(value)),
                }
                offset += end + 4;
                continue;
            }
        }
        let ch = template[offset..].chars().next().expect("remaining text");
        out.push(ch);
        offset += ch.len_utf8();
    }
    out
}

fn map_json_with_values(
    value: &Value,
    text: &str,
    source: &str,
    target: &str,
    params: &BTreeMap<String, String>,
    secrets: &BTreeMap<String, String>,
) -> Value {
    match value {
        Value::String(value) => Value::String(substitute_values(
            value,
            text,
            source,
            target,
            Context::Raw,
            params,
            secrets,
        )),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| map_json_with_values(value, text, source, target, params, secrets))
                .collect(),
        ),
        Value::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        map_json_with_values(value, text, source, target, params, secrets),
                    )
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}

#[cfg(test)]
fn map_json(value: &Value, text: &str, source: &str, target: &str) -> Value {
    map_json_with_values(
        value,
        text,
        source,
        target,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
}

fn map_json_without_source(
    value: &Value,
    text: &str,
    target: &str,
    params: &BTreeMap<String, String>,
    secrets: &BTreeMap<String, String>,
) -> Result<Value, ProviderError> {
    match value {
        Value::Object(values) => values
            .iter()
            .filter(|(_, value)| value.as_str() != Some("{{src}}"))
            .map(|(key, value)| {
                Ok((
                    key.clone(),
                    map_json_without_source(value, text, target, params, secrets)?,
                ))
            })
            .collect::<Result<serde_json::Map<String, Value>, ProviderError>>()
            .map(Value::Object),
        Value::Array(values) => values
            .iter()
            .map(|value| map_json_without_source(value, text, target, params, secrets))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::String(value) if value.contains("{{src}}") => Err(ProviderError::Template(
            "auto source cannot be omitted from a mixed template".into(),
        )),
        _ => Ok(map_json_with_values(
            value, text, "", target, params, secrets,
        )),
    }
}

fn template_keys(request: &Request) -> Vec<String> {
    fn collect_from_value(value: &Value, templates: &mut Vec<String>) {
        match value {
            Value::String(text) => templates.push(text.clone()),
            Value::Array(items) => {
                for item in items {
                    collect_from_value(item, templates);
                }
            }
            Value::Object(items) => {
                for item in items.values() {
                    collect_from_value(item, templates);
                }
            }
            _ => {}
        }
    }
    let mut templates = vec![request.url.clone()];
    templates.extend(request.query.values().cloned());
    templates.extend(request.headers.values().cloned());
    if let Some(body) = &request.body {
        collect_from_value(&body.json, &mut templates);
    }
    let mut keys = Vec::new();
    for template in templates {
        let mut remaining = template.as_str();
        while let Some(start) = remaining.find("{{") {
            remaining = &remaining[start + 2..];
            if let Some(end) = remaining.find("}}") {
                keys.push(remaining[..end].to_owned());
                remaining = &remaining[end + 2..];
            } else {
                break;
            }
        }
    }
    keys
}

fn validate_template(s: &str) -> Result<(), ProviderError> {
    let mut rest = s;
    while let Some(i) = rest.find("{{") {
        rest = &rest[i + 2..];
        let e = rest
            .find("}}")
            .ok_or_else(|| ProviderError::Template("unclosed placeholder".into()))?;
        if !matches!(&rest[..e], "text" | "src" | "dst") && !valid_named_placeholder(&rest[..e]) {
            return Err(ProviderError::Template(format!(
                "unsupported placeholder {{{{{}}}}}",
                &rest[..e]
            )));
        }
        rest = &rest[e + 2..];
    }
    Ok(())
}
fn valid_named_placeholder(key: &str) -> bool {
    let Some((kind, name)) = key.split_once('.') else {
        return false;
    };
    matches!(kind, "param" | "secret")
        && !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-".contains(c))
}
fn percent_encode(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char)
        } else {
            o.push_str(&format!("%{b:02X}"))
        }
    }
    o
}
fn validate_json(v: &Value) -> Result<(), ProviderError> {
    match v {
        Value::String(s) => validate_template(s),
        Value::Array(a) => a.iter().try_for_each(validate_json),
        Value::Object(o) => o.values().try_for_each(validate_json),
        _ => Ok(()),
    }
}
enum Segment {
    Key(String),
    Index(usize),
    Wildcard,
}
fn segments(p: &str) -> Result<Vec<Segment>, ProviderError> {
    if !p.starts_with('$') {
        return Err(ProviderError::Invalid("JSONPath must start with $".into()));
    }
    let b = p.as_bytes();
    let mut i = 1;
    let mut out = Vec::new();
    while i < b.len() {
        if b[i] == b'.' {
            i += 1;
            let st = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'-') {
                i += 1
            }
            if st == i {
                return Err(ProviderError::Invalid("invalid JSONPath key".into()));
            }
            out.push(Segment::Key(p[st..i].into()));
        } else if b[i] == b'[' {
            i += 1;
            if i < b.len() && b[i].is_ascii_digit() {
                let st = i;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1
                }
                let n = p[st..i]
                    .parse()
                    .map_err(|_| ProviderError::Invalid("JSONPath index overflow".into()))?;
                if i >= b.len() || b[i] != b']' {
                    return Err(ProviderError::Invalid("invalid JSONPath index".into()));
                }
                i += 1;
                out.push(Segment::Index(n));
            } else if i < b.len() && b[i] == b'*' {
                i += 1;
                if i >= b.len() || b[i] != b']' {
                    return Err(ProviderError::Invalid("invalid JSONPath wildcard".into()));
                }
                i += 1;
                out.push(Segment::Wildcard);
            } else {
                return Err(ProviderError::Invalid(
                    "only numeric indices and [*] are supported".into(),
                ));
            }
        } else {
            return Err(ProviderError::Invalid("unsupported JSONPath syntax".into()));
        }
    }
    Ok(out)
}
/// Returns a warning for literal credential-like header/body values, without exposing them.
pub fn literal_credential_warnings(
    headers: &BTreeMap<String, String>,
    body: Option<&Value>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    for (name, value) in headers {
        let lower = name.to_ascii_lowercase();
        if !value.contains("{{")
            && !value.trim().is_empty()
            && (lower == "authorization" || lower.contains("key") || lower.contains("token"))
        {
            warnings.push(format!(
                "header {name}: This looks like a key stored in the file; move it to Secrets"
            ));
        }
    }
    fn visit(value: &Value, path: &str, warnings: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    let field = key.to_ascii_lowercase();
                    let here = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    if matches!(field.as_str(), "api_key" | "key" | "auth_key") && value.is_string()
                    {
                        let text = value.as_str().unwrap_or_default();
                        if !text.trim().is_empty() && !text.contains("{{") {
                            warnings.push(format!("body {here}: This looks like a key stored in the file; move it to Secrets"));
                        }
                    }
                    visit(value, &here, warnings);
                }
            }
            Value::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    visit(item, &format!("{path}[{i}]"), warnings);
                }
            }
            _ => {}
        }
    }
    if let Some(body) = body {
        visit(body, "", &mut warnings);
    }
    warnings
}

/// Replaces secret occurrences (longest first) with a name-bearing mask.
pub fn redact_secrets(text: &str, names_and_values: &[(String, String)]) -> String {
    let mut secrets: Vec<_> = names_and_values
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .collect();
    secrets.sort_by_key(|(_, value)| std::cmp::Reverse(value.len()));
    let mut output = String::with_capacity(text.len());
    let mut remaining = text;
    while !remaining.is_empty() {
        if let Some((name, value)) = secrets
            .iter()
            .find(|(_, value)| remaining.starts_with(value.as_str()))
        {
            output.push_str(&format!("••••{name}"));
            remaining = &remaining[value.len()..];
        } else {
            let ch = remaining.chars().next().expect("nonempty remainder");
            output.push(ch);
            remaining = &remaining[ch.len_utf8()..];
        }
    }
    output
}

/// Whether an HTTP URL containing a secret requires user confirmation.
pub fn requires_insecure_http_confirmation(url: &str, contains_secret: bool) -> bool {
    if !contains_secret {
        return false;
    }
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return url
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http://"));
    };
    if parsed.scheme() != "http" {
        return false;
    }
    match parsed.host_str() {
        Some(host) if host.eq_ignore_ascii_case("localhost") => false,
        Some(host) => !host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback()),
        None => true,
    }
}

/// Build the disabled official LibreTranslate preset; its server address comes from settings.
pub fn libretranslate_preset() -> ValidatedProvider {
    const DEFINITION: &str = r#"
[provider]
id = "libretranslate"
name = "LibreTranslate"
max_chars = 5000
[languages]
auto = "auto"
[params]
server = { label = "Server address", type = "text" }
[request]
method = "POST"
url = "{{param.server}}/translate"
body.json = { q = "{{text}}", source = "{{src}}", target = "{{dst}}" }
[response]
text = { jsonpath = "$.translatedText" }
error = { jsonpath = "$.error" }
"#;
    ProviderDefinition::from_toml(DEFINITION).expect("built-in LibreTranslate definition is valid")
}

/// Map a LibreTranslate call without a key, or inject a caller-supplied optional key.
pub fn libretranslate_request(
    provider: &ValidatedProvider,
    text: &str,
    source: &str,
    target: &str,
    params: &BTreeMap<String, String>,
    key: Option<&str>,
) -> Result<MappedRequest, ProviderError> {
    if provider.definition().provider.id != "libretranslate" {
        return Err(ProviderError::Invalid(
            "expected LibreTranslate provider".into(),
        ));
    }
    let mut mapped =
        provider.map_request_with_values(text, source, target, params, &BTreeMap::new())?;
    if let Some(key) = key.filter(|key| !key.is_empty()) {
        if requires_insecure_http_confirmation(&mapped.url, true) {
            return Err(ProviderError::Invalid(
                "HTTP with a secret requires explicit user confirmation".into(),
            ));
        }
        if let Some(Value::Object(body)) = &mut mapped.body {
            body.insert("api_key".into(), Value::String(key.to_owned()));
        }
    }
    Ok(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn libretranslate_preset_maps_keyless_and_keyed_requests() {
        let provider = libretranslate_preset();
        assert!(!provider.definition().provider.enabled);
        let mut params = BTreeMap::new();
        assert!(matches!(
            libretranslate_request(&provider, "hello", "ja", "en", &params, None),
            Err(ProviderError::Template(_))
        ));
        params.insert("server".into(), "http://127.0.0.1:5000".into());
        let mapped =
            libretranslate_request(&provider, "hello", "auto", "fr", &params, None).unwrap();
        assert_eq!(mapped.method, "POST");
        assert_eq!(mapped.url, "http://127.0.0.1:5000/translate");
        assert_eq!(
            mapped.body.unwrap(),
            serde_json::json!({"q":"hello","source":"auto","target":"fr"})
        );
        let mapped =
            libretranslate_request(&provider, "hello", "en", "fr", &params, Some("private-key"))
                .unwrap();
        assert_eq!(mapped.body.unwrap()["api_key"], "private-key");
        params.insert("server".into(), "http://example.org".into());
        assert!(matches!(
            libretranslate_request(&provider, "hi", "en", "fr", &params, Some("private-key")),
            Err(ProviderError::Invalid(_))
        ));
        assert_eq!(
            provider
                .extract_translation(&serde_json::json!({"translatedText":"bonjour"}))
                .unwrap(),
            "bonjour"
        );
    }

    #[test]
    fn libretranslate_preset_reads_recorded_local_reply_and_error() {
        let provider = libretranslate_preset();
        let (url, server) = serve_once(200, r#"{"translatedText":"bonjour"}"#);
        let params = BTreeMap::from([("server".into(), url.trim_end_matches("/translate").into())]);
        let request =
            libretranslate_request(&provider, "こんにちは", "ja", "fr", &params, None).unwrap();
        let result = provider
            .extract_translation_from_bytes(&provider.send(&request).unwrap())
            .unwrap();
        assert_eq!(result, "bonjour");
        assert!(
            server
                .join()
                .unwrap()
                .starts_with("POST /translate HTTP/1.1")
        );
        assert!(matches!(
            provider.extract_translation_from_bytes(br#"{"error":"unsupported language"}"#),
            Err(ProviderError::Response(message)) if message.contains("unsupported language")
        ));
    }

    #[test]
    fn provider_parse_exposes_safe_literal_key_and_http_warnings() {
        let source = "[provider]\nid='safe'\nname='Safe'\n[request]\nmethod='POST'\nurl='http://example.org'\nheaders={Authorization='Bearer PLACEHOLDER'}\nbody.json={api_key='PLACEHOLDER', q='{{text}}'}\n[response]\ntext={jsonpath='$.translatedText'}\n";
        let provider = ProviderDefinition::from_toml(source).unwrap();
        let warnings = provider.warnings().join("\n");
        assert!(warnings.contains("header Authorization"));
        assert!(warnings.contains("body api_key"));
        assert!(!warnings.contains("PLACEHOLDER"));
        assert!(requires_insecure_http_confirmation(
            "HTTP://example.org",
            true
        ));
        assert!(!requires_insecure_http_confirmation(
            "http://127.2.3.4",
            true
        ));
        assert!(warnings.contains("requires explicit user confirmation"));
    }

    #[test]
    fn runtime_params_and_secrets_map_without_exposing_values() {
        let source = "[provider]\nid='preset'\nname='Preset'\n[params]\nserver={label='Server', type='text'}\n[request]\nmethod='POST'\nurl='{{param.server}}/translate'\nheaders={Authorization='Bearer {{secret.api-key}}'}\nbody.json={q='{{text}}', api_key='{{secret.api-key}}'}\n[response]\ntext={jsonpath='$.translatedText'}\n";
        let provider = ProviderDefinition::from_toml(source).unwrap();
        let mut params = BTreeMap::new();
        let mut secrets = BTreeMap::new();
        assert!(matches!(
            provider.map_request("a&b", "ja", "en"),
            Err(ProviderError::Template(_))
        ));
        params.insert("server".into(), "https://example.org".into());
        assert!(
            matches!(provider.map_request_with_values("a&b", "ja", "en", &params, &secrets), Err(ProviderError::NeedsSecret(name)) if name == "api-key")
        );
        secrets.insert("api-key".into(), "private value".into());
        let mapped = provider
            .map_request_with_values("a&b", "ja", "en", &params, &secrets)
            .unwrap();
        assert_eq!(mapped.url, "https://example.org/translate");
        assert_eq!(mapped.headers["Authorization"], "Bearer private value");
        assert_eq!(mapped.body.as_ref().unwrap()["q"], "a&b");
        assert_eq!(mapped.body.as_ref().unwrap()["api_key"], "private value");
        assert!(!format!("{mapped:?}").contains("private value"));
        secrets.insert("api-key".into(), String::new());
        assert!(matches!(
            provider.map_request_with_values("a&b", "ja", "en", &params, &secrets),
            Err(ProviderError::NeedsSecret(_))
        ));
        secrets.insert("api-key".into(), "private value".into());
        params.insert("server".into(), "http://example.org".into());
        assert!(matches!(
            provider.map_request_with_values("a&b", "ja", "en", &params, &secrets),
            Err(ProviderError::Invalid(_))
        ));
    }

    #[test]
    fn security_helpers() {
        assert!(
            literal_credential_warnings(
                &BTreeMap::new(),
                Some(&serde_json::json!({"nested":{"api_key":"secret"}}))
            )
            .iter()
            .all(|w| !w.contains("secret"))
        );
        assert_eq!(
            redact_secrets(
                "x secret secretish secret",
                &[
                    ("k".into(), "secret".into()),
                    ("long".into(), "secretish".into())
                ]
            ),
            "x ••••k ••••long ••••k"
        );
        assert_eq!(
            redact_secrets("unchanged", &[("k".into(), String::new())]),
            "unchanged"
        );
        assert!(!requires_insecure_http_confirmation(
            "http://127.0.0.1/x",
            true
        ));
        assert!(!requires_insecure_http_confirmation(
            "https://example.com",
            true
        ));
        assert!(requires_insecure_http_confirmation(
            "http://example.com",
            true
        ));
    }
    const LIBRE: &str = r#"[provider]
id = "my-libretranslate"
name = "LibreTranslate (my server)"
enabled = true
max_chars = 5000

[request]
method = "POST"
url = "https://translate.example.org/translate"
body.json = { q = "{{text}}", source = "{{src}}", target = "{{dst}}", api_key = "{{secret.libre-key}}" }

[response]
text = { jsonpath = "$.translatedText" }
error = { jsonpath = "$.error" }
"#;
    #[test]
    fn manual_send_splits_maps_and_joins_in_order() {
        let source = "[provider]\nid = 'local'\nname = 'Local'\nmax_chars = 4\n[request]\nmethod = 'POST'\nurl = 'https://example.org'\nbody.json = { q = '{{text}}' }\n[response]\ntext = { jsonpath = '$.translatedText' }\n";
        let provider = ProviderDefinition::from_toml(source).unwrap();
        let mut seen = Vec::new();
        let result = provider
            .send_manual_with("A.界!B?", "ja", "en", |request| {
                seen.push(
                    request.body.as_ref().unwrap()["q"]
                        .as_str()
                        .unwrap()
                        .to_owned(),
                );
                let value =
                    serde_json::json!({"translatedText": format!("<{}>", seen.last().unwrap())});
                Ok(serde_json::to_vec(&value).unwrap())
            })
            .unwrap();
        assert_eq!(seen, ["A.界!", "B?"]);
        assert_eq!(result, "<A.界!><B?>");
    }

    #[test]
    fn manual_send_rejects_long_sentence_before_transport() {
        let source = "[provider]\nid = 'local'\nname = 'Local'\nmax_chars = 3\n[request]\nmethod = 'POST'\nurl = 'https://example.org'\nbody.json = { q = '{{text}}' }\n[response]\ntext = { jsonpath = '$.translatedText' }\n";
        let provider = ProviderDefinition::from_toml(source).unwrap();
        let mut calls = 0;
        let error = provider
            .send_manual_with("界界界界.", "auto", "en", |_| {
                calls += 1;
                Ok(Vec::new())
            })
            .unwrap_err();
        assert!(error.to_string().contains("Local (5 characters, limit 3)"));
        assert_eq!(calls, 0);
    }

    #[test]
    fn folder_loader_keeps_valid_files_and_reports_duplicates_and_parse_errors() {
        use std::fs;
        let folder = std::env::temp_dir().join(format!(
            "ta-provider-folder-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&folder).unwrap();
        let valid = |id: &str| {
            format!(
                "[provider]\nid = '{id}'\nname = 'Local'\n[request]\nmethod = 'GET'\nurl = 'https://example.org'\n[response]\ntext = {{ jsonpath = '$.result' }}\n"
            )
        };
        fs::write(folder.join("a.toml"), valid("first")).unwrap();
        fs::write(folder.join("b.toml"), "[provider]\nid = [\n").unwrap();
        fs::write(folder.join("c.toml"), valid("first")).unwrap();
        fs::write(folder.join("d.toml"), valid("second")).unwrap();
        fs::write(folder.join("ignored.txt"), valid("third")).unwrap();
        let report = ProviderDefinition::load_folder(&folder);
        assert_eq!(report.providers.len(), 2);
        assert_eq!(report.providers[0].definition().provider.id, "first");
        assert_eq!(report.providers[1].definition().provider.id, "second");
        assert_eq!(report.errors.len(), 2);
        assert_eq!(report.errors[0].path, folder.join("b.toml"));
        assert!(
            report.errors[0].to_string().contains("line 2, column"),
            "{:?}",
            report.errors
        );
        assert!(report.errors[0].to_string().contains("b.toml"));
        assert_eq!(report.errors[1].path, folder.join("c.toml"));
        assert!(report.errors[1].to_string().contains("a.toml"));
        assert!(report.errors[1].to_string().contains("c.toml"));
        fs::remove_dir_all(folder).unwrap();
    }

    fn serve_once(status: u16, body: &'static str) -> (String, std::thread::JoinHandle<String>) {
        serve_raw_once(status, body.as_bytes(), "")
    }

    fn serve_raw_once(
        status: u16,
        body: &'static [u8],
        content_type: &'static str,
    ) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/translate", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut received = Vec::new();
            let mut bytes = [0u8; 4096];
            loop {
                let count = stream.read(&mut bytes).unwrap();
                assert!(count > 0, "client closed before request headers");
                received.extend_from_slice(&bytes[..count]);
                if received.windows(4).any(|part| part == b"\r\n\r\n") {
                    break;
                }
            }
            let header_end = received
                .windows(4)
                .position(|part| part == b"\r\n\r\n")
                .unwrap()
                + 4;
            let headers = String::from_utf8_lossy(&received[..header_end]);
            let length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            while received.len() - header_end < length {
                let count = stream.read(&mut bytes).unwrap();
                assert!(count > 0, "client closed before request body");
                received.extend_from_slice(&bytes[..count]);
            }
            let reply = format!(
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\n{content_type}Connection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(reply.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
            String::from_utf8(received).unwrap()
        });
        (url, handle)
    }

    #[test]
    fn transport_does_not_follow_redirects() {
        let (url, server) = serve_once(302, "redirect body");
        let source = format!(
            "[provider]\nid='local'\nname='Local'\n[request]\nmethod='GET'\nurl='{url}'\n[response]\ntext={{jsonpath='$.translatedText'}}"
        );
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        let request = provider.map_request("x", "ja", "en").unwrap();
        assert!(matches!(
            provider.send(&request),
            Err(ProviderError::HttpStatus(302))
        ));
        assert!(
            server
                .join()
                .unwrap()
                .starts_with("GET /translate HTTP/1.1")
        );
    }

    #[test]
    fn transport_sends_mapped_query_and_headers_once() {
        let (url, server) = serve_once(200, r#"{"translatedText":"done"}"#);
        let source = format!(
            "[provider]\nid='local'\nname='Local'\n[request]\nmethod='GET'\nurl='{url}?old=1'\nquery={{ 'term&'='{{{{text}}}}' }}\nheaders={{ 'X-Test'='yes' }}\n[response]\ntext={{jsonpath='$.translatedText'}}"
        );
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        let request = provider.map_request("a & b", "ja", "en").unwrap();
        let bytes = provider.send(&request).unwrap();
        assert_eq!(
            provider.extract_translation_from_bytes(&bytes).unwrap(),
            "done"
        );
        let received = server.join().unwrap();
        assert!(
            received.starts_with("GET /translate?old=1&term%26=a+%26+b HTTP/1.1"),
            "{received}"
        );
        assert!(received.to_ascii_lowercase().contains("x-test: yes"));
    }

    #[test]
    fn literal_query_key_is_hidden_from_service_errors() {
        let (url, server) = serve_once(403, r#"{"error":"invalid API key KEY123"}"#);
        let source = format!(
            "[provider]\nid='local'\nname='Local'\n[request]\nmethod='GET'\nurl='{url}'\nquery={{api_key='KEY123'}}\n[response]\ntext={{jsonpath='$.translatedText'}}\nerror={{jsonpath='$.error'}}"
        );
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        let request = provider.map_request("hello", "en", "fr").unwrap();
        let error = provider.send_translation(&request).unwrap_err();
        assert!(error.to_string().contains("service error"));
        assert!(!error.to_string().contains("KEY123"));
        server.join().unwrap();
    }

    #[test]
    fn send_reports_http_status_before_reading_an_oversized_error_body() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/translate", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let amount = stream.read(&mut request).unwrap();
            assert!(amount > 0);
            stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 9437184\r\nConnection: close\r\n\r\n").unwrap();
        });
        let source = format!(
            "[provider]\nid='local'\nname='Local'\n[request]\nmethod='GET'\nurl='{url}'\n[response]\ntext={{jsonpath='$.translatedText'}}"
        );
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        let request = provider.map_request("hello", "en", "fr").unwrap();
        assert_eq!(
            provider.send(&request).unwrap_err(),
            ProviderError::HttpStatus(403)
        );
        server.join().unwrap();
    }

    #[test]
    fn transport_service_error_precedes_http_status_without_exposing_body() {
        let (url, server) = serve_once(
            403,
            r#"{"error":"quota exceeded","translatedText":"ignored"}"#,
        );
        let source = format!(
            "[provider]\nid='local'\nname='Local'\n[request]\nmethod='GET'\nurl='{url}'\n[response]\ntext={{jsonpath='$.translatedText'}}\nerror={{jsonpath='$.error'}}"
        );
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        let request = provider.map_request("hello", "en", "fr").unwrap();
        assert!(matches!(
            provider.send_translation(&request),
            Err(ProviderError::Response(message)) if message.contains("Local: quota exceeded") && !message.contains("ignored")
        ));
        server.join().unwrap();

        let (url, server) = serve_once(403, "SECRET_BODY");
        let source = format!(
            "[provider]\nid='local'\nname='Local'\n[request]\nmethod='GET'\nurl='{url}'\n[response]\ntext={{jsonpath='$.translatedText'}}"
        );
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        let request = provider.map_request("hello", "en", "fr").unwrap();
        let error = provider.send_translation(&request).unwrap_err();
        assert_eq!(error.to_string(), "HTTP 403 Forbidden");
        assert!(!error.to_string().contains("SECRET_BODY"));
        server.join().unwrap();
    }

    #[test]
    fn transport_error_does_not_echo_a_secret_from_url() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let source = format!(
            "[provider]\nid='local'\nname='Local'\ntimeout_seconds=1\n[request]\nmethod='GET'\nurl='http://127.0.0.1:{port}/{{{{secret.api-key}}}}'\n[response]\ntext={{jsonpath='$.translatedText'}}"
        );
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        let secrets = std::collections::BTreeMap::from([("api-key".into(), "KEY123".into())]);
        let request = provider
            .map_request_with_values("hello", "en", "fr", &Default::default(), &secrets)
            .unwrap();
        let error = provider.send_translation(&request).unwrap_err();
        assert!(!error.to_string().contains("KEY123"));
    }

    #[test]
    fn service_error_does_not_repeat_a_request_secret() {
        let (url, server) =
            serve_once(403, r#"{"error":"echo KEY123","translatedText":"ignored"}"#);
        let source = format!(
            "[provider]\nid='local'\nname='Local'\n[request]\nmethod='GET'\nurl='{url}'\nheaders={{Authorization='Bearer {{{{secret.api-key}}}}'}}\n[response]\ntext={{jsonpath='$.translatedText'}}\nerror={{jsonpath='$.error'}}"
        );
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        let secrets =
            std::collections::BTreeMap::from([("api-key".to_owned(), "KEY123".to_owned())]);
        let request = provider
            .map_request_with_values("hello", "en", "fr", &Default::default(), &secrets)
            .unwrap();
        let error = provider.send_translation(&request).unwrap_err();
        assert!(error.to_string().contains("Local: service error"));
        assert!(!error.to_string().contains("KEY123"));
        server.join().unwrap();
    }

    #[test]
    fn transport_decodes_declared_charset_and_replaces_invalid_utf8() {
        let (url, server) = serve_raw_once(
            200,
            b"{\"translatedText\":\"caf\xe9\"}",
            "Content-Type: application/json; charset=iso-8859-1\r\n",
        );
        let source = format!(
            "[provider]\nid='local'\nname='Local'\n[request]\nmethod='GET'\nurl='{url}'\n[response]\ntext={{jsonpath='$.translatedText'}}"
        );
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        let request = provider.map_request("hello", "en", "fr").unwrap();
        assert_eq!(provider.send_translation(&request).unwrap(), "café");
        server.join().unwrap();

        let (url, server) = serve_raw_once(200, b"{\"translatedText\":\"bad\xff\"}", "");
        let source = source.replace(&request.url, &url);
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        let request = provider.map_request("hello", "en", "fr").unwrap();
        assert_eq!(provider.send_translation(&request).unwrap(), "bad�");
        server.join().unwrap();
    }

    #[test]
    fn transport_reports_malformed_json_without_reply_text() {
        let (url, server) = serve_once(200, "<html>login page</html>");
        let source = format!(
            "[provider]\nid='local'\nname='Local'\n[request]\nmethod='GET'\nurl='{url}'\n[response]\ntext={{jsonpath='$.translatedText'}}"
        );
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        let request = provider.map_request("hello", "en", "fr").unwrap();
        let error = provider.send_translation(&request).unwrap_err();
        assert!(error.to_string().contains("Reply was not JSON"));
        assert!(!error.to_string().contains("login page"));
        server.join().unwrap();
    }

    #[test]
    fn transport_posts_json_and_hides_non_success_body() {
        let (url, server) = serve_once(403, "do not expose this body");
        let source = format!(
            "[provider]\nid='local'\nname='Local'\n[request]\nmethod='POST'\nurl='{url}'\nbody.json={{ q='{{{{text}}}}' }}\n[response]\ntext={{jsonpath='$.translatedText'}}"
        );
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        let request = provider.map_request("こんにちは", "ja", "en").unwrap();
        let error = provider.send(&request).unwrap_err();
        assert!(matches!(error, ProviderError::HttpStatus(403)));
        assert!(!error.to_string().contains("do not expose this body"));
        let received = server.join().unwrap();
        assert!(received.starts_with("POST /translate HTTP/1.1"));
        assert!(
            received
                .to_ascii_lowercase()
                .contains("content-type: application/json")
        );
        assert!(received.contains("こんにちは"));
    }

    #[test]
    fn response_charset_and_invalid_utf8_are_lossy_and_default_to_utf8() {
        assert_eq!(decode_response_body(b"caf\xe9", Some("ISO-8859-1")), "café");
        assert_eq!(decode_response_body(b"a\xffb", None), "a\u{fffd}b");
        assert_eq!(
            charset_from_content_type("text/plain; charset=latin1").as_deref(),
            Some("latin1")
        );
    }

    #[test]
    fn exact_documented_schema_parses_and_maps_json_body() {
        let d: ProviderDefinition = toml::from_str(LIBRE).unwrap();
        assert_eq!(d.provider.id, "my-libretranslate");
        assert_eq!(d.provider.name, "LibreTranslate (my server)");
        assert!(d.provider.enabled);
        assert_eq!(d.provider.max_chars, 5000);
        assert_eq!(d.request.method, "POST");
        assert_eq!(d.request.url, "https://translate.example.org/translate");
        let body = d.request.body.as_ref().unwrap();
        assert_eq!(
            map_json(&body.json, "Hello", "en", "fr"),
            serde_json::json!({"q":"Hello","source":"en","target":"fr","api_key":"{{secret.libre-key}}"})
        );
        assert_eq!(d.response.text.jsonpath, "$.translatedText");
        assert_eq!(d.response.error.unwrap().jsonpath, "$.error");
    }
    #[test]
    fn guide_optional_provider_fields_have_defaults() {
        let d = ProviderDefinition::from_toml(
            &LIBRE
                .replace("enabled = true\nmax_chars = 5000\n", "")
                .replace(", api_key = \"{{secret.libre-key}}\"", ""),
        )
        .unwrap();
        assert!(!d.definition().provider.enabled);
        assert!(!d.definition().provider.unofficial);
        assert!(!d.definition().provider.auto_translate);
        assert_eq!(d.definition().provider.max_chars, 1000);
        assert_eq!(d.definition().provider.timeout_seconds, 15);
        assert!(!d.definition().response.html_decode);
    }
    #[test]
    fn missing_provider_identity_is_rejected() {
        for bad in [
            LIBRE.replace("id = \"my-libretranslate\"\n", ""),
            LIBRE.replace("name = \"LibreTranslate (my server)\"\n", ""),
        ] {
            assert!(matches!(
                ProviderDefinition::from_toml(&bad),
                Err(ProviderError::Parse(_))
            ));
        }
    }
    #[test]
    fn language_mapping_and_auto_source_apply_to_placeholders() {
        let s = LIBRE.replace("enabled = true\nmax_chars = 5000\n", "")
            .replace("[request]", "[languages]\nauto = \"detect\"\nmap = { \"zh-Hans\" = \"zh-CN\" }\nonly = [\"en\", \"zh-Hans\"]\n\n[request]")
            .replace("{{secret.libre-key}}", "key");
        let p = ProviderDefinition::from_toml(&s).unwrap();
        let mapped = p.map_request("hello", "auto", "zh-Hans").unwrap();
        let body = mapped.body.unwrap();
        assert_eq!(body["source"], "detect");
        assert_eq!(body["target"], "zh-CN");
        assert!(p.map_request("hello", "en", "fr").is_err());
        assert!(
            p.map_request("x".repeat(5001).as_str(), "en", "zh-Hans")
                .is_err()
        );
    }
    #[test]
    fn empty_auto_source_omits_whole_query_and_json_fields() {
        let s = LIBRE
            .replace(
                "[request]\n",
                "[languages]\nauto = \"\"\n\n[request]\nquery = { source = \"{{src}}\" }\n",
            )
            .replace("{{secret.libre-key}}", "key");
        let p = ProviderDefinition::from_toml(&s).unwrap();
        let mapped = p.map_request("hello", "auto", "fr").unwrap();
        assert!(!mapped.query.contains_key("source"));
        assert!(mapped.body.unwrap().get("source").is_none());
    }
    #[test]
    fn empty_auto_source_rejects_mixed_source_template() {
        let s = LIBRE
            .replace("[request]\n", "[languages]\nauto = \"\"\n\n[request]\n")
            .replace("source = \"{{src}}\"", "source = \"prefix-{{src}}\"")
            .replace("{{secret.libre-key}}", "key");
        let p = ProviderDefinition::from_toml(&s).unwrap();
        assert!(matches!(
            p.map_request("hello", "auto", "fr"),
            Err(ProviderError::Template(_))
        ));
    }
    #[test]
    fn secret_placeholder_fails_closed() {
        let provider = ProviderDefinition::from_toml(LIBRE).unwrap();
        assert!(matches!(
            provider.map_request("hello", "ja", "en"),
            Err(ProviderError::NeedsSecret(name)) if name == "libre-key"
        ));
    }
    #[test]
    fn query_mapping_encodes_and_is_single_pass() {
        let s = "[provider]\nid='test'\nname='Test'\n[request]\nmethod='POST'\nurl='https://example.test/{{text}}'\n[request.query]\nq='{{text}}'\nlang='{{src}}-{{dst}}'\n[response]\ntext={jsonpath='$.text'}";
        let p = ProviderDefinition::from_toml(s).unwrap();
        let m = p.map_request("{{dst}} &", "en", "fr").unwrap();
        assert_eq!(m.url, "https://example.test/%7B%7Bdst%7D%7D%20%26");
        assert_eq!(m.query["q"], "{{dst}} &");
        assert_eq!(m.query["lang"], "en-fr");
    }
    #[test]
    fn unknown_fields_warn_and_keep_definition_defaults() {
        let source = "[provider]\nid = 'test'\nname = 'Test'\nmax_char = 'TOP_SECRET'\nfuture_flag = 1\n[request]\nmethod = 'GET'\nurl = 'https://example.org'\n[response]\ntext = { jsonpath = '$.result' }\n";
        let parsed = ProviderDefinition::from_toml(source).unwrap();
        assert_eq!(parsed.definition().provider.max_chars, 1000);
        assert_eq!(
            parsed.warnings(),
            &[
                "provider.future_flag".to_string(),
                "provider.max_char".to_string()
            ]
        );
        assert!(
            parsed
                .warnings()
                .iter()
                .all(|warning| !warning.contains("TOP_SECRET"))
        );
    }
    #[test]
    fn jsonpath_numeric_indices_work() {
        let p = ProviderDefinition::from_toml(
            &LIBRE
                .replace(", api_key = \"{{secret.libre-key}}\"", "")
                .replace("$.translatedText", "$.items[0].text"),
        )
        .unwrap();
        assert_eq!(
            p.extract_translation(&serde_json::json!({"items":[{"text":"ok"}]}))
                .unwrap(),
            "ok"
        );
    }

    #[test]
    fn jsonpath_wildcard_joins_string_matches_and_missing_path_errors() {
        let source = LIBRE
            .replace(", api_key = \"{{secret.libre-key}}\"", "")
            .replace("$.translatedText", "$.items[*].text")
            .replace(
                "text = { jsonpath = \"$.items[*].text\" }",
                "text = { jsonpath = \"$.items[*].text\", join = \" | \" }",
            );
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        assert_eq!(
            provider
                .extract_translation(&serde_json::json!({"items":[{"text":"A"},{"text":"B"}]}))
                .unwrap(),
            "A | B"
        );
        assert!(matches!(
            provider.extract_translation(&serde_json::json!({"items":[]})),
            Err(ProviderError::Response(_))
        ));
    }

    #[test]
    fn jsonpath_wildcard_rejects_non_string_match() {
        let source = LIBRE
            .replace(", api_key = \"{{secret.libre-key}}\"", "")
            .replace("$.translatedText", "$.items[*].text");
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        assert_eq!(
            provider
                .extract_translation(&serde_json::json!({"items":[{"text":"A"},{"text":null}]}))
                .unwrap(),
            "A"
        );
        assert!(matches!(
            provider.extract_translation(
                &serde_json::json!({"items":[{"text":null},{"text":{"nested":true}}]})
            ),
            Err(ProviderError::Response(_))
        ));
    }

    #[test]
    fn nested_wildcards_skip_null_and_join_numbers() {
        let source = LIBRE
            .replace(", api_key = \"{{secret.libre-key}}\"", "")
            .replace("$.translatedText", "$[0][*][0]")
            .replace(
                "text = { jsonpath = \"$[0][*][0]\" }",
                "text = { jsonpath = \"$[0][*][0]\", join = \"-\" }",
            );
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        assert_eq!(
            provider
                .extract_translation_with_match_count(&serde_json::json!([[
                    ["A", null],
                    ["B"],
                    [4]
                ]]))
                .unwrap(),
            ("A-B-4".into(), 3)
        );
    }

    #[test]
    fn provider_error_takes_precedence_over_translation() {
        let source = LIBRE.replace(", api_key = \"{{secret.libre-key}}\"", "");
        let provider = ProviderDefinition::from_toml(&source).unwrap();
        let error =
            provider.extract_translation_from_bytes(br#"{"error":"quota","translatedText":"ok"}"#);
        assert!(
            matches!(error, Err(ProviderError::Response(message)) if message.contains("quota"))
        );
    }

    #[test]
    fn regex_error_takes_precedence_and_default_returns_first_match() {
        let source = "[provider]\nid='test'\nname='Test'\n[request]\nmethod='GET'\nurl='https://example.test'\n[response]\ntext={regex='<span>(.*?)</span>'}\nerror={regex='Error: ([^<]+)'}";
        let provider = ProviderDefinition::from_toml(source).unwrap();
        assert_eq!(
            provider
                .extract_translation_from_bytes(b"<span>A</span><span>B</span>")
                .unwrap(),
            "A"
        );
        let error = provider.extract_translation_from_bytes(b"Error: blocked<span>A</span>");
        assert!(
            matches!(error, Err(ProviderError::Response(message)) if message.contains("blocked"))
        );
    }

    #[test]
    fn response_extraction_supports_regex_join_and_html_entities() {
        let s = "[provider]\nid='test'\nname='Test'\n[request]\nmethod='GET'\nurl='https://example.test'\n[response]\nhtml_decode=true\ntext={regex='<span>(.*?)</span>', join=' | '}";
        let provider = ProviderDefinition::from_toml(s).unwrap();
        assert_eq!(
            provider
                .extract_translation_from_bytes(
                    br#"<span>A &amp; B</span><span>&#39;C&#39;</span>"#
                )
                .unwrap(),
            "A & B | 'C'"
        );
    }
}
