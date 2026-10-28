# Native build verification

Version 0.3.1 was checked on Linux x86-64 with GTK 3.24.41 and WebKitGTK
2.52.6 (API 4.1).

- `cargo fmt --check`, Clippy with warnings denied, and all 14 Rust tests pass.
- An independent process/accessibility check identifies the running Rust
  executable, native GtkWindow and GtkButton controls, and linked GTK/WebKitGTK
  libraries. No Firefox or libxul is loaded.
- Startup creates no WebView or web-renderer process. The sidebar occupies
  56 pixels and exposes accessible names for all five service buttons.
- Activating the native Telegram button starts the embedded renderer. This
  checks the service action, not account authentication.
- A local HTML fixture verifies embedded document loading, inactive-view
  destruction, view recreation on wake, and an enabled WebKit sandbox.
- All five fixture documents stay loaded across 40 native service-button
  switches and 20 repeated current-service selections. View identities and actual
  WebKit load-start counts stay unchanged; repeated clicks do not move focus.
- After a real 35-second inactive period, all five documents remain intact.
  A separate expired-deadline check confirms ten-minute idle unloading, and
  superseded selections do not create an intermediate cold view.
- In one observed run, button signal to GTK visible-child selection took
  0.22 ms median, 0.28 ms at the 95th percentile, and 0.37 ms maximum. This measures
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
engine workers add memory. The release executable is approximately 608 KiB.

Cookie persistence and private website storage are configured through WebKit's
public APIs. Real-account login, storage across authenticated sessions, sending
messages, calls, media permissions, uploads, and downloads still require manual
testing with the providers. Embedded-engine sign-in restrictions may apply,
particularly to Google account flows.

To repeat the independent native fixture check, run:

~~~sh
python3 tests/native_smoke.py --run-self-test
~~~

The QA tool launches the app with a separate temporary data directory, inspects
only its own window and process tree, saves a report and screenshot, then closes
it. It requires a graphical Linux session and the Python GTK/accessibility
bindings; these are test dependencies, not runtime dependencies of the app.
