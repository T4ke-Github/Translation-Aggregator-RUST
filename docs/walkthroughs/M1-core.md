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
| Startup baseline | Wait for `Watching for clipboard changes (Ctrl+C to stop).`, then wait one second. | The existing `開始前` value produces zero requests. | Pending | Pending |
| Japanese-only filter | Type `Set-RejectedClipboard` and wait one second. | The mock receives zero requests. | Pending | Pending |
| Whitespace | Type `Set-WhitespaceClipboard` and wait one second. | The mock receives zero requests. | Pending | Pending |
| Exclusion format (IN-02.1) | Type `Set-ProtectedClipboard` and wait one second. | `ひみつ` is absent from harness output and the mock receives zero requests. | Pending | Pending |
| Clipboard to result (IN-01.1, WEB-06.2) | Type `Set-AcceptedClipboard`. | The harness prints context `clipboard`, original `こんにちは`, processed `こんばんは`, and `Good evening`. The mock receives one `POST /translate` with `q` equal to `こんばんは` (logged as Unicode escapes), `source` equal to `ja`, `target` equal to `en`, and no `api_key`. | Pending | Pending |
| Cache hit (PIP-08.1) | Type `Set-EquivalentClipboard` after the accepted case. | The harness prints the new original, the same processed text and translation, and `Status: cached`. The mock's request count stays at one. | Pending | Pending |
| Pause and resume (IN-02.2) | Type `pause` in the harness, set the clipboard to a fresh value, then type `resume` in the harness. | The paused value never reaches console output or the mock. The next fresh clipboard value is eligible. | Pending | Pending |

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

## Acceptance evidence still required

| Criterion | Steps and evidence | Result |
| --- | --- | --- |
| WEB-06.1 | Copy text with a fresh configuration and confirm the console offers Add a translator with zero requests. Set the server address and repeat the smoke test. | Pending |
| WEB-06.2 (keyed request) | Verify the built-in preset sends a configured key from Windows Credential Manager. The smoke test covers the keyless request. | Pending |
| WEB-06.5 | Stop the mock. Start a reachable LibreTranslate instance on loopback with Japanese-to-English support. Start the harness against it and copy a Japanese sentence after readiness. Record the actual translation on both Windows targets. | Pending |
| IN-01.1, NFR-03.1 | Run the clipboard integration check on the reference Windows PC for both targets. Measure from notification receipt to Original output, excluding configured buffering time. The spec allows up to 100 ms. | Pending |
| IN-02.1 | Verify the excluded clipboard value produces no history entry in the required integration test. | Pending |
| PIP-11.6 | Enable history, capture a line, wait for its result, and quit. Verify that reopening restores the snapshot and disabling retention removes it. Use a test profile. | Pending |
| PIP-02.6, NFR-03.2 | Time a warm 500-character input with 10,000 substitution rules across five profiles on the reference Windows PC for both targets. Record timings against the 300 ms budget. Compare substitutions with observations from the running legacy app captured under [`fixtures/legacy/`](../../fixtures/legacy/README.md). | Pending |
| IN-06.4–5, IN-07.3, IN-08.1–6, IN-09.3 | Capture context-filter cases from a running legacy build and compare the rewrite's outputs against them. | Pending |

The existing scale test prints the substitution time. Run it on the reference PC after a warm-up
run for each target and record the timings separately from the legacy fixture comparison.

```powershell
cargo test --release --locked -p ta-substitute --target x86_64-pc-windows-msvc substitution_scale -- --nocapture
cargo test --release --locked -p ta-substitute --target i686-pc-windows-msvc substitution_scale -- --nocapture
```

The loopback mock proves only the observed smoke-test cases. Record a Passed or Failed result in
the spec tracker when the full criterion has been checked with its specified verification method.
