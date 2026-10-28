# Native build verification

Version 0.3.4 was checked on Linux x86-64 with GTK 3.24.41 and WebKitGTK
2.52.6 (API 4.1).

- `cargo fmt --check`, Clippy across all targets with warnings denied, and all
  18 Rust tests pass.
- An independent process/accessibility check identifies the running Rust
  executable, native GtkWindow and GtkButton controls, and linked GTK/WebKitGTK
  libraries. No Firefox or libxul is loaded.
- Startup creates no WebView or web-renderer process. The sidebar occupies
  56 pixels and exposes accessible names for all five service buttons.
- A local HTML fixture verifies embedded document loading, inactive-view
  destruction, view recreation on wake, and an enabled WebKit sandbox.
- A 12-step local document fixture exercises WhatsApp single-pane navigation:
  a full-width list, opening a chat, Back, and reopening the same row at
  420 × 640; resizing the chat to 320 × 640; replacing its DOM as a single-page
  app would; restoring both panes at 1100 × 320 and 1100 × 740; and disabling
  and reenabling the adapter. Narrow panes fill the available width.
- Page zoom stays at 100% throughout. The fixture's computed composer font
  remains 18 CSS pixels. Back and reopening preserve the draft and the same
  chat DOM identity; the explicit DOM replacement changes that identity from
  1 to 2, and the adapter recovers. Wide mode and disabling remove the added
  Back button; compact chat mode has exactly one button after reenabling.
- These steps preserve all five WebViews and produce zero extra document
  loads. Repeated native evaluation preserves the adapter's isolated-world
  state without duplicating its Back button. This is a synthetic document,
  not an authenticated WhatsApp session.
- All five fixture documents stay loaded across 40 native service-button
  switches and 20 repeated current-service selections. View identities and actual
  WebKit load-start counts stay unchanged; repeated clicks do not move focus.
- After a real 35-second inactive period, all five documents remain intact.
  A separate expired-deadline check confirms ten-minute idle unloading, and
  superseded selections do not create an intermediate cold view.
- In one observed run, button signal to GTK visible-child selection took
  0.344 ms median, 0.728 ms at the 95th percentile, and 1.227 ms maximum. This
  measures native selection only, excluding screen presentation, provider
  startup, and network loading; it is not a provider-performance benchmark.
- Normal window close returns exit code zero. All observed app-owned renderer,
  network, and sandbox child processes exit; none remain after the checks.

The empty v0.3.4 app used approximately 19.2 MB proportional memory (PSS) and
64.4 MB resident memory (RSS) in this run. PSS varies as system libraries are
shared with other applications. These figures describe startup before opening
a service, not a memory ceiling; provider pages and engine workers add memory.
The release executable is 661,312 bytes, approximately 646 KiB.

Earlier v0.3.2 window/accessibility checks covered 1100 × 740, 640 × 480,
420 × 640, 320 × 240, 1100 × 320, and restored 1100 × 740 sizes. All seven
controls stayed inside the client area without overlap. That separate six-size
empty-window check was not rerun for v0.3.4. Earlier checks also activated the
native Telegram button and verified before-unload cancellation and confirmation;
these did not test provider authentication.

Cookie persistence and private website storage are configured through WebKit's
public APIs. Real-account login, storage across authenticated sessions, sending
messages, calls, media permissions, uploads, and downloads still require manual
testing with the providers. The WhatsApp adapter relies on undocumented provider
DOM structure; these local regressions do not verify its compatibility with a
current signed-in account. Embedded-engine sign-in restrictions may apply,
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
