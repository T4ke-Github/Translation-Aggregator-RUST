# M1 core walkthrough

Run on an interactive Windows 10 or 11 desktop. Use synthetic clipboard text in a dedicated test
session. Wait for each case to finish before changing the clipboard again. Record the commit,
Windows version, screen scale, build target, and observed result for each run.

## Setup

Build both targets from the repository root in PowerShell.

```powershell
rustup target add x86_64-pc-windows-msvc i686-pc-windows-msvc
cargo build --locked -p ta-gui --target x86_64-pc-windows-msvc
cargo build --locked -p ta-gui --target i686-pc-windows-msvc
```

Save the following as `libretranslate-mock.py` in a scratch directory. Run
`py -3 libretranslate-mock.py` in a separate terminal. It listens on loopback, logs each request,
and returns a fixed response.

```python
import json
from http.server import BaseHTTPRequestHandler, HTTPServer


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        print(self.path, json.dumps(request, ensure_ascii=True), flush=True)
        if self.path != "/translate":
            self.send_error(404)
            return
        reply = b'{"translatedText":"Good evening"}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(reply)))
        self.end_headers()
        self.wfile.write(reply)


with HTTPServer(("127.0.0.1", 5000), Handler) as server:
    print("Listening on 127.0.0.1:5000", flush=True)
    server.serve_forever()
```

Before starting the harness, enter these commands in a separate Windows PowerShell 5.1 terminal
started with `powershell.exe -STA`. This prepares the test inputs.

```powershell
Add-Type -AssemblyName System.Windows.Forms
function Set-ProtectedClipboard {
    $data = New-Object System.Windows.Forms.DataObject
    $data.SetText('ひみつ', [System.Windows.Forms.TextDataFormat]::UnicodeText)
    $marker = [System.IO.MemoryStream]::new([byte[]](1, 0, 0, 0))
    $data.SetData('ExcludeClipboardContentFromMonitorProcessing', $false, $marker)
    [System.Windows.Forms.Clipboard]::SetDataObject($data, $true)
    if (-not [System.Windows.Forms.Clipboard]::GetDataObject().GetDataPresent(
        'ExcludeClipboardContentFromMonitorProcessing', $false)) {
        throw 'Clipboard exclusion marker is missing'
    }
}
function Set-RejectedClipboard { Set-Clipboard -Value 'English only' }
function Set-WhitespaceClipboard { Set-Clipboard -Value '   ' }
function Set-AcceptedClipboard { Set-Clipboard -Value 'こんにちは' }
function Set-EquivalentClipboard { Set-Clipboard -Value 'こんばんは' }
function Set-BaselineClipboard { Set-Clipboard -Value '開始前' }
```

From the repository root, enter the launch command in a third terminal. Before executing it,
type `Set-BaselineClipboard` in the clipboard terminal. Type the remaining test commands so
copying the walkthrough does not create additional clipboard events.

```powershell
$target = 'x86_64-pc-windows-msvc' # Repeat with 'i686-pc-windows-msvc'.
& ".\target\$target\debug\translation-aggregator.exe" --clipboard `
    --libretranslate-server 127.0.0.1:5000 --source ja --target en `
    --japanese-only --substitute 'こんにちは' 'こんばんは'
```

## Loopback smoke test

Type the commands in the clipboard terminal. Check the harness and mock logs after each step.
Stop the harness with Ctrl+C. Restart the mock to clear its log for the next run, then set the
baseline and repeat the sequence with the other build.

| Check | Steps | Expected | 64-bit | 32-bit |
| --- | --- | --- | --- | --- |
| Startup baseline | Wait for `Watching for clipboard changes (Ctrl+C to stop).`, then wait one second. | The existing `開始前` value produces zero requests. | Passed | Passed |
| Japanese-only filter | Type `Set-RejectedClipboard` and wait one second. | The mock receives zero requests. | Passed | Passed |
| Whitespace | Type `Set-WhitespaceClipboard` and wait one second. | The mock receives zero requests. | Passed | Passed |
| Exclusion format (IN-02.1) | Type `Set-ProtectedClipboard` and wait one second. | `ひみつ` is absent from harness output and the mock receives zero requests. | Passed | Passed |
| Clipboard to result (IN-01.1, WEB-06.2) | Type `Set-AcceptedClipboard`. | The harness prints context `clipboard`, original `こんにちは`, processed `こんばんは`, and `Good evening`. The mock receives one `POST /translate` with `q` equal to `こんばんは` (logged as Unicode escapes), `source` equal to `ja`, `target` equal to `en`, and no `api_key`. | Passed | Passed |
| Cache hit (PIP-08.1) | Type `Set-EquivalentClipboard` after the accepted case. | The harness prints the new original, the same processed text and translation, and `Status: cached`. The mock's request count stays at one. | Passed | Passed |
| Pause and resume (IN-02.2) | Type `pause` in the harness, set the clipboard to a fresh value, then type `resume` in the harness. | The paused value never reaches console output or the mock. The next fresh clipboard value is eligible. | Passed | Passed |

## Settings and optional key

In the test profile, merge this table into `%APPDATA%\Translation Aggregator\settings.toml`.
In portable mode the equivalent is `data\settings.toml` beside the executable.

```toml
[providers.libretranslate]
server = "127.0.0.1:5000"
# credential = "libre-key"
# auto_max_chars = 500
# cache = true
# rate_limit = { requests = 10, per_seconds = 60 }
# timeout_seconds = 15
```

Launch with `--clipboard --source ja --target en` and repeat the accepted input case. Supplying
`--libretranslate-server` explicitly uses a keyless test session with default capture settings and
in-memory history. To test with a key, uncomment
`credential`, then enter a disposable test key via the masked prompt:

```powershell
$key = Read-Host 'LibreTranslate key' -AsSecureString
([System.Net.NetworkCredential]::new('', $key)).Password |
    & ".\target\$target\debug\translation-aggregator.exe" --store-credential libre-key
Remove-Variable key
```

Use a mock only with a disposable key. It prints request bodies, including any key it receives.
If the named credential is missing, expect `Status: LibreTranslate needs key` and zero requests.
Remove the test credential in Windows Credential Manager after the run.

Set `cache = false` to send every eligible keyless request. Keyed requests bypass the cache
because their credential can rotate independently. The automatic limit counts Unicode scalar
values after substitutions and is bounded by the preset's request limit.

Clipboard and provider output share an `Entry` number. During a slow request the console keeps
accepting clipboard text and sends only the newest waiting eligible entry when the request
finishes. The older waiting entries show `skipped (newer clipboard input)`. A transient request
failure can trigger up to two retries, with `Retry-After` capped at the request timeout.
Newer eligible input cancels the older entry's retry wait.

## Clipboard filters and history

The harness accepts `pause`, `resume`, and `quit` as console commands. Opt-in history is written
to `history.json` in the data directory (`%LOCALAPPDATA%\Translation Aggregator`, or `data` in
portable mode). It contains the accepted original text, processed text and retained outcomes.
With `keep_between_sessions` set to false, the next launch using settings removes the snapshot
and backup. Use a dedicated test profile when exercising that setting.

```toml
[inputs.clipboard]
enabled = true
auto_translate = true
idle_delay_ms = 0
max_input_chars = 20000
japanese_only = false
suppress_repeats = true
detect_loops = false
phrase_extensions = false
phrase_repeat = false
repeat_characters = "none"
line_breaks = "keep"

[history]
keep_between_sessions = false
```

Set `auto_translate = false` to display and store accepted text without sending it. An empty
provider configuration also captures text and offers `Add a translator`. The character repeat
options are `none`, `preset_2`, `auto_constant`, `auto_variable`, and `all`. Line breaks support
`keep`, `remove`, and `keep_some` with `keep_first` and `keep_last` counts.

## Saved substitution profiles

Add profiles to `settings.toml` to reuse substitution rules when capturing clipboard text or
using `--text` or `--stdin`. `*` applies to every context. A matching context profile takes
precedence over the global profile for matches of the same length. Use `*` within a context
pattern to match multiple contexts.

```toml
[substitutions]
enabled = true

[[substitutions.profiles]]
context = "*"
rules = [{ match = '猫', replacement = 'Cat' }, { match = '犬', replacement = 'Dog' }]

[[substitutions.profiles]]
context = "clipboard"
rules = [{ match = '猫', replacement = 'Kitten' }]
```

With those settings, copying `猫と犬` produces `KittenとDog`. Typed input gives `CatとDog`.
Use an empty replacement to delete a match. Set `enabled = false` to turn substitutions off.
Use TOML literal strings for rules containing `\n`, `\t`, or `\\` so the substitution engine
receives the escapes. The explicit keyless `--libretranslate-server` session uses only rules
given on its command line.

Both Windows builds loaded the saved profiles from portable settings, including when the
console was started with a different working directory.

## Configured web providers

With no `--libretranslate-server` override, the console loads custom `.toml` definitions from
`%APPDATA%\Translation Aggregator\providers` or `data\providers` in portable mode.
Use unique provider IDs. The built-in preset uses `libretranslate`.
The [custom web services guide](../guides/custom-web-services.md) describes the definition format.

Set `enabled = true` and `auto_translate = true` on each loopback provider that should
translate new clipboard entries. Type `translate-all` to send the most recent captured entry
to every enabled provider, including those with automatic translation off. This command also
works while capture is paused. Off-device HTTPS providers can receive manually requested text.
Loopback providers can send automatically when enabled. Off-device providers need a fixed HTTPS
origin and explicit consent before automatic sends.

Start three local mocks and point three enabled definitions at them. Delay one mock and check
that all three receive the entry and the fast results appear while the slow request is active.
The `Provider` line ties each outcome to its provider ID. Each completion reports elapsed time
and cache status and stores the same fields in history. Pause capture and use
`translate-all` to verify that an enabled manual-only provider gets the entry and a disabled
provider gets no request. Both Windows builds passed these cases with the native clipboard.

## Automatic off-device translation

For PIP-05.2, use an interactive Windows terminal, a disposable profile and a test HTTPS endpoint
you control. Use synthetic clipboard text. Configure a custom provider with a fixed HTTPS host
and an enabled flag. Start with `auto_translate` omitted or false.

1. Start the console, copy a test line and confirm there is no prompt and no automatic request.
2. Set `auto_translate = true` and restart. Confirm the prompt names the provider and shows only
   its HTTPS origin. It must say clipboard text leaves the machine and requests may cost money.
   Press Enter to cancel, then copy a fresh test line. No automatic request should reach the endpoint.
3. Restart, type `yes`, wait for clipboard readiness and then copy a fresh test line. Confirm the
   expected request arrives. Restart again with the same configuration and confirm the console
   remembers approval without a new prompt.
4. Change the provider's HTTPS host or port and restart. Confirm the console asks again and sends
   nothing after cancellation. Set `auto_translate = false` and restart to remove that provider's
   approval. Switching the provider back on requires another confirmation.

Approval is stored in `remote-auto-consent.json` in the profile's data directory and applies to
the provider ID and the exact HTTPS origin. A new approval requires an interactive terminal. The
console validates the destination of each mapped request against the approval before sending.
Record the run on each Windows build. Native manual confirmation is pending.

## Acceptance evidence

| Criterion | Steps and evidence | Result |
| --- | --- | --- |
| PIP-02.3–5 | Exercise saved global, wildcard, and exact-context rules, an empty replacement, and the enable switch. Launch from a different working directory. | Passed on both Windows targets. |
| PIP-04.1–2 | Send one native clipboard entry to three loopback providers and hold one until its HTTP timeout. | Passed on both targets. The fast results were delivered while the slow request was still active. |
| PIP-04.3 | Check the provider ID, elapsed time, cache flag, and outcome on the console and associated history entry. | Passed for the M1 console and history on both targets. The local API output stream is M5. |
| PIP-04.4–5, PIP-05.4 | Run Translate All while capture is paused. Check that manual-only providers receive it, disabled providers receive nothing, and a fresh profile offers Add a translator. | Passed on both targets. |
| WEB-06.1 | Copy text with a fresh configuration and confirm the console offers Add a translator with zero requests. Set the server address and repeat the smoke test. | Passed on both targets with a fresh portable profile. |
| WEB-06.2 (keyed request) | Verify the built-in preset sends a configured key from Windows Credential Manager. The smoke test covers the keyless request. | Passed on both targets. The mock verified a disposable key from Credential Manager. A missing key made no request. |
| WEB-06.5 | Stop the mock. Start a reachable LibreTranslate instance on loopback with Japanese-to-English support. Start the harness against it and copy a Japanese sentence after readiness. Record the actual translation on both Windows targets. | Passed on both targets against LibreTranslate 1.9.6 on loopback. Input: 「今日はいい天気です。」. Console result: 「It is a good weather today。」. |
| IN-01.1, NFR-03.1 | Run the clipboard integration check on the reference Windows PC for both targets. Measure from notification receipt to Original output, excluding configured buffering time. The spec allows up to 100 ms. | Passed on both targets on the approved M1 PC. Maximum observed upper bound was 0.78 ms on 64-bit and 0.87 ms on 32-bit. |
| IN-02.1 | Verify the excluded clipboard value produces no history entry in the required integration test. | Passed on both targets. The retained snapshot contained only the accepted entry. |
| PIP-11.6 | Enable history, capture a line, wait for its result, and quit. Verify that reopening restores the snapshot and disabling retention removes it. Use a test profile. | Passed on both targets. Reopening restored the original entry and its outcome. Opt-out removed the snapshot and backup. |
| PIP-02.6, NFR-03.2 | Time a warm 500-character input with 10,000 substitution rules across five profiles on the reference Windows PC for both targets. Record timings against the 300 ms budget. Compare substitutions with observations from the running legacy app captured under [`fixtures/legacy/`](../../fixtures/legacy/README.md). | Passed on both targets on the approved M1 PC. Warm runs took 13.71 ms on 64-bit and 19.16 ms on 32-bit. Both matched the captured substitution outputs. |
| IN-06.4–5, IN-07.3, IN-08.1–6, IN-09.3 | Capture context-filter cases from a running legacy build and compare the rewrite's outputs against them. | [Legacy fixture replay](../../fixtures/legacy/context-filters/capture.md) passed on both targets. The clipboard console also replayed the single-input cases. Automatic romaji conversion has a separate M2 acceptance check. |

### Windows results on 2026-11-21

The tests used revision 95f9a1f with local working-tree edits on Windows 11 Pro build 26300,
an AMD Ryzen 7 5800X3D, and three displays. The primary display was 2560 x 1440 at 100% scale.
The mock and history checks used synthetic clipboard values. The settings and history cases
used disposable portable profiles.

| Measurement | 64-bit MSVC | 32-bit MSVC |
| --- | ---: | ---: |
| Maximum clipboard write start to console Original line receipt, five warm samples, zero idle delay | 0.78 ms | 0.87 ms |
| Warm release substitution scale test, 500 characters and 10,000 rules across five profiles | 13.71 ms | 19.16 ms |

The clipboard interval is an upper bound because it starts before the Windows notification and
ends after console output. On 2026-11-21, the owner approved this development PC for M1
performance sign-off. On both builds, native clipboard capture met the 100 ms budget and
the warm substitution run met the 300 ms budget.

The existing scale test prints the substitution time. Run it on the reference PC after a warm-up
run for each target and record the timings separately from the legacy fixture comparison.

```powershell
cargo test --release --locked -p ta-substitute --target x86_64-pc-windows-msvc substitution_scale -- --nocapture
cargo test --release --locked -p ta-substitute --target i686-pc-windows-msvc substitution_scale -- --nocapture
```

The loopback mock proves only the observed smoke-test cases. Record a Passed or Failed result in
the spec tracker when the full criterion has been checked with its specified verification method.
