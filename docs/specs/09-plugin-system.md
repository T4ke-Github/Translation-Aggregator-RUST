# 09 Plugin system (`PLG`)

## 1. Purpose and scope

Plugins add translators, text transforms and input sources without changing the app. Every plugin
is a **separate process** that talks a versioned JSON-RPC protocol with the app (D15). This gives
crash isolation, works across bitness (a 32-bit plugin with the 64-bit app and the other way round,
D1), and lets plugins be written in any language.

Covered here: the manifest, discovery, the trust prompt, transport and framing, the handshake and
versioning, the three plugin kinds, plugin settings and secrets, supervision and crash isolation,
cross-bitness, moving the LLM translator out of process (M6), the protocol v1 document, a
conformance kit and a sample text-transform plugin.

Not covered here: what the LLM translator does (see the LLM spec), the hook plugin's capture (see
the HOOK spec; it feeds text through the local API, see the API spec), and how plugin panes look
(see the UI spec). Plugin timeouts and the restart backoff are shared numbers (NFR-04.7,
NFR-04.8).

## 2. Goals

1. A plugin can crash, hang or misbehave without taking down the app or blocking the UI (NFR-03.3).
2. Any bitness of plugin works with any bitness of the app (D1).
3. No plugin runs before the user has agreed to run it (PLG-02).
4. The protocol is versioned and documented well enough that third parties can write plugins
   against it without reading app code.
5. First-party plugins (LLM translator, sample transform) use only what the protocol offers.

## 3. Features

### PLG-01 Plugin manifest and discovery

**Priority:** Must · **Milestone:** M6 · **Decisions:** D15 · **Legacy:** features.md §4 "Plugins" (`plugins\*.dll`, replaced) · **Depends on:** CFG-01

As a user, I want to install a plugin by putting its folder in the plugins folder, so that the app
finds it and tells me what it is.

**Acceptance criteria**

- **PLG-01.1** Given a folder `plugins\<id>\` with a `plugin.toml` in the data folder (or in the app's
  own `plugins` folder for first-party plugins), when the app starts or the user clicks "Rescan",
  then the plugin is listed with its id, name, version, publisher, description and kinds.
  *(Verify: integration)*
- **PLG-01.2** Given a manifest, when it is read, then it names the executable for x86 and/or x64,
  the supported protocol major version, the kinds (input source, translator, text transform),
  whether it sends text off the machine, and the secret names it may ask for. *(Verify: unit)*
- **PLG-01.3** Given a manifest with a syntax error, a missing required field, or no executable that
  exists, when it is read, then the plugin is listed as "broken" with the reason and is never
  started. *(Verify: unit)*
- **PLG-01.4** Given two plugins with the same id, when they are found, then only the first (by folder
  name) is used and a warning names both folders. *(Verify: unit)*
- **PLG-01.5** Given the app is running, when it looks for plugins, then it never downloads or
  installs a plugin by itself. *(Verify: security)*

### PLG-02 Trust prompt before first run

**Priority:** Must · **Milestone:** M6 · **Decisions:** D15 · **Legacy:** none (legacy DLLs loaded without asking) · **Depends on:** PLG-01

As a user, I want to be asked before a new plugin runs, so that no program starts on my PC that I
did not approve.

**Acceptance criteria**

- **PLG-02.1** Given a third-party plugin that has never run, when it would start, then the app asks
  first and shows its name, publisher, kinds, executable path, SHA-256 hash, whether it sends text
  off the machine, and "it runs with your user rights". *(Verify: manual)*
- **PLG-02.2** Given the user declines, when the app continues, then the plugin stays disabled and the
  app doesn't ask again until the user enables it in settings. *(Verify: integration)*
- **PLG-02.3** Given the user approved a plugin, when its executable later changes (different hash),
  then the app asks again before running it. *(Verify: security)*
- **PLG-02.4** Given a first-party plugin shipped in the app's own folder whose hash matches the list
  in the release, when it starts, then no prompt is shown; a modified copy is treated as
  third-party. *(Verify: security)*

### PLG-03 Transport and framing

**Priority:** Must · **Milestone:** M6 · **Decisions:** D15 · **Legacy:** none (new) · **Depends on:** —

As a plugin author, I want a simple transport any language can use, so that writing a plugin needs
only standard input and output and a JSON library.

**Acceptance criteria**

- **PLG-03.1** Given a started plugin, when the app talks to it, then messages are JSON-RPC 2.0, UTF-8,
  one message per line on the plugin's stdin and stdout. *(Verify: contract)*
- **PLG-03.2** Given the plugin writes a line that is not valid JSON or not valid JSON-RPC, when the
  app reads it, then the app answers with the standard JSON-RPC error (−32700 or −32600), logs it,
  and keeps the plugin running; after 10 such lines within 1 minute, the plugin is restarted
  (PLG-08). *(Verify: contract)*
- **PLG-03.3** Given a message larger than 4 MiB, when either side receives it, then it is rejected as
  a protocol error and not parsed. *(Verify: contract)*
- **PLG-03.4** Given the plugin writes to stderr, when it does, then the output goes to the plugin's
  log file (capped at 1 MB, rotated) and never into the JSON-RPC stream. *(Verify: integration)*
- **PLG-03.5** Given a manifest asks for a named pipe instead of stdio, when the plugin starts, then
  the same messages and framing are used over a pipe that only the current user can open.
  *(Verify: security)*

### PLG-04 Handshake and protocol versioning

**Priority:** Must · **Milestone:** M6 · **Decisions:** D15 · **Legacy:** formats-and-interfaces.md §4 (legacy version check, replaced) · **Depends on:** PLG-03

As a plugin author, I want a clear handshake with versions and capabilities, so that my plugin keeps
working when the app gets new features.

**Acceptance criteria**

- **PLG-04.1** Given a plugin starts, when the app sends `initialize` with its protocol version
  (`1.<minor>`), app version and bitness, then the plugin answers with its protocol version, plugin
  version, kinds and capabilities, and the app then sends `initialized`. *(Verify: contract)*
- **PLG-04.2** Given the plugin answers with an unsupported major version, when the handshake ends,
  then the app stops the plugin and shows "needs protocol version N; this app supports 1".
  *(Verify: contract)*
- **PLG-04.3** Given the plugin has a newer minor version than the app, when the handshake ends, then
  both use only the methods and fields both know; unknown fields are ignored on both sides.
  *(Verify: contract)*
- **PLG-04.4** Given the plugin doesn't answer `initialize` within the handshake timeout
  (NFR-04.7), when the time passes, then the plugin is stopped and shown as "did not start".
  *(Verify: contract)*
- **PLG-04.5** Given the plugin sends a request before the handshake ends, or calls a method for a
  kind it did not declare, when the app gets it, then the app answers with an error and ignores
  it. *(Verify: contract)*
- **PLG-04.6** Given the app shuts a plugin down, when it sends `shutdown` and then `exit`, then a
  conforming plugin exits with code 0. *(Verify: contract)*

### PLG-05 Translator plugins

**Priority:** Must · **Milestone:** M6 · **Decisions:** D8, D15 · **Legacy:** none (new kind) · **Depends on:** PLG-04, PIP-03

As a user, I want a translator plugin to work like any built-in translator, so that I get the same
pane, cache, limits and history for it.

**Acceptance criteria**

- **PLG-05.1** Given a translator plugin, when the handshake ends, then it declares the capabilities
  of PIP-03.1, and it appears as a pane that the router uses like a built-in translator.
  *(Verify: contract)*
- **PLG-05.2** Given an entry, when the app sends `translate` (request id, text, source and target
  language, context id, optional previous lines of the same context), then the plugin answers with
  the translation or an error code from the protocol document. *(Verify: contract)*
- **PLG-05.3** Given a streaming translator, when it sends `translate/partial` notifications before
  the final answer, then the pane and the output stream update in order (PIP-03.5).
  *(Verify: contract)*
- **PLG-05.4** Given a newer entry replaces a request (PIP-10), when the app sends `$/cancel` for it,
  then the plugin stops within the cancel time (NFR-04.7); any later answer for that request is
  ignored. *(Verify: contract)*
- **PLG-05.5** Given the plugin returns an error such as "auth missing", "rate limited" or "quota",
  when the app shows it, then the pane shows the same state as for a built-in provider, and retries
  follow PIP-09. *(Verify: contract)*

### PLG-06 Text-transform plugins

**Priority:** Must · **Milestone:** M6 · **Decisions:** D15 · **Legacy:** features.md §4 "Plugins (pre/post substitution string transforms, active profile notification)" · **Depends on:** PLG-04, PIP-01

As a user, I want plugins that change text before or after substitutions, so that I can clean up
text for a game in ways substitution rules can't.

**Acceptance criteria**

- **PLG-06.1** Given a text-transform plugin enabled for the pre-substitution stage, when an entry
  reaches stage 3 of PIP-01.1, then the app sends `transform` with the stage, text, context id and
  active substitution profiles, and uses the returned text. The same holds for the
  post-substitution stage (stage 5). *(Verify: contract)*
- **PLG-06.2** Given two transform plugins in the same stage, when an entry passes, then they run in
  the order the user set, each getting the previous one's output. *(Verify: integration)*
- **PLG-06.3** Given a transform doesn't answer within the transform timeout (NFR-04.7), or returns
  an error or something other than a string, when the stage runs, then that transform is skipped
  for this entry (PIP-01.3). *(Verify: contract)*
- **PLG-06.4** Given a transform returns the text unchanged or empty, when the stage ends, then
  unchanged text continues, and empty text is handled as in PIP-01.4. *(Verify: unit)*
- **PLG-06.5** Given the active substitution profiles change, when the next `transform` is sent, then
  it carries the new list (replaces the legacy "active profile" notification). *(Verify: contract)*

### PLG-07 Input-source plugins

**Priority:** Should · **Milestone:** M6 · **Decisions:** D5, D6, D15 · **Legacy:** none (new kind) · **Depends on:** PLG-04, IN-05

As a plugin author, I want to push text into the app over the plugin channel, so that my input tool
doesn't need the local API to be turned on.

**Acceptance criteria**

- **PLG-07.1** Given an input-source plugin, when it sends `text/push` with a source name and text,
  then the text enters context `plugin:<plugin-id>/<source>` and goes through the same pipeline as
  any other input (IN-05, PIP-01). *(Verify: contract)*
- **PLG-07.2** Given a plugin that did not declare the input-source kind, when it sends `text/push`,
  then the app answers with an error and ignores the text. *(Verify: contract)*
- **PLG-07.3** Given an input-source plugin pushes text faster than the input rate limit (NFR-04.6),
  when the limit is reached, then extra pushes get a "rate limited" error and the UI stays within
  NFR-03.3. *(Verify: integration)*
- **PLG-07.4** Given input is paused (IN-02.2), when a plugin pushes text, then it gets an
  "input paused" error. Input-source plugins may use the local API instead (the hook plugin does,
  D6). *(Verify: contract)*

### PLG-08 Supervision and crash isolation

**Priority:** Must · **Milestone:** M6 · **Decisions:** D15 · **Legacy:** none (a crashing legacy plugin DLL crashed the app) · **Depends on:** PLG-04

As a user, I want a broken plugin to be restarted or switched off without affecting the rest of the
app, so that one bad plugin doesn't ruin my session.

**Acceptance criteria**

- **PLG-08.1** Given an approved, enabled plugin, when the app starts or the plugin's pane or stage is
  first needed, then it is started; plugins that are not needed are not started.
  *(Verify: integration)*
- **PLG-08.2** Given a running plugin, when the app sends `ping` at the ping interval and gets no
  answer within the ping timeout (NFR-04.7), then the plugin is treated as hung, killed and
  restarted. *(Verify: integration)*
- **PLG-08.3** Given a plugin exits unexpectedly, when it is restarted, then the waits follow the
  restart backoff (NFR-04.8); after the crash limit in NFR-04.8 it is disabled with "crashed
  repeatedly" and a manual "Restart" action. *(Verify: integration)*
- **PLG-08.4** Given a plugin crashes while requests are in flight, when this happens, then those
  requests end with "plugin stopped", other translators and the UI are unaffected (NFR-03.3), and
  the app does not crash. *(Verify: integration)*
- **PLG-08.5** Given the app stops a plugin, when it sends `shutdown`, then it waits for the
  shutdown time (NFR-04.7) and then kills it. Given the app itself crashes or is killed, then no plugin process keeps running.
  *(Verify: integration)*
- **PLG-08.6** Given the plugins page, when the user opens it, then each plugin shows its state
  (running, stopped, starting, crashed, disabled, broken), restart count, and a link to its log.
  *(Verify: manual)*

### PLG-09 Cross-bitness

**Priority:** Must · **Milestone:** M6 · **Decisions:** D1, D15 · **Legacy:** none (legacy plugins had to be 32-bit) · **Depends on:** PLG-01

As a user, I want plugins to work whatever build of the app I use, so that I don't need matching
32-bit and 64-bit copies of every plugin.

**Acceptance criteria**

- **PLG-09.1** Given a manifest with both x86 and x64 executables, when the app starts the plugin,
  then it uses the one that matches the app's own bitness. *(Verify: unit)*
- **PLG-09.2** Given a manifest with only the other bitness, on 64-bit Windows, when the app starts
  the plugin, then that executable is used and works: the 32-bit app runs a 64-bit plugin and the
  64-bit app runs a 32-bit plugin. *(Verify: CI)*
- **PLG-09.3** Given a plugin with only an x64 executable, on 32-bit Windows, when it would start,
  then it is listed as "needs 64-bit Windows" and not started. *(Verify: unit)*
- **PLG-09.4** Given the CI matrix {32-bit app, 64-bit app} × {32-bit fake plugin, 64-bit fake
  plugin}, when the conformance tests run, then all four combinations pass. *(Verify: CI)*

### PLG-10 Plugin settings and secrets

**Priority:** Must · **Milestone:** M6 · **Decisions:** D8, D15 · **Legacy:** none (new) · **Depends on:** PLG-04, CFG-05

As a user, I want to set up a plugin in the settings UI and give it an API key safely, so that I
never edit plugin files or paste keys into them.

**Acceptance criteria**

- **PLG-10.1** Given a manifest with a settings schema, when the user opens the plugin's settings, then
  a form is shown for its fields (text, number, yes/no, choice, secret reference), and values are
  stored in the app's TOML settings. *(Verify: manual)*
- **PLG-10.2** Given settings are saved or the plugin starts, when the app sends `configure` with the
  settings, then a rejected field is shown next to that field with the plugin's message.
  *(Verify: contract)*
- **PLG-10.3** Given a plugin calls `secrets/get` for a name listed in its manifest, when the secret
  exists, then the app returns its value; for a name not in the manifest, the app returns an error
  and doesn't reveal whether that secret exists. *(Verify: security)*
- **PLG-10.4** Given secrets pass over the protocol, when messages are logged, then secret values are
  replaced by `***` in the app's protocol log (NFR-05.1). *(Verify: security)*

### PLG-11 LLM translator out of process

**Priority:** Must · **Milestone:** M6 · **Decisions:** D8, D15, D1 · **Legacy:** none (new) · **Depends on:** PLG-05, PLG-08, PLG-10, LLM-01 … LLM-11

As a user, I want the LLM translator to run as its own process, so that a hang or crash in it never
affects the app, and it uses the same protocol as third-party plugins.

This feature replaces LLM-12 (withdrawn), so the move out of process is specified only here.

**Acceptance criteria**

- **PLG-11.1** Given the M6 release, when the app runs as the 32-bit or the 64-bit build, then the LLM
  translator runs as a plugin process that the app supervises (PLG-08). *(Verify: CI)*
- **PLG-11.2** Given the LLM contract tests of LLM-01 to LLM-11 (recorded streams, mock server),
  when they run against the plugin process, then they pass unchanged. *(Verify: contract)*
- **PLG-11.3** Given the optional compiled-in build of the LLM translator, when the same tests run,
  then they pass using only protocol messages (an in-memory channel), with no extra access to the
  app. *(Verify: contract)*
- **PLG-11.4** Given a user with M3 LLM settings, when they upgrade to the M6 build, then backends,
  models, prompts and secret references are kept (CFG-02), without asking for keys again.
  *(Verify: integration)*
- **PLG-11.5** Given a streaming translation, when a newer entry arrives, then the plugin stops it
  within the cancel time (PLG-05.4) and the pane shows the newer entry. *(Verify: integration)*
- **PLG-11.6** Given the LLM plugin process crashes during a stream, when the app notices, then the
  pane keeps the partial text marked "plugin stopped", the app stays responsive, and the plugin is
  restarted as in PLG-08.3. (Moved from LLM-12.3.) *(Verify: integration)*
- **PLG-11.7** Given the LLM plugin needs a key, when it gets it, then the key is passed only over
  the plugin channel (PLG-10.3) or read by the plugin from the credential store, never through
  command-line arguments, environment variables or files. (Moved from LLM-12.4.)
  *(Verify: security)*

### PLG-12 Protocol v1 document, conformance kit and sample plugin

**Priority:** Must · **Milestone:** M6 · **Decisions:** D15, D3, D13 · **Legacy:** formats-and-interfaces.md §4 "Sample Dll" (replaced) · **Depends on:** PLG-03, PLG-04, PLG-06

As a plugin author, I want a complete protocol document, a test kit and a working example, so that I
can write and check a plugin on my own.

**Acceptance criteria**

- **PLG-12.1** Given the M6 release, when a reader opens `docs/plugins/protocol-v1.md`, then it
  describes the manifest, framing, every method and notification with its fields, error codes,
  timeouts, the trust and supervision rules, and the versioning policy: minor versions only add
  optional methods and fields; a new major version keeps the previous one supported for at least one
  release. *(Verify: manual)*
- **PLG-12.2** Given JSON Schema files for protocol v1, when the conformance tests run, then every
  message the app and the first-party plugins send validates. *(Verify: contract)*
- **PLG-12.3** Given the conformance kit (a fake app that drives a plugin executable), when a plugin
  author runs it against a plugin, then it checks the handshake, each declared kind, cancellation,
  error codes and clean shutdown, and reports pass or fail per check. *(Verify: CI)*
- **PLG-12.4** Given the sample text-transform plugin in the repo (licensed `MIT OR Apache-2.0`,
  written fresh, D13), when CI builds it for x86 and x64, then it passes the conformance kit and
  changes text as its README describes. *(Verify: CI)*

## 4. Out of scope

- Legacy `__stdcall` plugin DLLs (`TAPlugin.h`) and any in-process DLL plugins: not supported (D15).
- Downloading plugins, a plugin store or automatic updates: the app never downloads plugins
  (risks-and-open-questions.md §8).
- Sandboxing beyond a separate process (restricted tokens, AppContainer): not in v1; plugins run with
  the user's rights and the trust prompt says so.
- Plugins that change the UI or trigger actions from translated text: plugins only provide text.
- The hook plugin's internal link to its hook DLLs: private to the hook plugin (D6; see the HOOK
  spec).

## 5. Open questions

None of O10–O14 affects this area directly. Translator plugins with remote = yes start with
auto-translate off like every added translator (PIP-05, *assumes O10 recommendation*).

## 6. Traceability

| Feature | Priority | Milestone | Decisions |
|---|---|---|---|
| PLG-01 Plugin manifest and discovery | Must | M6 | D15 |
| PLG-02 Trust prompt before first run | Must | M6 | D15 |
| PLG-03 Transport and framing | Must | M6 | D15 |
| PLG-04 Handshake and protocol versioning | Must | M6 | D15 |
| PLG-05 Translator plugins | Must | M6 | D8, D15 |
| PLG-06 Text-transform plugins | Must | M6 | D15 |
| PLG-07 Input-source plugins | Should | M6 | D5, D6, D15 |
| PLG-08 Supervision and crash isolation | Must | M6 | D15 |
| PLG-09 Cross-bitness | Must | M6 | D1, D15 |
| PLG-10 Plugin settings and secrets | Must | M6 | D8, D15 |
| PLG-11 LLM translator out of process | Must | M6 | D1, D8, D15 |
| PLG-12 Protocol v1 document, conformance kit and sample plugin | Must | M6 | D3, D13, D15 |
