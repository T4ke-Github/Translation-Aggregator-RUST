//! Declarative web translation provider schema, mapping, and HTTP transport.
//! Unsupported here: sessions, secret resolution, retries, auth flows, and advanced JSONPath.
#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
/// A declarative web translation provider configuration.
/// Provider identity and capabilities.
pub struct ProviderDefinition {
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
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for ProviderError {}
#[derive(Clone, Debug, PartialEq)]
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

fn prune_unknown(value: &mut toml::Value, path: &str, warnings: &mut Vec<String>) {
    let Some(table) = value.as_table_mut() else {
        return;
    };
    let schema: &[&str] = match path {
        "" => &["provider", "languages", "request", "response"],
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
impl ValidatedProvider {
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
        use std::io::Read;
        const MAX: u64 = 8 * 1024 * 1024;
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(
                self.0.provider.timeout_seconds.max(1),
            ))
            .build()
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        let mut outgoing = client.request(
            request
                .method
                .parse::<reqwest::Method>()
                .map_err(|e| ProviderError::Transport(e.to_string()))?,
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
        let mut response = outgoing.send().map_err(|e| {
            if e.is_timeout() {
                ProviderError::Timeout
            } else {
                ProviderError::Transport(e.to_string())
            }
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::HttpStatus(status.as_u16()));
        }
        if response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .is_some_and(|n| n > MAX)
        {
            return Err(ProviderError::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        response
            .by_ref()
            .take(MAX + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| ProviderError::Transport(e.kind().to_string()))?;
        if bytes.len() as u64 > MAX {
            return Err(ProviderError::ResponseTooLarge);
        }
        Ok(bytes)
    }
    /// Applies placeholders to construct a request.
    pub fn map_request(
        &self,
        text: &str,
        source: &str,
        target: &str,
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
        Ok(MappedRequest {
            method: r.method.clone(),
            url: substitute(&r.url, text, source, target, Context::Url),
            query: r
                .query
                .iter()
                .filter(|(_, v)| !omit_source || v.as_str() != "{{src}}")
                .map(|(k, v)| (k.clone(), substitute(v, text, source, target, Context::Raw)))
                .collect(),
            headers: r
                .headers
                .iter()
                .map(|(k, v)| (k.clone(), substitute(v, text, source, target, Context::Raw)))
                .collect(),
            body: r
                .body
                .as_ref()
                .map(|b| {
                    if omit_source {
                        map_json_without_source(&b.json, text, target)
                    } else {
                        Ok(map_json(&b.json, text, source, target))
                    }
                })
                .transpose()?,
        })
    }
    /// Extracts a translation string from a JSON response.
    pub fn extract_translation(&self, response: &Value) -> Result<String, ProviderError> {
        let selector = &self.0.response.text;
        let matches = select_json_many(response, &selector.jsonpath)?;
        let strings = matches
            .into_iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| ProviderError::Response("selected value is not a string".into()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        match &selector.join {
            Some(separator) => Ok(strings.join(separator)),
            None if strings.len() == 1 => Ok(strings.into_iter().next().expect("one match")),
            None => Ok(strings.into_iter().next().expect("matches are nonempty")),
        }
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
            if let Some(message) = re.captures(&raw).and_then(|c| c.get(1)) {
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
                && let Ok(error) = select_json(&json, &error_selector.jsonpath)
            {
                let message = error.as_str().unwrap_or("");
                if !message.is_empty() {
                    return Err(ProviderError::Response(format!(
                        "{}: {message}",
                        self.0.provider.name
                    )));
                }
            }
            self.extract_translation(&json)?
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

fn select_json<'a>(response: &'a Value, path: &str) -> Result<&'a Value, ProviderError> {
    let mut value = response;
    for segment in segments(path)? {
        value = match segment {
            Segment::Key(key) => value.get(key),
            Segment::Index(index) => value.get(index),
            Segment::Wildcard => None,
        }
        .ok_or_else(|| ProviderError::Response(format!("path {path} did not match")))?;
    }
    Ok(value)
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
fn substitute(s: &str, text: &str, src: &str, dst: &str, ctx: Context) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < s.len() {
        if s[i..].starts_with("{{")
            && let Some(end) = s[i + 2..].find("}}")
        {
            let key = &s[i + 2..i + 2 + end];
            let val = match key {
                "text" => Some(text),
                "src" => Some(src),
                "dst" => Some(dst),
                _ => None,
            };
            if let Some(val) = val {
                out.push_str(&match ctx {
                    Context::Raw => val.to_owned(),
                    Context::Url => percent_encode(val),
                });
                i += end + 4;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}
fn validate_template(s: &str) -> Result<(), ProviderError> {
    let mut rest = s;
    while let Some(i) = rest.find("{{") {
        rest = &rest[i + 2..];
        let e = rest
            .find("}}")
            .ok_or_else(|| ProviderError::Template("unclosed placeholder".into()))?;
        if !matches!(&rest[..e], "text" | "src" | "dst") {
            return Err(ProviderError::Template(format!(
                "unsupported placeholder {{{{{}}}}}",
                &rest[..e]
            )));
        }
        rest = &rest[e + 2..];
    }
    Ok(())
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
fn map_json(v: &Value, t: &str, s: &str, d: &str) -> Value {
    match v {
        Value::String(x) => Value::String(substitute(x, t, s, d, Context::Raw)),
        Value::Array(a) => Value::Array(a.iter().map(|x| map_json(x, t, s, d)).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| (k.clone(), map_json(x, t, s, d)))
                .collect(),
        ),
        _ => v.clone(),
    }
}
fn map_json_without_source(v: &Value, text: &str, target: &str) -> Result<Value, ProviderError> {
    match v {
        Value::Object(values) => values
            .iter()
            .filter(|(_, value)| value.as_str() != Some("{{src}}"))
            .map(|(key, value)| Ok((key.clone(), map_json_without_source(value, text, target)?)))
            .collect::<Result<serde_json::Map<String, Value>, ProviderError>>()
            .map(Value::Object),
        Value::Array(values) => values
            .iter()
            .map(|value| map_json_without_source(value, text, target))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::String(s) if s.contains("{{src}}") => Err(ProviderError::Template(
            "auto source cannot be omitted from a mixed template".into(),
        )),
        _ => Ok(map_json(v, text, "", target)),
    }
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
#[cfg(test)]
mod tests {
    use super::*;
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
            report.errors[0].to_string().contains("line 3, column"),
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
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(reply.as_bytes()).unwrap();
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
        assert!(matches!(
            ProviderDefinition::from_toml(LIBRE),
            Err(ProviderError::Template(_))
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
        assert!(matches!(
            provider
                .extract_translation(&serde_json::json!({"items":[{"text":"A"},{"text":null}]})),
            Err(ProviderError::Response(_))
        ));
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
        let p = ProviderDefinition::from_toml(s).unwrap();
        assert_eq!(
            p.extract_translation_from_bytes(b"<span>A &amp; B</span><span>&quot;C&quot;</span>")
                .unwrap(),
            "A & B | \"C\""
        );
    }
}
