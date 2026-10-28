# Native build verification

Version 0.3.5 was checked on Linux x86-64 with GTK 3.24.41 and WebKitGTK
2.52.6 (API 4.1).

- `cargo fmt --check`, Clippy across all targets with warnings denied, and all
  18 Rust tests pass.
- An independent process/accessibility check identifies the running Rust
  executable, native GtkWindow and GtkButton controls, and linked GTK/WebKitGTK
  libraries. No Firefox or libxul is loaded.
- Startup creates no WebView or web-renderer process. The sidebar occupies
  56 pixels and exposes accessible names for all five service buttons.
- Independent window/accessibility checks pass at 1100 × 740, 640 × 480,
  420 × 640, 320 × 240, 1100 × 320, and restored 1100 × 740 sizes. All visible
  controls stay inside the client area without overlap, with minimum
  44 × 44 targets. Compact mode has five equally allocated service tabs in a
  56-pixel bottom bar; Sleep and Quit are hidden there. Sidebar mode restores
  all seven controls. Content fills the remaining area at every size.
- A local HTML fixture verifies embedded document loading, inactive-view
  destruction, view recreation on wake, and an enabled WebKit sandbox.
- A 19-step local document fixture exercises WhatsApp single-pane navigation:
  a full-width list, opening a chat, Back, and reopening the same row at
  420 × 640; resizing the chat to 320 × 640; replacing its DOM as a single-page
  app would; restoring both panes at 1100 × 320 and 1100 × 740; and disabling
  and reenabling the adapter. Further steps check the navigation drawer and
  320 × 360 chat/list navigation. The fixture includes a desktop navigation
  rail, nested minimum widths, a crowded header, and a minimum-width composer.
  Narrow panes fill the available width.
- Page zoom stays at 100% throughout. The fixture's computed composer font
  remains 18 CSS pixels. Chat headers are 56 pixels high; their action controls
  and composer buttons stay at least 44 × 44 and inside the viewport. The
  composer remains fully visible, including at 320 × 360.
- Back and reopening preserve the draft and the same
  chat DOM identity; the explicit DOM replacement changes that identity from
  1 to 2, and the adapter recovers. Wide mode and disabling remove the added
  Back button; compact chat mode has exactly one button after reenabling.
- Back restores focus to the originating chat row and retains the fixture's
  30-pixel list scroll position. Reopening restores its 160-pixel conversation
  scroll position. The navigation button opens a 72-pixel drawer with a
  full-viewport scrim; dismissing it restores focus to the navigation button.
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
  0.254 ms median, 0.482 ms at the 95th percentile, and 0.883 ms maximum. This
  measures native selection only, excluding screen presentation, provider
  startup, and network loading; it is not a provider-performance benchmark.
- Normal window close returns exit code zero. All observed app-owned renderer,
  network, and sandbox child processes exit; none remain after the checks.

The empty v0.3.5 app used approximately 23.2 MB proportional memory (PSS) and
63.7 MB resident memory (RSS) in this run. PSS varies as system libraries are
shared with other applications. These figures describe startup before opening
a service, not a memory ceiling; provider pages and engine workers add memory.
The release executable is 681,944 bytes, approximately 666 KiB.

Earlier checks also activated the native Telegram button and verified
before-unload cancellation and confirmation; these did not test provider
authentication.

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
