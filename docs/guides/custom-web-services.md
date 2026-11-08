# Custom web services

The web connector lets you add any web-based translation service without a new app version.
A service is a small TOML file that tells the app where to send the text and where to find the
translation in the reply.

> **Status: draft.** This describes the planned format of `ta-provider-web` (see
> `docs/product-direction.md`, D10 and D21). Field names may change until the format is frozen.
> The app does not exist yet, so the settings steps describe the planned UI.

## Contents

1. [Before you start](#before-you-start)
2. [Set up the Google and Bing web versions](#set-up-the-google-and-bing-web-versions)
3. [Add your own service](#add-your-own-service)
4. [Format reference](#format-reference)
5. [When a service stops working](#when-a-service-stops-working)

## Before you start

There are two kinds of web services:

| Kind | Examples | Needs a key | Reliability |
|---|---|---|---|
| **Official API** (built-in presets) | DeepL API, Google Cloud Translation, Microsoft Translator (Bing), LibreTranslate | yes (LibreTranslate: only on some servers) | stable, documented |
| **Unofficial web endpoint** (custom service) | Google web, Bing web | no | can break at any time |

The unofficial endpoints are what the services' own websites use. They are not meant for other
programs: they can change without notice, they limit how fast you can send requests, and automated
use may be against the service's terms. The app marks them **unofficial**, keeps them off until you
turn them on, and doesn't send clipboard text to them automatically unless you allow it.

If you translate a lot, the official APIs are the better choice. Google Cloud Translation and
Microsoft Translator both have a free monthly allowance.

## Set up the Google and Bing web versions

The app ships two ready-made definitions:

- `examples/providers/google-web-unofficial.toml`
- `examples/providers/bing-web-unofficial.toml`

1. Open **Settings > Translators**.
2. Click **Add > Custom web service > From example**, then choose **Google (web, unofficial)** or
   **Bing (web, unofficial)**. The app copies the file into your providers folder
   (`%APPDATA%\Translation Aggregator\providers\`).
3. Read the warning and click **Enable**.
4. Click **Test**. The app translates a short sample and shows the request it sent, the raw reply,
   and the extracted translation. For Bing it also shows the values it got from the session step.
5. Optional: turn on **Auto translate** for this service if you want clipboard text sent to it
   automatically. Keep the rate limit in place, or the service may block you for a while.
6. The service now has its own pane. Drag it where you want it in the layout.

To install a definition by hand, copy the `.toml` file into the providers folder and restart the app,
or click **Reload providers** in Settings.

## Add your own service

1. Find out how the service works: its address, which method (GET or POST), how the text and
   languages are sent, and what the reply looks like. For an official API, use its documentation.
   For a website, open your browser's developer tools (F12), go to the **Network** tab, translate
   something on the site, and look at the request that returns the translation.
2. Copy the example closest to your service and change `id`, `name` and the `[request]` and
   `[response]` sections.
3. Put any API key in the credential store, not in the file: **Settings > Translators > Secrets >
   Add**, give it a name (for example `my-service-key`), and use `{{secret.my-service-key}}` in the file.
4. Use **Test** until the extracted translation is right, then enable the service.

Example: an official API with a key (LibreTranslate-style JSON API).

```toml
[provider]
id = "my-libretranslate"
name = "LibreTranslate (my server)"
enabled = true
max_chars = 5000

[request]
method = "POST"
url = "https://translate.example.org/translate"
body.json = { q = "{{text}}", source = "{{src}}", target = "{{dst}}", api_key = "{{secret.libre-key}}" }

[response]
text  = { jsonpath = "$.translatedText" }
error = { jsonpath = "$.error" }
```

## Format reference

### `[provider]`

| Field | Meaning | Default |
|---|---|---|
| `id` | Unique id, lowercase with hyphens | required |
| `name` | Name shown on the pane | required |
| `enabled` | Service is active | `false` |
| `unofficial` | Show the *unofficial* badge and warning | `false` |
| `auto_translate` | Send incoming clipboard/API text automatically | `false` |
| `max_chars` | Longest text sent in one request. Longer text is split at sentence ends and sent as several requests; a single sentence longer than this is not sent, and the pane says why. | `1000` |
| `rate_limit` | `{ requests = N, per_seconds = S }` | none |
| `timeout_seconds` | Give up after this long | `15` |
| `website` | Opened from the pane's menu | none |

### `[languages]`

| Field | Meaning |
|---|---|
| `auto` | Code the service uses for "detect the source language". Set `auto = ""` for services that detect the language when no source is sent: a query, form or JSON field whose whole value is `{{src}}` is then left out of the request. |
| `map` | App code → service code, e.g. `{ "zh-Hans" = "zh-CN" }`. Codes not listed are sent as they are. |
| `only` | Optional list of supported app codes; the pane is greyed out for others |

### `[params]` (optional)

Values the user sets in the settings UI without editing the file, such as a DeepL plan (Free or Pro),
a Microsoft resource region, or a LibreTranslate server address. Each entry has a label, a type and a
default; use the value as `{{param.name}}`.

```toml
[params]
region = { label = "Resource region", type = "text", default = "global" }
plan   = { label = "Plan", type = "choice", choices = ["free", "pro"], default = "free" }
```

A choice can also pick the address: `url = "{{param.plan_url}}"` together with
`plan_url = { type = "choice", choices = { free = "https://...", pro = "https://..." } }`.

### `[session]` (optional)

Runs before the first request and whenever it needs refreshing. Use it for services whose web page
hands out values that later requests must include.

| Field | Meaning |
|---|---|
| `request` | `{ method, url, headers }` of the page to load |
| `keep_cookies` | Reuse cookies from this response in later requests |
| `refresh_after_uses` | Run the session step again after N translations |
| `refresh_after_seconds` | Run it again after this many seconds |
| `refresh_on_status` | Run it again (and retry once) when a request returns one of these HTTP statuses |
| `[session.extract]` | `name = { regex = '...' }`. The first capture group becomes `{{session.name}}`. Use single quotes so backslashes stay as they are. |

### `[request]`

| Field | Meaning |
|---|---|
| `method` | `GET` or `POST` |
| `url` | Address; placeholders are URL-encoded automatically |
| `query` | Extra URL parameters as a table, e.g. `{ from = "{{src}}", to = "{{dst}}" }`; encoded and appended to the URL for GET and POST |
| `headers` | Extra headers, e.g. `{ Authorization = "Bearer {{secret.my-key}}" }` |
| `body.form` | Form fields (`application/x-www-form-urlencoded`) |
| `body.json` | JSON body; placeholders are inserted as JSON strings |

Use only one of `body.form` and `body.json`.

### `[response]`

| Field | Meaning |
|---|---|
| `text` | Where the translation is: `{ jsonpath = "..." }` for JSON replies, `{ regex = '...' }` for HTML or text replies (first capture group). Add `join = ""` to join several matches (for example one per sentence). |
| `error` | Optional: where an error message is. If it matches, the pane shows it instead of a translation. |
| `refresh_session_if` | Optional: if this matches, the reply means "session no longer valid": run the session step again and retry once. |
| `html_decode` | Decode HTML entities such as `&amp;` in the result (default `false`) |

### Placeholders

| Placeholder | Value |
|---|---|
| `{{text}}` | Text to translate (after substitutions) |
| `{{src}}`, `{{dst}}` | Source and target language, after `[languages]` mapping |
| `{{counter}}` | Requests made with the current session (1, 2, 3, …) |
| `{{session.name}}` | A value extracted in the session step |
| `{{param.name}}` | A value the user set for this service (see `[params]`) |
| `{{secret.name}}` | A secret from the credential store; never written to logs or shown in **Test** |

## When a service stops working

Unofficial endpoints break from time to time. The pane shows the error. Then:

1. Click **Test** and look at the raw reply.
2. **The session step finds nothing** (Bing: "regex did not match"): the web page changed. Open the
   page in your browser, view its source (Ctrl+U), search for the old value's name, and update the
   regex in `[session.extract]`.
3. **HTTP 429, "too many requests", or Bing replies `{"ShowCaptcha": ...}`:** the service rejected
   the request, usually because of too many requests. Wait, and lower `rate_limit`. If it keeps
   happening right after a refresh, the session values are wrong: see step 2.
4. **The reply has a different shape:** update the `jsonpath` in `[response]`. Paste the raw reply
   into a JSONPath tester to find the new path.
5. **Nothing works any more:** the service may have closed the endpoint. Switch to its official API
   preset.

Updated example definitions are published in the repository under `examples/providers/`.
