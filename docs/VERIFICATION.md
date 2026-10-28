# Native build verification

Version 0.3.2 was checked on Linux x86-64 with GTK 3.24.41 and WebKitGTK
2.52.6 (API 4.1).

- `cargo fmt --check`, Clippy with warnings denied, and all 17 Rust tests pass.
- An independent process/accessibility check identifies the running Rust
  executable, native GtkWindow and GtkButton controls, and linked GTK/WebKitGTK
  libraries. No Firefox or libxul is loaded.
- Startup creates no WebView or web-renderer process. The sidebar occupies
  56 pixels and exposes accessible names for all five service buttons.
- Activating the native Telegram button starts the embedded renderer. This
  checks the service action, not account authentication.
- A local HTML fixture verifies embedded document loading, inactive-view
  destruction, view recreation on wake, and an enabled WebKit sandbox.
- Independent window/accessibility checks pass at 1100 × 740, 640 × 480,
  420 × 640, 320 × 240, 1100 × 320, and restored 1100 × 740 sizes. All seven
  controls remain inside the client area without overlap; content fills the
  remaining area. Captures use current window bounds after each resize.
- Four loaded-page resize transitions retain all five WebViews and produce no
  extra document loads. The active WebView receives the expected viewport size.
- All five fixture documents stay loaded across 40 native service-button
  switches and 20 repeated current-service selections. View identities and actual
  WebKit load-start counts stay unchanged; repeated clicks do not move focus.
- After a real 35-second inactive period, all five documents remain intact.
  A separate expired-deadline check confirms ten-minute idle unloading, and
  superseded selections do not create an intermediate cold view.
- In one observed run, button signal to GTK visible-child selection took
  0.21 ms median, 0.62 ms at the 95th percentile, and 0.93 ms maximum. This measures
  native selection only, excluding screen presentation, provider startup, and
  network loading; it is not a complete provider-performance benchmark.
- A separate runtime fixture verifies WebKit's before-unload response semantics:
  cancelling keeps the page open; confirming permits closing it.
- Normal window close returns exit code zero. All observed app-owned renderer,
  network, and sandbox child processes exit; none remain after the checks.

The empty app used approximately 44 MB proportional memory (PSS) and
61 MB resident memory (RSS) in the v0.3.1 check on this machine. PSS varies as
system libraries are shared with other applications. These figures describe
startup before opening a service, not a memory ceiling; provider pages and
engine workers add memory. The release executable is approximately 613 KiB.

Cookie persistence and private website storage are configured through WebKit's
public APIs. Real-account login, storage across authenticated sessions, sending
messages, calls, media permissions, uploads, and downloads still require manual
testing with the providers. Embedded-engine sign-in restrictions may apply,
particularly to Google account flows.

To repeat the independent native fixture check, run:

~~~sh
python3 tests/native_smoke.py --run-self-test
python3 tests/responsive_smoke.py
~~~

The QA tool launches the app with a separate temporary data directory, inspects
only its own window and process tree, saves a report and screenshot, then closes
it. It requires a graphical Linux session and the Python GTK/accessibility
bindings; these are test dependencies, not runtime dependencies of the app.
