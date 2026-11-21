use std::{collections::BTreeMap, io};
use ta_provider_web::{
    MappedRequest, ProviderError, ValidatedProvider, redact_secrets,
    requires_insecure_http_confirmation,
};

pub(crate) fn map_custom_request(
    provider: &ValidatedProvider,
    text: &str,
    languages: (&str, &str),
    params: &BTreeMap<String, String>,
    secrets: &mut BTreeMap<String, String>,
    read_key: fn(&str) -> io::Result<Option<String>>,
) -> Result<MappedRequest, String> {
    let request = loop {
        match provider.map_request_with_values(text, languages.0, languages.1, params, secrets) {
            Ok(request) => break request,
            Err(ProviderError::NeedsSecret(name)) => {
                if secrets.contains_key(&name) || secrets.len() >= 64 {
                    return Err("Needs key".into());
                }
                let value = read_key(&name)
                    .map_err(|_| "Needs key (credential store unavailable)".to_owned())?
                    .ok_or_else(|| "Needs key".to_owned())?;
                secrets.insert(name, value);
            }
            Err(error) => {
                let values: Vec<_> = secrets
                    .iter()
                    .map(|(name, value)| (name.clone(), value.clone()))
                    .collect();
                return Err(format!(
                    "provider request: {}",
                    redact_secrets(&error.to_string(), &values)
                ));
            }
        }
    };
    if requires_insecure_http_confirmation(&request.url, true) {
        return Err("refusing non-loopback HTTP provider in console mode".into());
    }
    Ok(request)
}
