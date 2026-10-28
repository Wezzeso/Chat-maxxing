# Native build verification

Version 0.3.3 was checked on Linux x86-64 with GTK 3.24.41 and WebKitGTK
2.52.6 (API 4.1).

- `cargo fmt --check`, Clippy across all targets with warnings denied, and all
  25 Rust tests pass.
- An independent process/accessibility check identifies the running Rust
  executable, native GtkWindow and GtkButton controls, and linked GTK/WebKitGTK
  libraries. No Firefox or libxul is loaded.
- Startup creates no WebView or web-renderer process. The sidebar occupies
  56 pixels and exposes accessible names for all five service buttons.
- A local HTML fixture verifies embedded document loading, inactive-view
  destruction, view recreation on wake, and an enabled WebKit sandbox.
- A local document with a fixed minimum layout width exercises the WhatsApp
  fit path at 420 × 640, 320 × 240, 1100 × 320, and 1100 × 740 window sizes.
  Page zoom is 56.1% and 42.7% at the two narrow sizes, then returns to 100%
  at both wide sizes. Measured horizontal overflow stays within the four-CSS-pixel
  tolerance at all four sizes. This is a synthetic document, not an
  authenticated WhatsApp session.
- All four transitions preserve the fixture's draft and all five WebViews,
  produce zero extra document loads, and leave the other services at 100% zoom.
  The active WebView receives the expected content allocation.
- All five fixture documents stay loaded across 40 native service-button
  switches and 20 repeated current-service selections. View identities and actual
  WebKit load-start counts stay unchanged; repeated clicks do not move focus.
- After a real 35-second inactive period, all five documents remain intact.
  A separate expired-deadline check confirms ten-minute idle unloading, and
  superseded selections do not create an intermediate cold view.
- In one observed run, button signal to GTK visible-child selection took
  0.245 ms median, 0.372 ms at the 95th percentile, and 0.534 ms maximum. This
  measures native selection only, excluding screen presentation, provider
  startup, and network loading; it is not a provider-performance benchmark.
- Normal window close returns exit code zero. All observed app-owned renderer,
  network, and sandbox child processes exit; none remain after the checks.

The empty v0.3.3 app used approximately 18.6 MB proportional memory (PSS) and
64.5 MB resident memory (RSS) in this run. PSS varies as system libraries are
shared with other applications. These figures describe startup before opening
a service, not a memory ceiling; provider pages and engine workers add memory.
The release executable is 672,568 bytes, approximately 657 KiB.

Earlier v0.3.2 window/accessibility checks covered 1100 × 740, 640 × 480,
420 × 640, 320 × 240, 1100 × 320, and restored 1100 × 740 sizes. All seven
controls stayed inside the client area without overlap. That separate six-size
empty-window check was not rerun for v0.3.3. Earlier checks also activated the
native Telegram button and verified before-unload cancellation and confirmation;
these did not test provider authentication.

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
