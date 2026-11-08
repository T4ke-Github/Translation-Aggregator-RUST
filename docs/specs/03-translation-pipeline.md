# 03 Translation pipeline (`PIP`)

## 1. Purpose and scope

This area covers what happens to an entry after its input context has flushed it (see the IN spec)
until results reach the panes, the history and the output stream:

- the order of the pipeline stages,
- the substitutions engine with a global and per-context profiles,
- the capabilities every translator declares,
- the router: fan-out, auto-translate defaults, budgets, the auto-request length cap, cache, rate
  limits, timeouts, retries, cancel and replace,
- the history: back/forward, search, export, size limits.

Not covered here: how individual translators build requests (see the WEB and LLM specs), MeCab and
JParser output (see the JP spec), the plugin protocol (see the PLG spec), and how panes look (see the
UI spec).

## 2. Goals

1. Every input reaches every chosen translator through one fixed, documented order of stages, so
   results are predictable (D5).
2. Substitutions give the same results as the legacy app for the same rules (D4, D12, D16).
3. A slow, failing or expensive provider never blocks the others or the UI.
4. Paid and remote providers only get text the user agreed to send, within limits the user sets
   (O10, NFR-05).
5. Every result can be found again in the history without a new request.

## 3. Features

### PIP-01 Pipeline stages and order

**Priority:** Must · **Milestone:** M1 · **Decisions:** D5, D12, D15 · **Legacy:** architecture.md §2.2 "Translation pipeline"; features.md §4 "Substitutions", "Plugins" · **Depends on:** IN-06, PIP-02, PIP-04

As a user, I want every entry to pass through the same steps in the same order, so that I can tell
why a translator got the text it got.

Stages arrive with their features: normalisation (stage 2) in M2 with IN-11, and text transforms
(stages 3 and 5) in M6 with PLG-06. Until then those stages pass text on unchanged; the M1 order
test uses test-only transforms. Stages 2–5 keep the legacy order. The history entry is written
after processing (stage 6) instead of before substitutions as in the legacy app, so it can hold
both texts (behaviour change, D20).

**Acceptance criteria**

- **PIP-01.1** Given an automatic entry, when it is processed, then the stages run in this order:
  (1) context buffering and filters (IN-06 to IN-10), (2) normalisation (IN-11), (3) pre-substitution
  text transforms (PLG-06), (4) substitutions (PIP-02), (5) post-substitution text transforms
  (PLG-06), (6) history entry (PIP-11), (7) router to translators and local tools (PIP-04). A test
  with marker-adding transforms checks the order. *(Verify: unit)*
- **PIP-01.2** Given an entry, when it reaches the router, then every translator and local tool
  (MeCab, JParser) receives the same processed text, and the Original Text pane can show both the
  original and the processed text. The one exception is an LLM provider with "send substitutions
  as glossary" on, which gets the text before substitutions (LLM-05.3). *(Verify: integration)*
- **PIP-01.3** Given a text-transform plugin fails or times out, when its stage runs, then that
  transform is skipped for this entry, a warning names the plugin, and the entry continues
  unchanged by it. *(Verify: integration)*
- **PIP-01.4** Given substitutions turn an entry into empty text, when the router is reached, then no
  request is sent and the entry is shown as "empty after substitutions". *(Verify: unit)*
- **PIP-01.5** Given any input, when the pipeline runs, then no stage runs on the UI thread
  (NFR-03.3), and local-only results arrive within NFR-03.2. *(Verify: integration)*

### PIP-02 Substitutions engine and profiles

**Priority:** Must · **Milestone:** M1 · **Decisions:** D12, D16, D4 · **Legacy:** features.md §4 "Substitutions"; formats-and-interfaces.md §3 · **Depends on:** IN-05

As a user, I want my substitution rules to fix names and terms before translation, with global
rules and rules for one game or source, so that translators get the words I want.

**Acceptance criteria**

- **PIP-02.1** Given rules `日本` → `Japan` and `日本語` → `Japanese`, when `日本語と日本` is processed,
  then the result is `JapaneseとJapan`: the longest match wins, matching runs left to right without
  overlaps, and replaced text is not scanned again (`a` → `b`, `b` → `c` turns `a` into `b`).
  *(Verify: parity)*
- **PIP-02.2** Given a rule with `\n`, `\t` or `\\` in its match or replacement, when it is applied,
  then they mean line break, tab and backslash; any other backslash is taken literally.
  *(Verify: parity)*
- **PIP-02.3** Given the global profile `*` and a profile assigned to context `api:hook/game.exe`
  (by exact id or a `*` wildcard pattern), when an entry from that context is processed, then the
  rules of both profiles are active; entries from other contexts use only `*`. *(Verify: unit)*
- **PIP-02.4** Given the global and a context profile both have a match of the same length at the same
  place, when the entry is processed, then the context profile's rule wins. *(Verify: unit)*
- **PIP-02.5** Given a rule with an empty replacement, when it matches, then the match is deleted;
  given a rule with an empty match, when it is saved or imported, then it is rejected with a
  message. Given "Enable substitutions" is off, then text passes unchanged. *(Verify: unit)*
- **PIP-02.6** Given 10,000 rules over 5 profiles, when a 500-character entry is processed, then
  substitution finishes within NFR-03.2, and the Phase-0 `UserDict.txt` fixtures give the legacy
  results. *(Verify: parity)*

### PIP-03 Translator capabilities

**Priority:** Must · **Milestone:** M1 · **Decisions:** D5, D8, D10, D17 · **Legacy:** features.md §5 "Panes enable or disable themselves by capability" · **Depends on:** —

As a user, I want each translator to say what it can do, so that the app only sends it text it can
handle and tells me why when it can't.

**Acceptance criteria**

- **PIP-03.1** Given any translator (web preset, custom service, LLM, plugin, local tool), when it is
  registered, then it declares id, name, kind, supported source and target languages (and whether
  it detects the source language), maximum request length, streaming yes/no, remote yes/no ("text
  leaves this machine") and a cost hint (free, metered, paid). *(Verify: contract)*
- **PIP-03.2** Given the current language pair is not supported by a translator, when an entry
  arrives, then that translator gets no request and its pane is greyed out with the reason.
  *(Verify: unit)*
- **PIP-03.3** Given an entry longer than a translator's maximum request length, when it would be
  sent (automatically or manually), then it is not sent and the pane says "Too long for this
  provider (N characters, maximum M)". *(Verify: unit)*
- **PIP-03.4** Given a translator declares remote = yes, when its pane is shown, then it carries the
  "text leaves this machine" marker (NFR-05.3). *(Verify: manual)*
- **PIP-03.5** Given a translator declares streaming, when partial results arrive, then the pane and
  the output stream update as they arrive; otherwise the result is shown when complete.
  *(Verify: integration)*
- **PIP-03.6** Given a translator's capabilities change (definition edited, plugin restarted), when
  the next entry arrives, then the new capabilities apply without an app restart.
  *(Verify: integration)*

### PIP-04 Router fan-out

**Priority:** Must · **Milestone:** M1 · **Decisions:** D5 · **Legacy:** features.md §1 "Common pane behaviour"; architecture.md §2.2 "AutoTranslate" · **Depends on:** PIP-03

As a user, I want each new entry to go to all my chosen translators at once, so that I can compare
results side by side.

**Acceptance criteria**

- **PIP-04.1** Given three translators with auto-translate on and a context with auto translate on
  (IN-10), when an entry arrives, then all three get a request in parallel. *(Verify: integration)*
- **PIP-04.2** Given one translator takes until its timeout (NFR-04.2), when the others answer in
  1 s, then their results are shown at once, not after the slow one. *(Verify: integration)*
- **PIP-04.3** Given a result arrives, when it is delivered, then the pane, the history entry and the
  output stream (API-04, from M5) get the text or error, status, elapsed time and cached flag.
  *(Verify: integration)*
- **PIP-04.4** Given the user runs "Translate All", when the entry is sent, then every enabled
  translator gets it, whether or not its auto-translate is on. *(Verify: integration)*
- **PIP-04.5** Given no translator is enabled, when an entry arrives, then it is shown and stored,
  no error is raised, and the Original Text pane offers "Add a translator". *(Verify: manual)*

### PIP-05 Auto-translate defaults for remote and paid providers

**Priority:** Must · **Milestone:** M1 · **Decisions:** D5, D8, D17 · **Legacy:** features.md §1 "Automatic Clipboard Translation" toggle (default on for local panes, off for remote ones) · **Depends on:** PIP-03

As a user, I want remote and paid translators to stay quiet until I turn on automatic translation
for them, so that I don't send private text or spend money by accident. *(assumes O10
recommendation)*

**Acceptance criteria**

- **PIP-05.1** Given a newly added translator (web preset, custom service, LLM backend or translator
  plugin), when it is enabled, then its auto-translate is off; built-in local tools (MeCab, JParser)
  start with it on. *(assumes O10 recommendation)* *(Verify: unit)*
- **PIP-05.2** Given a translator with remote = yes, when the user turns its auto-translate on, then
  a one-time confirmation names the provider, says the text leaves this machine and may cost money;
  "Cancel" keeps it off. *(Verify: manual)*
- **PIP-05.3** Given auto-translate is on but the provider has no key or is disabled, when an entry
  arrives, then no request is sent and the pane shows that state. *(Verify: unit)*
- **PIP-05.4** Given automatic input is paused (IN-02.2), when entries would be sent, then no
  automatic request is sent; manual requests still work. *(Verify: integration)*
- **PIP-05.5** Given a legacy INI where a remote pane had "Auto Clipboard" on, when it is imported
  (CFG-06), then the mapped provider gets auto-translate on, and the confirmation of PIP-05.2 is shown
  once before its first automatic request. *(M2, with CFG-06.)* *(Verify: integration)*

### PIP-06 Per-provider budgets

**Priority:** Should · **Milestone:** M3 · **Decisions:** D8, D17 · **Legacy:** none (new); risks-and-open-questions.md §2 · **Depends on:** PIP-04

As a user, I want to cap how many requests or characters a provider gets per day or month, so that
automatic translation can't run up a bill. *(assumes O10 recommendation)*

**Acceptance criteria**

- **PIP-06.1** Given a provider settings page, when the user sets a budget, then it can limit requests
  and/or characters per day and/or per month; no budget is set by default. *(Verify: manual)*
- **PIP-06.2** Given a budget, when requests are sent, then the pane header shows used / limit; only
  requests actually sent count (cache hits don't, each retry does). *(Verify: unit)*
- **PIP-06.3** Given a budget is used up, when the next automatic entry arrives, then no request is
  sent and the pane shows "Budget reached (N of N today)"; a manual request asks for confirmation
  once per period. *(Verify: integration)*
- **PIP-06.4** Given entries arrive in parallel, when the budget is nearly used up, then it is never
  exceeded: the budget is reserved before each request is sent. *(Verify: unit)*
- **PIP-06.5** Given the app restarts, when counters are shown, then they keep their values, and they
  reset at local midnight (daily) and on the first day of the month (monthly). *(Verify: unit)*

### PIP-07 Auto-request length cap

**Priority:** Must · **Milestone:** M1 · **Decisions:** D5, D20 · **Legacy:** features.md §3 "500-character cap for automatic web requests" · **Depends on:** PIP-03

As a user, I want very long text not to be sent automatically to remote providers, so that an
accidental copy of a whole document doesn't go out or cost money.

**Acceptance criteria**

- **PIP-07.1** Given the default cap (NFR-04.1), when an automatic entry is longer than the cap after
  substitutions, then translators get no request (local tools are exempt, PIP-07.4) and their panes
  say "Too long for automatic
  translation (N characters, limit L). Use Translate to send it." *(Verify: unit)*
- **PIP-07.2** Given an entry of exactly the cap length, when it is automatic, then it is sent (the
  legacy app skipped it; listed as a behaviour change, D20). *(Verify: unit)*
- **PIP-07.3** Given an entry over the cap, when the user runs Translate on that pane, then it is sent,
  still subject to the provider's maximum request length (PIP-03.3). *(Verify: integration)*
- **PIP-07.4** Given a provider has its own cap setting, when entries arrive, then that value is used
  for it instead of the global cap; local tools have no cap by default. *(Verify: unit)*
- **PIP-07.5** Given text with characters outside the Basic Multilingual Plane, when length is
  checked, then characters are counted as Unicode scalar values, not UTF-16 units.
  *(Verify: unit)*

### PIP-08 Result cache

**Priority:** Must · **Milestone:** M1 · **Decisions:** D5 · **Legacy:** features.md §1 "Results cached in History" · **Depends on:** PIP-04

As a user, I want text I have translated before to come back at once without a new request, so
that repeated lines are free and fast.

**Acceptance criteria**

- **PIP-08.1** Given a provider has translated a processed text with the same languages and the same
  provider settings, when the same text arrives again, then the cached result is shown within
  NFR-03.2, marked "cached", no request is sent and no budget is used. *(Verify: integration)*
- **PIP-08.2** Given a provider's settings change (definition, model, prompt, options), when the same
  text arrives, then the cache misses and a new request is sent. *(Verify: unit)*
- **PIP-08.3** Given a request failed, was cancelled or ended after partial output, when the same text
  arrives, then nothing from that request is served from the cache. *(Verify: unit)*
- **PIP-08.4** Given the cache holds its maximum (NFR-04.9), when a new result is added, then the least recently used one is dropped, and memory stays bounded.
  *(Verify: unit)*
- **PIP-08.5** Given a provider definition says caching is not allowed (provider terms), when it
  returns results, then they are never served from the cache. *(Verify: unit)*
- **PIP-08.6** Given the user runs "Retry" on a pane, when it runs, then the cache is bypassed and the
  new result replaces the cached one. *(Verify: integration)*

### PIP-09 Rate limits, timeouts and retries

**Priority:** Must · **Milestone:** M1 · **Decisions:** D10, D17 · **Legacy:** none (legacy had none per provider); risks-and-open-questions.md §2 · **Depends on:** PIP-04, PIP-10

As a user, I want the app to respect each provider's limits and to recover from short failures by
itself, so that I don't get blocked and don't have to click retry for every hiccup.

**Acceptance criteria**

- **PIP-09.1** Given a provider with a rate limit of N requests per S seconds, when entries arrive
  faster, then no more than N requests are sent in any S-second window; extra requests wait in the
  provider's queue (PIP-10). *(Verify: unit)*
- **PIP-09.2** Given the default timeout (NFR-04.2), when a provider does not answer in time, then the
  request is abandoned and the pane shows "Timed out after T s" with a Retry action; a
  per-provider timeout overrides the default. *(Verify: contract)*
- **PIP-09.3** Given a transient failure (connection error, timeout, HTTP 429 or any HTTP 5xx), when it
  happens, then the request is retried as in NFR-04.3; a `Retry-After` shorter than the timeout is
  honoured. *(Verify: contract)*
- **PIP-09.4** Given a permanent failure (HTTP 400, 401, 403, quota exceeded, bad definition), when it
  happens, then it is not retried and the pane shows the matching state (key missing or invalid,
  quota, bad definition). A status listed in a web definition's `refresh_on_status` is handled by
  WEB-08.5 first. *(Verify: contract)*
- **PIP-09.5** Given a provider is down (every request refused), when entries keep arriving, then
  other providers are unaffected, the provider's health shows the error state (WEB-12.1, UI-10.5;
  indicator from M2), and the next entry tries again. *(Verify: integration)*

### PIP-10 Cancel and replace in-flight requests

**Priority:** Must · **Milestone:** M1 · **Decisions:** D5, D20 · **Legacy:** features.md §1 "One request in flight; a newer request replaces the queued one" · **Depends on:** PIP-04

As a user, I want panes to follow the newest text when lines arrive quickly, so that I am not
waiting for translations of lines I already skipped.

**Acceptance criteria**

- **PIP-10.1** Given a provider has a request in flight, when two newer entries arrive, then the
  provider has at most one queued request, for the newest entry; the middle entry is never sent.
  *(Verify: unit)*
- **PIP-10.2** Given a streaming or local translator is working on an entry, when a newer entry
  arrives, then the older request is cancelled within 1 s, its partial output is removed from the
  pane, and the newer entry starts. *(Verify: integration)*
- **PIP-10.3** Given a non-streaming remote request is in flight, when a newer entry arrives, then
  the request finishes, its result is stored in history and shown until the newer result arrives.
  *(Verify: integration)*
- **PIP-10.4** Given 20 entries arrive within 1 s, when the burst ends, then each provider has sent at
  most 2 requests for it, and every pane ends showing the result for the last entry.
  *(Verify: integration)*
- **PIP-10.5** Given a request is cancelled, when this is recorded, then it is not an error, not
  retried, not cached, and its history slot shows "skipped". *(Verify: unit)*

### PIP-11 History: storage and back/forward

**Priority:** Must · **Milestone:** M1 (PIP-11.2 to PIP-11.4: M2, they need the GUI) · **Decisions:** D11, D12 · **Legacy:** features.md §3 "Translation history back/forward"; architecture.md §2.2 "History navigation" · **Depends on:** PIP-04

As a user, I want to step back and forward through earlier lines and see the translations I already
got, so that I can re-read dialogue without new requests.

**Acceptance criteria**

- **PIP-11.1** Given an entry is processed, when it is stored, then the history keeps its id, time,
  context, original text, processed text, and per provider the result text or error, status,
  elapsed time and cached flag. *(Verify: unit)*
- **PIP-11.2** Given several entries, when the user presses Back or Forward (Alt-Up/Alt-PgUp,
  Alt-Down/Alt-PgDn, optional global hotkeys), then the Original Text pane and every pane show the
  stored entry and results, and no request is sent. *(Verify: integration)*
- **PIP-11.3** Given a stored entry has no result for a provider, when the user navigates to it, then
  that pane says "Not translated for this entry" and offers Translate; nothing is sent
  automatically. *(Verify: integration)*
- **PIP-11.4** Given the user is viewing an older entry, when a new automatic entry arrives, then
  the view stays on the entry being read: the Original Text pane and every pane keep showing it.
  The new entry is still processed, translated and added to the history, and the status line
  shows "N newer entries" with a "Go to newest" button. Forward moves one entry newer. At the
  newest entry Forward does nothing and at the oldest Back does nothing; the status line says so.
  This differs from the legacy app, which jumped to the newest entry (owner decision 2026-11-08).
  *(Verify: manual)*
- **PIP-11.5** Given the history holds its maximum (NFR-04.9), when a new entry is added, then the oldest is dropped. *(Verify: unit)*
- **PIP-11.6** Given default settings, when the app closes, then history is not written to disk;
  with "keep history between sessions" on it is restored on start, and turning it off deletes the
  stored file. *(Verify: integration)*

### PIP-12 History search and export

**Priority:** Should · **Milestone:** M2 · **Decisions:** D11 · **Legacy:** features.md §3 "Translation log file" (replaced by history export); ui-reference.md §10 item 8 · **Depends on:** PIP-11

As a user, I want to search my history and export it, so that I can find a line again and keep a
log of a game.

**Acceptance criteria**

- **PIP-12.1** Given a full history (NFR-04.9), when the user searches for a word, then entries whose original,
  processed text or any translation contains it (case-insensitive) are listed within 200 ms, and
  the search runs off the UI thread (NFR-03.3). *(Verify: integration)*
- **PIP-12.2** Given search results, when the user filters by context or provider, then only matching
  entries remain. *(Verify: unit)*
- **PIP-12.3** Given an entry in the results, when the user runs Re-translate, then the chosen
  providers translate it again (bypassing the cache) and the stored results are replaced.
  *(Verify: integration)*
- **PIP-12.4** Given entries are selected, when the user exports them as UTF-8 text, TSV or JSON, then
  the file holds time, context, original text and each provider's result, and no secret, token or
  provider credential (NFR-05.1). *(Verify: security)*
- **PIP-12.5** Given the user clears history (all or one context) and confirms, when it completes,
  then the entries are gone from memory and, if history is kept between sessions, from disk.
  *(Verify: integration)*

## 4. Out of scope

- ATLAS rule sets (`.trs`) for sentence breaking before ATLAS: cut with ATLAS (D7).
- Legacy `__stdcall` pre/post-substitution plugin DLLs: replaced by out-of-process text transforms
  (D15, PLG-06).
- Profiles that turn on because a matching process is running: replaced by profiles keyed by
  context (D6; the hook plugin's context ids name the game).
- Automatic splitting of over-long text by the router: not planned for v1; too-long text is
  reported (PIP-03.3). A web provider may split text itself at sentence ends (WEB-05.2); its
  maximum request length in PIP-03.1 is then the total length it accepts.
- Offline translation engines in the core: cut (D7); local translators can be plugins (PLG-05).

## 5. Open questions

- **O10** (default auto-translate for remote providers): this spec uses the recommendation, off for
  every added translator with per-provider budgets (PIP-05, PIP-06).

Note for the owner: legacy "post-substitution" means a plugin step after substitutions and before
translation (PIP-01.1, stage 5). A transform of translated output is not part of v1.

## 6. Traceability

| Feature | Priority | Milestone | Decisions |
|---|---|---|---|
| PIP-01 Pipeline stages and order | Must | M1 | D5, D12, D15 |
| PIP-02 Substitutions engine and profiles | Must | M1 | D4, D12, D16 |
| PIP-03 Translator capabilities | Must | M1 | D5, D8, D10, D17 |
| PIP-04 Router fan-out | Must | M1 | D5 |
| PIP-05 Auto-translate defaults | Must | M1 | D5, D8, D17 (O10) |
| PIP-06 Per-provider budgets | Should | M3 | D8, D17 (O10) |
| PIP-07 Auto-request length cap | Must | M1 | D5, D20 |
| PIP-08 Result cache | Must | M1 | D5 |
| PIP-09 Rate limits, timeouts and retries | Must | M1 | D10, D17 |
| PIP-10 Cancel and replace in-flight requests | Must | M1 | D5, D20 |
| PIP-11 History: storage and back/forward | Must | M1 (navigation M2) | D11, D12 |
| PIP-12 History search and export | Should | M2 | D11 |
