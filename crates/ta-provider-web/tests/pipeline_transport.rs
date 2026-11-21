//! Offline contract tests for routing decisions and web transport.

use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};
use ta_provider_web::{ProviderDefinition, ProviderError, ValidatedProvider};
use ta_translate::{
    RequestMode, RoutingRejection, RoutingRequest, TranslatorCapabilities, TranslatorKind,
    routing_request_rejection,
};

fn provider(base: &str) -> ValidatedProvider {
    ProviderDefinition::from_toml(&format!(
        r#"
[provider]
id = "offline"
name = "Offline"
enabled = true
max_chars = 500
[languages]
only = ["en", "fr"]
map = {{ en = "en-US", fr = "fr-FR" }}
[request]
method = "POST"
url = "{base}/translate"
[request.body]
content_type = "application/json"
json = {{ text = "{{{{text}}}}", source = "{{{{src}}}}", target = "{{{{dst}}}}" }}
[response.text]
jsonpath = "$.translation"
"#
    ))
    .unwrap()
}

fn server(status: u16, body: &'static str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        let count = stream.read(&mut request).unwrap();
        write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        String::from_utf8_lossy(&request[..count]).into_owned()
    });
    (format!("http://{address}"), handle)
}

fn capabilities() -> TranslatorCapabilities {
    let mut caps = TranslatorCapabilities::new(TranslatorKind::Remote);
    caps.id = "offline".into();
    caps.name = "Offline".into();
    caps.supported_source_languages = Some(vec!["en".into()]);
    caps.supported_target_languages = Some(vec!["fr".into()]);
    caps
}

fn eligible(source: &str, target: &str, remote_permission: bool) -> Result<(), RoutingRejection> {
    let caps = capabilities();
    routing_request_rejection(RoutingRequest {
        capabilities: &caps,
        provider_enabled: true,
        credentials_available: true,
        remote_permission,
        auto_translate_enabled: true,
        source,
        target,
        text: "hello",
        mode: RequestMode::Manual,
    })
    .map_or(Ok(()), Err)
}

#[test]
fn eligible_routing_maps_sends_and_extracts_but_rejections_never_connect() {
    let (base, received) = server(200, r#"{"translation":"bonjour"}"#);
    assert_eq!(eligible("en", "fr", true), Ok(()));
    let connected_provider = provider(&base);
    let mapped = connected_provider.map_request("hello", "en", "fr").unwrap();
    assert_eq!(
        mapped.body.unwrap(),
        serde_json::json!({"text":"hello","source":"en-US","target":"fr-FR"})
    );
    assert_eq!(
        connected_provider
            .send_translation(&connected_provider.map_request("hello", "en", "fr").unwrap())
            .unwrap(),
        "bonjour"
    );
    let request = received.join().unwrap();
    assert!(request.starts_with("POST /translate HTTP/1.1"));
    assert!(request.contains("\"source\":\"en-US\""));

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    assert_eq!(
        eligible("en", "fr", false),
        Err(RoutingRejection::RemotePermissionRequired)
    );
    let p = provider(&base);
    assert_eq!(
        p.map_request("hello", "en", "fr").unwrap().url,
        format!("{base}/translate")
    );

    assert!(matches!(
        eligible("en", "de", true),
        Err(RoutingRejection::Provider(_))
    ));
    assert!(matches!(
        provider(&base).map_request("hello", "en", "de"),
        Err(ProviderError::Invalid(_))
    ));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn non_success_status_does_not_expose_server_body() {
    let (base, received) = server(503, r#"{"secret":"private diagnostic"}"#);
    let result = provider(&base)
        .send_translation(&provider(&base).map_request("hello", "en", "fr").unwrap());
    assert_eq!(result, Err(ProviderError::HttpStatus(503)));
    assert!(!format!("{result:?}").contains("private diagnostic"));
    assert!(
        received
            .join()
            .unwrap()
            .starts_with("POST /translate HTTP/1.1")
    );
}
