//! Basic declarative web translation provider schema and mapping (no HTTP client).
//! Unsupported here: sessions, secret resolution, HTTP execution, retries, auth flows,
//! response status handling, and advanced JSONPath/filter expressions.
#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
    /// Maximum text length per request, if supplied.
    pub max_chars: Option<usize>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
/// Response extraction configuration.
pub struct Response {
    /// Selector for the translated text.
    pub text: Selector,
    #[serde(default)]
    /// Optional selector for provider error information.
    pub error: Option<Selector>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
/// A supported, restricted JSONPath selector.
pub struct Selector {
    /// Path to the selected response value.
    pub jsonpath: String,
}
#[derive(Clone, Debug)]
/// A provider definition that passed schema validation.
pub struct ValidatedProvider(ProviderDefinition);
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
impl ProviderDefinition {
    /// Parses and validates a TOML provider definition.
    pub fn from_toml(s: &str) -> Result<ValidatedProvider, ProviderError> {
        let d: Self = toml::from_str(s).map_err(|e| ProviderError::Parse(e.to_string()))?;
        d.validate()
    }
    /// Checks configuration constraints and returns a validated provider.
    pub fn validate(self) -> Result<ValidatedProvider, ProviderError> {
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
        if path.is_empty() {
            return Err(ProviderError::Invalid(
                "response.text.jsonpath is required".into(),
            ));
        }
        segments(path)?;
        for s in std::iter::once(self.request.url.as_str())
            .chain(self.request.query.values().map(String::as_str))
            .chain(self.request.headers.values().map(String::as_str))
        {
            validate_template(s)?;
        }
        if let Some(body) = &self.request.body {
            validate_json(&body.json)?;
        }
        Ok(ValidatedProvider(self))
    }
}
impl ValidatedProvider {
    /// Returns the validated provider definition.
    pub fn definition(&self) -> &ProviderDefinition {
        &self.0
    }
    /// Applies placeholders to construct a request.
    pub fn map_request(&self, text: &str, source: &str, target: &str) -> MappedRequest {
        let r = &self.0.request;
        MappedRequest {
            method: r.method.clone(),
            url: substitute(&r.url, text, source, target, Context::Url),
            query: r
                .query
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        substitute(v, text, source, target, Context::Query),
                    )
                })
                .collect(),
            headers: r
                .headers
                .iter()
                .map(|(k, v)| (k.clone(), substitute(v, text, source, target, Context::Raw)))
                .collect(),
            body: r
                .body
                .as_ref()
                .map(|b| map_json(&b.json, text, source, target)),
        }
    }
    /// Extracts a translation string from a JSON response.
    pub fn extract_translation(&self, response: &Value) -> Result<String, ProviderError> {
        let path = &self.0.response.text.jsonpath;
        let mut v = response;
        for seg in segments(path)? {
            v = match seg {
                Segment::Key(k) => v.get(k),
                Segment::Index(i) => v.get(i),
            }
            .ok_or_else(|| ProviderError::Response(format!("path {path} did not match")))?;
        }
        v.as_str()
            .map(str::to_owned)
            .ok_or_else(|| ProviderError::Response("selected value is not a string".into()))
    }
}
#[derive(Clone, Copy)]
enum Context {
    Raw,
    Url,
    Query,
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
                    Context::Url | Context::Query => percent_encode(val),
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
            } else {
                return Err(ProviderError::Invalid(
                    "only numeric JSONPath indices are supported".into(),
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
    fn exact_documented_schema_parses_and_maps_json_body() {
        let d: ProviderDefinition = toml::from_str(LIBRE).unwrap();
        assert_eq!(d.provider.id, "my-libretranslate");
        assert_eq!(d.provider.name, "LibreTranslate (my server)");
        assert!(d.provider.enabled);
        assert_eq!(d.provider.max_chars, Some(5000));
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
    fn secret_placeholder_fails_closed() {
        assert!(matches!(
            ProviderDefinition::from_toml(LIBRE),
            Err(ProviderError::Template(_))
        ));
    }
    #[test]
    fn query_mapping_encodes_and_is_single_pass() {
        let s = "[request]\nmethod='POST'\nurl='https://example.test/{{text}}'\n[request.query]\nq='{{text}}'\nlang='{{src}}-{{dst}}'\n[response]\ntext={jsonpath='$.text'}";
        let p = ProviderDefinition::from_toml(s).unwrap();
        let m = p.map_request("{{dst}} &", "en", "fr");
        assert_eq!(m.url, "https://example.test/%7B%7Bdst%7D%7D%20%26");
        assert_eq!(m.query["q"], "%7B%7Bdst%7D%7D%20%26");
        assert_eq!(m.query["lang"], "en-fr");
    }
    #[test]
    fn unknown_fields_are_rejected() {
        assert!(
            ProviderDefinition::from_toml(
                &LIBRE.replace("enabled = true", "enabled = true\nfuture_option = 1")
            )
            .is_err()
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
}
