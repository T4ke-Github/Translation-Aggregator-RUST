# 02 Local API (`API`)

## 1. Purpose and scope

The local API lets other programs on the same PC send text to the app and receive the incoming text
and its translations. It is the second main input next to the clipboard (D5), the channel the hook
plugin uses (D6), and the output for overlays, OBS browser sources and scripts.

Covered here: the HTTP server, `POST /v1/text`, `GET /v1/status`, the WebSocket and SSE output
stream, the token, request checks (`Host`, `Origin`, content type), size and rate limits, flood
behaviour, error responses, and the versioned API document.

Not covered here: what happens to the text after it is accepted (contexts and filters: see the IN
spec; translation: see the PIP spec), how the hook plugin forms its context ids (see the HOOK
spec), and plugin JSON-RPC (see the PLG spec).

## 2. Goals

1. Any local program can send text with one HTTP request and see it translated like clipboard text
   (D5).
2. The API is safe by default: off until the user turns it on, reachable only from this machine,
   and not usable by web pages in the user's browser (D18).
3. A local reader of translations (overlay, OBS) gets a live stream, protected by a token (O12).
4. A misbehaving or flooding client cannot freeze the app or spend the user's translation budget.
5. The API is versioned and documented, so third-party tools keep working across app releases.

## 3. Features

### API-01 Server lifecycle, bind address and port

**Priority:** Must · **Milestone:** M5 · **Decisions:** D5, D18 · **Legacy:** none (new; replaces the legacy hook TCP port, features.md §3) · **Depends on:** CFG-01

As a user, I want the local API to be off until I turn it on, and to listen only on this machine,
so that nothing is exposed that I did not ask for.

**Acceptance criteria**

- **API-01.1** Given a fresh install, when the app starts, then no port is opened by the app.
  *(Verify: security)*
- **API-01.2** Given the user enables the API, when the setting is saved, then the server listens on
  `127.0.0.1` at the configured port within 1 s, without a restart. The default port is defined
  in the v1 API document (API-09). *(Verify: integration)*
- **API-01.3** Given the settings file asks for a bind address other than `127.0.0.1` (for example
  `0.0.0.0` or a LAN address), when the app loads it, then the server does not start, the setting
  is reported as invalid, and nothing listens on another interface. *(Verify: security)*
- **API-01.4** Given the configured port is in use by another program, when the API is enabled, then
  the settings page and status line show "port N is in use", the app keeps running, and no other
  port is chosen silently. *(Verify: integration)*
- **API-01.5** Given the API is running with connected stream clients, when the user disables it,
  then the listener and all connections are closed within 1 s. *(Verify: integration)*
- **API-01.6** Given the API is enabled, when the app runs as the 32-bit and the 64-bit build, then
  API-01.1 to API-01.5 pass on both. *(Verify: CI)*

### API-02 Submit text: `POST /v1/text`

**Priority:** Must · **Milestone:** M5 · **Decisions:** D5, D6 · **Legacy:** none (new) · **Depends on:** IN-05

As a tool author, I want to post text with a source name, so that it is translated like clipboard
text and kept apart from other sources.

**Acceptance criteria**

- **API-02.1** Given the API is enabled, when a client posts `{"text": "こんにちは", "source": "ocr"}`
  as `application/json`, then the response is `202 Accepted` with the context id `api:ocr`, and the
  text is shown in the Original Text pane within the budget in NFR-03.1. *(Verify: integration)*
- **API-02.2** Given a request without `source`, when it is accepted, then the text goes to context
  `api:default`. *(Verify: integration)*
- **API-02.3** Given a request with an optional `lang` (source-language hint) and `id` (client message
  id), when it is accepted, then `lang` is used for this entry instead of the global source
  language, and `id` is echoed in the matching `text` stream event (API-04). An unknown language
  code gives `400 unknown_language`. *(Verify: contract)*
- **API-02.4** Given `text` is missing, empty or only whitespace, when it is posted, then the
  response is `400 empty_text` and nothing enters the pipeline. *(Verify: contract)*
- **API-02.5** Given a body that is not valid UTF-8 or not valid JSON, when it is posted, then the
  response is `400 invalid_json`; given a `source` that breaks the context-id rules (IN-05.3), the
  response is `400 invalid_source`. *(Verify: contract)*
- **API-02.6** Given automatic input is paused (IN-02.2), when text is posted, then the response is
  `409 input_paused` and nothing is stored. Unknown JSON fields are ignored. *(Verify: contract)*

### API-03 Status: `GET /v1/status`

**Priority:** Must · **Milestone:** M5 · **Decisions:** D5, D18 · **Legacy:** none (new) · **Depends on:** PIP-03

As a tool author, I want to ask whether the app is ready and which translators are active, so that
my tool can show a useful state.

**Acceptance criteria**

- **API-03.1** Given the API is enabled, when a client calls `GET /v1/status`, then it receives `200`
  with the API version (`1.x`), app version, whether input is paused, the source and target
  language, and per enabled translator its id, name, state and whether it is remote.
  *(Verify: contract)*
- **API-03.2** Given secrets exist for providers or the API, when status is returned, then no
  secret, token, endpoint credential or file path under the user profile appears in it.
  *(Verify: security)*
- **API-03.3** Given a provider is failing (for example key missing), when status is returned, then
  that provider's state names the problem with the same code the pane shows. *(Verify: contract)*
- **API-03.4** Given "require token for input" is on (API-05), when status is called without a
  valid token, then the response is `401 unauthorized`. *(Verify: security)*

### API-04 Output stream: WebSocket and SSE

**Priority:** Must · **Milestone:** M5 · **Decisions:** D5, D18 · **Legacy:** none (new) · **Depends on:** API-05, PIP-04

As an overlay author, I want a live stream of incoming text and translations, so that I can show
them over a game or in OBS.

**Acceptance criteria**

- **API-04.1** Given the output stream is enabled and the client has a valid token, when it opens
  the WebSocket at `/v1/stream` or the SSE stream at `/v1/events`, then it first receives a `hello`
  event with the API version, then live events. *(Verify: integration)*
- **API-04.2** Given an entry is flushed and translated by two providers, when the client is
  connected, then it receives one `text` event (entry id, context, original text, text after
  substitutions, client ids) before any `translation` event for that entry, and one final
  `translation` event per provider (provider id, status, text or error code, cached flag, elapsed
  time, remote flag). *(Verify: integration)*
- **API-04.3** Given a provider streams its result (for example an LLM translator), when partial text
  arrives, then the client receives `translation_delta` events in order, followed by the final
  `translation` event. *(Verify: integration)*
- **API-04.4** Given a client reads too slowly and more than 256 events are waiting for it, when new
  events arrive, then the oldest waiting events for that client are dropped, the client gets a
  `lagged` event with the number dropped, and the pipeline and other clients are not slowed down.
  *(Verify: integration)*
- **API-04.5** Given the stream setting is off (the default), when a client opens `/v1/stream` or
  `/v1/events`, then the response is `404 not_found`. *(Verify: security)*
- **API-04.6** Given a client disconnects without closing, when the app notices (WebSocket ping
  every 30 s, or a failed write), then its resources are released. *(Verify: integration)*

### API-05 Token authentication

**Priority:** Must · **Milestone:** M5 · **Decisions:** D18 · **Legacy:** none (new) · **Depends on:** CFG-05

As a user, I want a token to protect the stream of my translations, and optionally the input, so
that other local programs cannot read or spend what I don't allow. *(assumes O12 recommendation)*

**Acceptance criteria**

- **API-05.1** Given the user enables the output stream and no token exists, when the setting is
  saved, then the app creates a token with at least 128 bits of randomness, stores it in the OS
  credential store (CFG-05) and shows a warning that the stream needs it. *(assumes O12
  recommendation)* *(Verify: integration)*
- **API-05.2** Given the stream is enabled, when a client connects without a token or with a wrong
  one, then the connection is refused with `401 unauthorized`; this holds even when input is
  tokenless. *(assumes O12 recommendation)* *(Verify: security)*
- **API-05.3** Given "require token for input" is on (off by default, D18), when `POST /v1/text` comes
  without a valid token, then the response is `401 unauthorized` and nothing is stored.
  *(Verify: security)*
- **API-05.4** Given a token, when a client sends it, then it is accepted as `Authorization: Bearer
  <token>`, and for WebSocket and SSE also as the `token` query parameter (browsers cannot set
  headers there); the comparison takes constant time. *(Verify: security)*
- **API-05.5** Given any request, when the app logs it, then the token and the `token` query value
  never appear in logs, status, history exports or crash reports (NFR-05.1). *(Verify: security)*
- **API-05.6** Given the user regenerates the token, when it is saved, then the old token is
  rejected at once and connected stream clients using it are closed. *(Verify: integration)*

### API-06 `Host`, `Origin` and content-type checks

**Priority:** Must · **Milestone:** M5 · **Decisions:** D18 · **Legacy:** none (new); risks-and-open-questions.md §4 · **Depends on:** API-01

As a user, I want web pages in my browser to be unable to post text or read my translations, so
that a malicious site cannot use the loopback API.

**Acceptance criteria**

- **API-06.1** Given a request whose `Host` header is not `127.0.0.1:<port>` or `localhost:<port>`
  (for example a DNS-rebinding name), when it arrives, then the response is `403 forbidden_host`
  and the request is not processed. *(Verify: security)*
- **API-06.2** Given a request with an `Origin` header that is not on the user's allow-list (empty by
  default), when it arrives, including a WebSocket upgrade or an `OPTIONS` preflight, then the
  response is `403 forbidden_origin`. Requests without `Origin` (non-browser tools) are allowed.
  *(Verify: security)*
- **API-06.3** Given a `POST /v1/text` with a content type other than `application/json` (for example
  `text/plain` or a form, which a browser can send without preflight), when it arrives, then the
  response is `415 unsupported_media_type`. *(Verify: security)*
- **API-06.4** Given default settings, when any response is sent, then it carries no CORS headers;
  given an allow-listed origin, only that exact origin is echoed. *(Verify: security)*
- **API-06.5** Given a test web page served from another origin, when it tries a simple POST, a
  `fetch` with JSON and a WebSocket to the API, then all three fail and no text enters the app.
  *(Verify: security)*

### API-07 Size limits, rate limits and flood behaviour

**Priority:** Must · **Milestone:** M5 · **Decisions:** D18 · **Legacy:** none (new); risks-and-open-questions.md §2, §4 · **Depends on:** IN-05, PIP-07, PIP-09, PIP-10

As a user, I want a buggy or hostile local program to be unable to freeze the app or burn my
budget, so that the API is safe to leave on.

**Acceptance criteria**

- **API-07.1** Given a request body larger than the body limit (NFR-04.5), when it arrives, then
  the response is `413 payload_too_large` and the body is not read further; given `text` longer
  than the maximum input length (NFR-04.4, IN-05.6), the response is `413 text_too_long`.
  *(Verify: security)*
- **API-07.2** Given default settings, when clients send more requests than the rate limit allows
  (NFR-04.6), then extra requests get `429 rate_limited` with a `Retry-After` header.
  *(Verify: security)*
- **API-07.3** Given a client sends 10,000 requests in 10 s, when the flood runs, then the UI stays
  within NFR-03.3, memory stays below the NFR-03.5 budget plus 50 MB, and the app responds normally
  within 2 s after the flood stops. *(Verify: integration)*
- **API-07.4** Given a flood of distinct texts, when they reach the pipeline, then remote providers
  receive no more requests than their rate limits (PIP-09.1), budgets (PIP-06, when set) and the
  auto-translate cap (PIP-07) allow, and queued requests are replaced, not piled up (PIP-10.1).
  *(Verify: integration)*
- **API-07.5** Given 8 stream clients are connected, when a ninth connects, then it gets
  `503 too_many_connections`. *(Verify: integration)*
- **API-07.6** Given a client opens a connection and sends nothing, or sends headers slowly, when
  10 s pass, then the connection is closed. *(Verify: security)*

### API-08 Error responses

**Priority:** Must · **Milestone:** M5 · **Decisions:** D18, D20 · **Legacy:** none (new) · **Depends on:** API-09

As a tool author, I want every error to have a stable code and a short message, so that my tool can
react correctly.

**Acceptance criteria**

- **API-08.1** Given any error, when the API answers, then the body is JSON
  `{"error": {"code": "<code>", "message": "<text>"}}` with a matching HTTP status, and the code is
  one of those listed in the API document. *(Verify: contract)*
- **API-08.2** Given an unknown path, when it is requested, then the response is `404 not_found`;
  given a known path with the wrong method, `405 method_not_allowed` with an `Allow` header.
  *(Verify: contract)*
- **API-08.3** Given a request to an unsupported version prefix (for example `/v2/text`), when it
  arrives, then the response is `404 unsupported_version`. *(Verify: contract)*
- **API-08.4** Given an internal failure, when the API answers, then the status is `500 internal`,
  the message contains no stack trace, path or secret, and the details go only to the app log.
  *(Verify: security)*

### API-09 Versioned API document and schema

**Priority:** Must · **Milestone:** M5 · **Decisions:** D5, D18 · **Legacy:** none (new) · **Depends on:** —

As a tool author, I want a versioned document with a schema, so that I can build against the API
and know what may change.

**Acceptance criteria**

- **API-09.1** Given the M5 release, when a reader opens `docs/api/v1.md`, then it describes every
  endpoint, request and response field, event type, error code, limit, the default port and the
  security rules, with a `curl` example per endpoint using `127.0.0.1`. *(Verify: manual)*
- **API-09.2** Given the JSON Schema files for v1, when the contract test suite runs, then every
  response and every stream event the app produces validates against them. *(Verify: contract)*
- **API-09.3** Given a later 1.x release, when the API changes, then it only adds optional fields,
  event types or endpoints; removing or renaming anything needs `/v2`, and `/v1` stays served for
  at least one release after `/v2` appears. *(Verify: manual)*
- **API-09.4** Given a client that ignores unknown fields, when it talks to a newer 1.x app, then it
  keeps working; the document says clients must ignore unknown fields and event types.
  *(Verify: contract)*

### API-10 API settings and visibility

**Priority:** Should · **Milestone:** M5 · **Decisions:** D11, D18 · **Legacy:** none (new) · **Depends on:** UI-10, UI-13

As a user, I want to see whether the API is on and who uses it, so that I know what can reach my
translations.

**Acceptance criteria**

- **API-10.1** Given the settings page for the local API, when the user opens it, then it shows on/off,
  port, stream on/off, the token, "require token for input", the origin allow-list, and a
  ready-to-copy `curl` example. The token is shown masked, with only its last 4 characters visible
  (for example `••••••••3f9a`), and has a "Copy" button and "Regenerate" (API-05.6). Copying puts
  the full token on the clipboard; the copied `curl` example includes it. *(Verify: manual)*
- **API-10.2** Given the stream is enabled, when the settings page is shown, then it warns that every
  translation can be read by programs that have the token. *(assumes O12 recommendation)*
  *(Verify: manual)*
- **API-10.3** Given the API is on, when the status line is shown, then it shows "API on", the
  number of connected stream clients, and the time of the last accepted request.
  *(Verify: manual)*
- **API-10.4** Given requests were rejected (401, 403, 429), when the user opens the API page, then
  the counts per reason since start are shown, without request contents. *(Verify: manual)*

## 4. Out of scope

- Access from other machines (LAN, internet), binding to other interfaces, and TLS: D18 binds
  `127.0.0.1` only. IPv6 `::1` is not bound either.
- CORS by default, cookies or browser sessions: not offered (D18, risks §4).
- Multiple user accounts or per-client tokens: one token is enough for v1.
- A synchronous "translate and return" endpoint: results arrive through the stream (D5 proposal).
- The legacy hook TCP protocol and its port setting: replaced; the hook plugin uses this API (D6).

## 5. Open questions

- **O12** (token for the output stream): this spec uses the recommendation. When the stream is
  enabled it always needs the token, even if input is tokenless, and the settings page warns about
  it (API-04, API-05, API-10).

Note for the owner: D18 says "bound to `127.0.0.1` only". Some clients resolve `localhost` to `::1`
first; the API document tells authors to use `127.0.0.1`.

## 6. Traceability

| Feature | Priority | Milestone | Decisions |
|---|---|---|---|
| API-01 Server lifecycle, bind address and port | Must | M5 | D5, D18 |
| API-02 Submit text | Must | M5 | D5, D6 |
| API-03 Status | Must | M5 | D5, D18 |
| API-04 Output stream | Must | M5 | D5, D18 |
| API-05 Token authentication | Must | M5 | D18 (O12) |
| API-06 Host, Origin and content-type checks | Must | M5 | D18 |
| API-07 Size limits, rate limits and flood behaviour | Must | M5 | D18 |
| API-08 Error responses | Must | M5 | D18, D20 |
| API-09 Versioned API document and schema | Must | M5 | D5, D18 |
| API-10 API settings and visibility | Should | M5 | D11, D18 |
