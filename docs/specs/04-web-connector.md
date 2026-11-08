# 04 Web connector (`WEB`)

## 1. Purpose and scope

The web connector turns a web translation service into a translator pane without new code. A
service is a TOML **provider definition** (data, not code, D10). The connector loads and checks
definitions, builds requests, runs an optional session step, extracts the translation from the
reply, and reports clear errors. It ships the official-API presets (D17) and the keyless Google and
Bing example definitions (D21).

Covered: definition format and validation, placeholders, request bodies, response extraction,
language mapping, session step, secrets, rate limits and length limits, the **Test** view, the
presets and examples, managing definitions in the settings UI, and provider health.

Not covered: the router, cache and history (see the pipeline spec, `PIP`), pane layout and the
settings window itself (see the UI spec, `UI`), credential storage internals and settings files
(see the settings spec, `CFG`), and translators that need real logic such as request signing or
OAuth (those are translator plugins, see the plugin spec, `PLG`).

The definition format follows `../guides/custom-web-services.md` (owner draft). Where this spec
needs something the draft does not have yet, the criterion says so and the guide is updated when
the format is frozen.

## 2. Goals

1. A user can add any simple web translation service by writing or importing one TOML file, with no
   rebuild and no restart.
2. Broken or wrong definitions fail with a message that names the file, the field and the reason.
3. Secrets never appear in definition files, logs, the Test view or error messages.
4. The four official presets (DeepL API, Google Cloud Translation, Microsoft Translator,
   LibreTranslate) work with only a key (and, where needed, a server address or region).
5. The keyless Google and Bing examples work through the same connector features and stay clearly
   marked as unofficial and off by default.
6. When a service breaks, the pane and the Test view show enough to repair the definition.

## 3. Features

### WEB-01 Provider definition files and validation

**Priority:** Must · **Milestone:** M1 · **Decisions:** D10, D20 · **Legacy:** features.md §1 (web panes; replaced) · **Depends on:** CFG-01

As a user, I want translation services to be described in small files that the app checks for me,
so that I can add or fix a service without waiting for a new app version.

**Acceptance criteria**

- **WEB-01.1** Given a valid definition file in the user providers folder, when the app starts or
  the user clicks **Reload providers**, then the provider appears in the translator list with the
  `name` from the file and gets its own pane slot. *(Verify: integration)*
- **WEB-01.2** Given a file with a TOML syntax error, when it is loaded, then that provider is not
  loaded, the settings UI shows "<file>: line L, column C: <reason>", and all other definitions
  still load. *(Verify: unit)*
- **WEB-01.3** Given a definition that misses a required field (`provider.id`, `provider.name`,
  `request.method`, `request.url`, `response.text`), or that uses both `body.form` and `body.json`,
  or that has an invalid regex or JSONPath, when it is loaded, then it is rejected with a message
  that names the field path (for example `response.text.jsonpath`) and the reason. *(Verify: unit)*
- **WEB-01.4** Given two definitions with the same `id`, when they are loaded, then the second one
  is rejected and the message names both files. *(Verify: unit)*
- **WEB-01.5** Given a definition with a field the app does not know, when it is loaded, then the
  definition still loads and the settings UI shows a warning naming the unknown field (so a typo
  such as `max_char` is visible). *(Verify: unit)*
- **WEB-01.6** Given a definition without optional fields, when it is loaded, then the defaults from
  the guide apply: `enabled = false`, `unofficial = false`, `auto_translate = false`,
  `max_chars = 1000`, no rate limit, `timeout_seconds` = the default in NFR-04.2, and
  `html_decode = false`. *(Verify: unit)*

### WEB-02 Placeholders, language mapping and request building

**Priority:** Must · **Milestone:** M1 · **Decisions:** D10, D21 · **Legacy:** features.md §5 "Source/target language menus" · **Depends on:** WEB-01

As a user, I want to describe where the text and languages go in a request, so that the app can talk
to services that expect JSON, form fields or URL parameters.

**Acceptance criteria**

- **WEB-02.1** Given a definition that uses `{{text}}`, `{{src}}`, `{{dst}}`, `{{counter}}`,
  `{{session.name}}` or `{{secret.name}}`, when a request is built, then each placeholder is
  replaced with its value and escaped for where it sits: URL-encoded in `url` and query parameters,
  form-encoded in `body.form`, and inserted as a JSON string value in `body.json`. *(Verify: unit)*
- **WEB-02.2** Given input text containing `&`, `=`, `"`, `\`, a newline, `{{text}}`, emoji and
  Japanese characters, when the request is built for each body kind, then the service receives the
  exact text, and placeholder syntax inside the text is not expanded again. *(Verify: unit)*
- **WEB-02.3** Given `body.json` with numbers, booleans, arrays and nested objects (for example a
  `text` field that is an array of one string), when the body is built, then non-placeholder values
  keep their JSON types and a placeholder inside an array becomes a string element. *(Verify: unit)*
- **WEB-02.4** Given a `query` table in `[request]`, when a request
  is built, then each entry is appended to the URL as an encoded query parameter, for both GET and
  POST. *(Verify: unit)*
- **WEB-02.5** Given `[languages]` with `map = { "zh-Hans" = "zh-CN" }`, when the target is
  Simplified Chinese, then `{{dst}}` is `zh-CN`; codes not in the map are sent unchanged. Given
  `only` is set and the chosen language is not in it, then the pane is greyed out with "Language not
  supported by <name>" and no request is sent. *(Verify: unit)*
- **WEB-02.6** Given the source language is "auto", when `[languages] auto` is set to a code, then
  `{{src}}` is that code. When `auto = ""`, then a query, form or JSON field whose whole value is
  `{{src}}` is left out of the request (needed by DeepL and Microsoft). When `auto` is missing,
  then the pane shows "<name> needs a source language" and no request is sent. *(Verify: unit)*
- **WEB-02.7** Given a definition that declares a user value in `[params]` (for example a server
  address or a region) and uses it as `{{param.name}}`, when a request is built, then the value
  from the user settings is inserted with the same escaping as WEB-02.1; the shipped definition
  file never holds the user's value. When the value is not set, the provider state is "Needs
  setup" and no request is sent. *(Verify: unit)*

### WEB-03 Response extraction and error extraction

**Priority:** Must · **Milestone:** M1 · **Decisions:** D10, D21 · **Legacy:** — · **Depends on:** WEB-02

As a user, I want the app to find the translation in a service's reply, and to show the service's
own error message when there is one, so that I see either a clean translation or a useful error.

**Acceptance criteria**

- **WEB-03.1** Given `text = { jsonpath = "$.translatedText" }` and a reply
  `{"translatedText":"Hello"}`, when the reply is processed, then the pane shows `Hello`.
  *(Verify: contract)*
- **WEB-03.2** Given `text = { jsonpath = "$[0][*][0]", join = "" }` and a reply with three
  sentence entries, when the reply is processed, then the three strings are joined in reply order
  with the join string. Without `join`, only the first match is used and the Test view notes that
  more matches were found. `null` matches are skipped; number matches are shown as text.
  *(Verify: unit)*
- **WEB-03.3** Given `text = { regex = '...' }` and an HTML or text reply, when the reply is
  processed, then the first capture group of the first match is the translation; with `join`, the
  first capture group of every match is joined in order. Given `html_decode = true`, then entities
  such as `&amp;`, `&#39;` and `&quot;` are decoded. *(Verify: unit)*
- **WEB-03.4** Given a reply, when it is processed, then the steps run in this order: (1) session
  refresh triggers (WEB-08), (2) `error` extraction, (3) `text` extraction. If `error` matches a
  non-empty value, the pane shows "<name>: <error text>" and no translation, even when the HTTP
  status is 200. *(Verify: unit)*
- **WEB-03.5** Given a reply that is not valid JSON while a `jsonpath` is configured (for example an
  HTML login or captcha page with status 200), when it is processed, then the pane shows "Reply was
  not JSON" and the Test view shows the raw reply. Given valid JSON where the path matches nothing,
  then the pane shows "Translation not found in reply (path `<path>` matched nothing)".
  *(Verify: contract)*
- **WEB-03.6** Given a non-2xx HTTP status and no `error` match, when the reply is processed, then
  the pane shows the status code and its meaning (for example "HTTP 403 Forbidden"); the reply body
  is shown only in the Test view. The reply is decoded with the charset from the reply headers,
  defaulting to UTF-8, and invalid bytes are replaced, never a crash. *(Verify: contract)*

### WEB-04 Secrets in definitions

**Priority:** Must · **Milestone:** M1 · **Decisions:** D10, D17 · **Legacy:** — · **Depends on:** CFG-05

As a user, I want API keys to stay in the Windows credential store, so that I can share or publish
a definition file without leaking my key.

**Acceptance criteria**

- **WEB-04.1** Given `{{secret.my-key}}` in a definition and a secret `my-key` in the credential
  store, when a request is sent, then the value is read at send time and inserted; the definition
  file on disk never contains the value. *(Verify: integration)*
- **WEB-04.2** Given `{{secret.my-key}}` and no such secret, when the definition loads, then it
  loads, the provider state is "Needs key", the pane offers **Add key**, and no request is sent.
  *(Verify: unit)*
- **WEB-04.3** Given a request that contains a secret in a header, the URL, a query parameter or the
  body, when it is logged, shown in the Test view, shown in an error message or written to history,
  then every occurrence of the value is replaced by `••••` plus the secret's name, including inside
  error text that the service echoes back. *(Verify: security)*
- **WEB-04.4** Given a definition whose `url` uses plain `http://` to a host that is not loopback and
  whose request contains a secret, when the user enables it, then the app warns "This service would
  receive your key without encryption" and requires confirmation. *(Verify: unit)*
- **WEB-04.5** Given a header named like `Authorization`, `*key*` or `*token*`, or a body field named
  `api_key`, `key` or `auth_key`, with a literal value instead of a placeholder, when the definition
  loads, then the settings UI warns "This looks like a key stored in the file; move it to Secrets".
  *(Verify: unit)*
- **WEB-04.6** Given a reply that redirects to a different host, when the redirect is followed, then
  headers that contain secrets are not sent to the new host. Secrets also follow NFR-05.1 and
  NFR-05.2.
  *(Verify: security)*

### WEB-05 Rate limits, length limits, timeouts and retries

**Priority:** Must · **Milestone:** M1 · **Decisions:** D10, D21 · **Legacy:** features.md §3 "500-character cap for automatic web requests" · **Depends on:** PIP-04, PIP-07, PIP-09

As a user, I want the app to respect each service's limits, so that I am not blocked and long text
is handled sensibly.

**Acceptance criteria**

- **WEB-05.1** Given `rate_limit = { requests = 10, per_seconds = 60 }`, when 15 requests are due
  within one minute (for example the chunks of a split text, WEB-05.2), then at most 10 are sent in
  any 60-second window; the rest wait, and the pane shows "Waiting Ns (rate limit)". Waiting
  entries are replaced by newer ones as in PIP-10.1. Session-step fetches and split chunks count as
  requests. *(Verify: unit)*
- **WEB-05.2** Given a text longer than `max_chars` (counted in Unicode scalar values), when it is
  sent, then it is split at sentence ends (`。！？.!?` and line breaks) into chunks of at most
  `max_chars`, sent in order, and the results are joined in order. *(Verify: unit)*
- **WEB-05.3** Given a single sentence longer than `max_chars`, when it is sent, then nothing is
  sent and the pane shows "Text too long for <name> (N characters, limit M)". Text is never cut in
  the middle of a sentence. *(Verify: unit)*
- **WEB-05.4** Given automatic input longer than the auto-translate cap (NFR-04.1), when it arrives,
  then the router does not send it to this provider automatically; a manual Translate still works
  under WEB-05.2/WEB-05.3. `max_chars` and the auto cap are separate limits. *(Verify: integration)*
- **WEB-05.5** Given a service that does not answer, when `timeout_seconds` passes (default
  NFR-04.2), then the request is cancelled and the pane shows "Timed out after N s". Retries follow
  NFR-04.3: only network errors, timeouts, HTTP 5xx and 429 are retried; 4xx other than 429 are not. A status listed in
  `refresh_on_status` is handled by WEB-08.5 instead. *(Verify: contract)*
- **WEB-05.6** Given an HTTP 429 reply with a `Retry-After` header, when it is handled, then the
  provider sends nothing until that time has passed and the pane shows "Rate limited until HH:MM:SS".
  *(Verify: contract)*

### WEB-06 LibreTranslate preset

**Priority:** Must · **Milestone:** M1 · **Decisions:** D17 · **Legacy:** features.md §1 "New in the rewrite" · **Depends on:** WEB-01 … WEB-05

As a user, I want a ready-made LibreTranslate translator, so that I get translations from a
self-hosted or hosted server by only entering its address (and a key if the server needs one).

**Acceptance criteria**

- **WEB-06.1** Given the app is installed, when the user opens **Settings > Translators**, then
  "LibreTranslate" is listed as a built-in, official preset, disabled until a server address is set
  (a `[params]` value, WEB-02.7). In M1, before the settings UI exists, the address is set in the
  settings file; the settings page is checked in M2. *(Verify: manual)*
- **WEB-06.2** Given a server address and no key, when the user translates "こんにちは" ja→en
  against a recorded LibreTranslate reply, then the pane shows the translated text; given a key,
  then the key is sent from the credential store. *(Verify: contract)*
- **WEB-06.3** Given the server replies with an error object (for example an invalid key or an
  unsupported language), when it is handled, then the pane shows the server's error text.
  *(Verify: contract)*
- **WEB-06.4** Given a LibreTranslate server running on `localhost`, when it is used, then the pane
  is not marked "text leaves this machine"; any other host is marked (NFR-05.3). *(Verify: unit)*
- **WEB-06.5** Given an opt-in live test with a reachable server, when it runs on both the 32-bit
  and the 64-bit build, then copying Japanese text produces a LibreTranslate translation (M1 exit).
  *(Verify: manual)*

### WEB-07 DeepL API, Google Cloud Translation and Microsoft Translator presets

**Priority:** Must · **Milestone:** M2 · **Decisions:** D17, D20 · **Legacy:** features.md §1 "Google", "Bing", "DeepL" (replaced) · **Depends on:** WEB-01 … WEB-05

As a user, I want built-in presets for the major official translation APIs, so that I can use
Google, Bing (Microsoft) and DeepL reliably with my own key.

**Acceptance criteria**

- **WEB-07.1** Given each preset, when it is written or updated, then it follows the provider's
  current official documentation; the preset records the date it was checked and links the
  provider's terms and key console from the pane's menu. *(Verify: manual)*
- **WEB-07.2** Given a preset that needs a user value other than a key (DeepL: Free or Pro plan;
  Microsoft: resource region; LibreTranslate: server address), when the user opens the preset, then
  the settings page asks for that value (declared in `[params]`, WEB-02.7); it is stored in user
  settings, not in the shipped file. *(Verify: manual)*
- **WEB-07.3** Given a recorded successful reply for each preset, when ja→en text with several
  sentences is translated, then the pane shows the full translation with all sentences, and text
  with `&`, `'` and `"` comes back without HTML entities. *(Verify: contract)*
- **WEB-07.4** Given recorded replies for "invalid key", "quota exceeded" and "too many requests"
  for each preset, when they are handled, then the pane shows "Key rejected", "Quota used up" and
  "Rate limited" with the provider's message. *(Verify: contract)*
- **WEB-07.5** Given the Google Cloud preset sends its key as a URL parameter, when the request is
  shown in the Test view or logged, then the key is masked (WEB-04.3). *(Verify: security)*
- **WEB-07.6** Given legacy users who relied on the DeepL web pane, when they use the DeepL API
  preset, then the legacy extra-space quirk is not reproduced, and this change is listed in the
  behaviour-changes list (D20). *(Verify: parity)*

### WEB-08 Session step

**Priority:** Must · **Milestone:** M2 · **Decisions:** D21 · **Legacy:** features.md §1 "Bing" (token scraping, refresh after 100 uses) · **Depends on:** WEB-02, WEB-03

As a user, I want a definition to load a web page first and reuse values and cookies from it, so
that services whose website hands out tokens can be used.

**Acceptance criteria**

- **WEB-08.1** Given a `[session]` with a page `request` and `[session.extract]` regexes, when the
  first translation is requested, then the page is fetched once, the first capture group of each
  regex becomes `{{session.<name>}}`, and the translation request uses those values.
  *(Verify: contract)*
- **WEB-08.2** Given `keep_cookies = true`, when later requests are sent, then cookies set by the
  session page (and updated by later replies) are sent with them. Session values and cookies are
  kept in memory per provider only; they are never written to disk or logs. *(Verify: contract)*
- **WEB-08.3** Given an extract regex that no longer matches the page, when the session step runs,
  then no translation request is sent and the pane shows "Session value '<name>' not found: the web
  page changed (regex did not match)". *(Verify: contract)*
- **WEB-08.4** Given `refresh_after_uses = 100` or `refresh_after_seconds = 3000`, when the 101st
  request is due or 3000 s have passed since the session step, then the session step runs again
  before that request and `{{counter}}` restarts at 1. *(Verify: unit)*
- **WEB-08.5** Given `refresh_on_status = [400, 401, 403, 429]` or a reply that matches
  `refresh_session_if` (for example Bing's `ShowCaptcha`), when such a reply arrives, then the
  session step runs again and the request is retried exactly once. If the retry fails the same way,
  the pane shows "Service rejected the request after a session refresh (captcha or blocked)", the
  provider health becomes "Blocked", and automatic requests to it pause until the user clicks
  **Retry**. *(Verify: contract)*
- **WEB-08.6** Given several requests waiting while a session refresh runs, when the refresh
  finishes, then the page was fetched only once and all waiting requests use the new values.
  *(Verify: unit)*

### WEB-09 Keyless Google and Bing examples

**Priority:** Must · **Milestone:** M2 · **Decisions:** D21, D13 · **Legacy:** features.md §1 "Google", "Bing" · **Depends on:** WEB-08, WEB-11

As a user, I want ready-made but clearly unofficial definitions for the Google and Bing web
versions, so that I can try them without a key if I accept the risk.

**Acceptance criteria**

- **WEB-09.1** Given a fresh install, when the user opens **Settings > Translators**, then Google
  (web, unofficial) and Bing (web, unofficial) are not active; they are offered only under
  **Add > Custom web service > From example**. *(Verify: manual)*
- **WEB-09.2** Given the user adds an example, when it is copied into the providers folder, then it
  stays disabled until the user reads the warning ("unofficial endpoint, can break at any time, may
  break the service's terms, use at your own risk; the official preset is recommended") and clicks
  **Enable**. *(Verify: manual)*
- **WEB-09.3** Given a definition with `unofficial = true`, when its pane is shown, then the pane
  header and the settings list show an *unofficial* badge, and auto translate stays off unless the
  user turns it on for that provider. *(Verify: manual)*
- **WEB-09.4** Given the shipped `examples/providers/google-web-unofficial.toml` and
  `bing-web-unofficial.toml`, when they are loaded by the validator, then they load without errors
  or warnings, and recorded replies (sentence-split Google array; Bing session page, translation
  reply, error and `ShowCaptcha` replies) give the expected translation or error. *(Verify: contract)*
- **WEB-09.5** Given an opt-in live test, when it runs, then the examples translate ja→en; a failure
  is reported as "example definition needs an update", not as an app failure, and does not block a
  release. *(Verify: manual)*

### WEB-10 Test view

**Priority:** Must · **Milestone:** M2 · **Decisions:** D10, D21 · **Legacy:** — · **Depends on:** WEB-03, WEB-04, WEB-08

As a user, I want to see exactly what the app sent and received, so that I can write a new
definition or repair a broken one.

**Acceptance criteria**

- **WEB-10.1** Given a definition (saved or still being edited), when the user clicks **Test** with
  a sample text (default "こんにちは。元気ですか？", editable) and a language pair, then the app
  sends one real request and shows: method, final URL, headers, body, HTTP status, reply headers,
  the raw reply, and the extracted translation or the error. *(Verify: manual)*
- **WEB-10.2** Given a definition with a session step, when Test runs, then the view also shows the
  session request, its status, and each extracted value or "not matched". *(Verify: manual)*
- **WEB-10.3** Given any secret in the request, when the Test view shows it, then the value is
  masked (WEB-04.3); a **Copy** of the request also contains only masked values. *(Verify: security)*
- **WEB-10.4** Given a raw reply larger than the view can show, when it is displayed, then it is
  shown pretty-printed if it is JSON, can be scrolled and searched, and can be copied whole.
  *(Verify: manual)*
- **WEB-10.5** Given a Test run, when it finishes, then the result is not added to history or the
  translation cache, and Test runs count against the provider's rate limit. *(Verify: integration)*

### WEB-11 Add, import, edit and reload definitions

**Priority:** Must · **Milestone:** M2 · **Decisions:** D10, D21 · **Legacy:** — · **Depends on:** WEB-01, UI-13

As a user, I want to manage translation services in the settings window, so that I don't have to
edit files by hand or restart the app.

**Acceptance criteria**

- **WEB-11.1** Given **Settings > Translators**, when the user chooses **Add > Custom web service**,
  then they can start from a blank template, from an example, or from **Import file…**; the new
  definition is saved as `<id>.toml` in the user providers folder. *(Verify: manual)*
- **WEB-11.2** Given an imported file that fails validation, when the import runs, then the file is
  not copied, and the errors are shown with line numbers. Given an imported `id` that already
  exists, then the user chooses **Replace** or **Keep both** (the copy gets a new id).
  *(Verify: integration)*
- **WEB-11.3** Given a user definition, when the user edits it in the built-in editor and saves,
  then it is validated first; on errors the file is not overwritten and each error is shown next to
  its line; on success the provider is reloaded at once, without a restart. *(Verify: integration)*
- **WEB-11.4** Given a built-in preset, when the user tries to edit it, then it is read-only and
  **Duplicate to edit** creates a user copy; app updates change presets but never user copies.
  *(Verify: manual)*
- **WEB-11.5** Given definition files changed on disk, when the user clicks **Reload providers**,
  then changed providers are reloaded, new ones are added, and removed ones disappear from the list
  while their layout slot is kept for when the file comes back. Requests already in flight finish
  with the old definition; the session state of a changed provider is reset. *(Verify: integration)*
- **WEB-11.6** Given **Export**, when the user exports a definition, then the file contains secret
  names only, never secret values. *(Verify: security)*

### WEB-12 Error states and provider health

**Priority:** Must · **Milestone:** M2 · **Decisions:** D10, D11 · **Legacy:** features.md §1 "Busy animation" · **Depends on:** WEB-03 … WEB-08, UI-09, UI-10

As a user, I want each service's state shown at a glance, so that I know whether a missing
translation is my mistake, the service's, or the network's.

**Acceptance criteria**

- **WEB-12.1** Given any failed request, when the pane shows the error, then it is one of these
  states with a short text and a suggested action: Needs key, Needs setup, Key rejected, Quota used up, Rate
  limited, Blocked (captcha), Session failed, Translation not found, Timed out, Offline, Definition
  error, Language not supported. *(Verify: unit)*
- **WEB-12.2** Given a provider, when its last request succeeded, then the health shows "OK" with
  the request time; after an error it shows the error state; the state is visible in the pane header
  and in the settings list. *(Verify: manual)*
- **WEB-12.3** Given no user action, when the app is idle, then it sends no health-check requests
  to services; health comes only from real requests and Test runs (no wasted quota).
  *(Verify: integration)*
- **WEB-12.4** Given the network is down (no route to host or DNS failure), when a request fails,
  then the state is "Offline" and the next automatic request is tried normally once the network is
  back, without a restart. *(Verify: contract)*
- **WEB-12.5** Given a definition whose endpoint was moved (HTTP 404, or 301 to an unexpected
  page), when a request fails, then the pane suggests "The service address may have changed: open
  Test" with a link to the guide section "When a service stops working". *(Verify: contract)*

## 4. Out of scope

- Hard-coded scraper panes for Excite, Yandex, SysTran, SDL, Infoseek, Babylon, Babel Fish,
  Baidu, WWWJDIC: no presets (D10, D17). Users may write definitions; services that need signed
  requests (Baidu) need a translator plugin.
- LEC Online, Honyaku, FreeTranslation: dead services (features.md §1).
- Request signing, computed tokens, OAuth service accounts (for example Google Cloud v3 with a
  service account): translator plugins, not definitions (product direction, "Translators").
- Scripting inside definitions: the format stays declarative; no template language (D10).
- Shipping any API key: every user brings their own (risks §5).
- Persisting session cookies across app restarts: not planned.

## 5. Open questions

- **O10** (default auto-translate for remote providers): this spec assumes the recommendation, so
  every web provider, preset or custom, starts with auto translate off (WEB-01.6, WEB-09.3).
- The `[request] query` table (WEB-02.4), `auto = ""` (WEB-02.6) and `[params]` (WEB-02.7) are now
  in the guide's draft format; field names are final only when the format is frozen.

## 6. Traceability

| Feature | Priority | Milestone | Decisions |
|---|---|---|---|
| WEB-01 Provider definition files and validation | Must | M1 | D10, D20 |
| WEB-02 Placeholders, language mapping and request building | Must | M1 | D10, D21 |
| WEB-03 Response extraction and error extraction | Must | M1 | D10, D21 |
| WEB-04 Secrets in definitions | Must | M1 | D10, D17 |
| WEB-05 Rate limits, length limits, timeouts and retries | Must | M1 | D10, D21 |
| WEB-06 LibreTranslate preset | Must | M1 | D17 |
| WEB-07 DeepL API, Google Cloud Translation and Microsoft Translator presets | Must | M2 | D17, D20 |
| WEB-08 Session step | Must | M2 | D21 |
| WEB-09 Keyless Google and Bing examples | Must | M2 | D21, D13 |
| WEB-10 Test view | Must | M2 | D10, D21 |
| WEB-11 Add, import, edit and reload definitions | Must | M2 | D10, D21 |
| WEB-12 Error states and provider health | Must | M2 | D10, D11 |
