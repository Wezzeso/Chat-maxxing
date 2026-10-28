# Changelog

## 0.3.3

- Fit WhatsApp's desktop layout into narrow windows using measured whole-page zoom; restore 100% in wide windows.
- Add a native **Fit width** toggle to the WhatsApp logo menu, enabled by default.
- Bound geometry probes to three per native size allocation, without polling, page reloads, user-agent changes, or provider styles.
- Verify view and draft preservation through resize transitions with a local fixture; real-account WhatsApp behavior remains unverified.

## 0.3.2

- Adapt the native sidebar into a compact bottom bar on narrow or short windows.
- Keep all seven controls visible down to 320 × 240 logical pixels.
- Fit the initial main window and sign-in popups to the monitor workarea.
- Preserve loaded WebViews and their state while resizing.
- Verify six empty-window layouts and four loaded-page resize transitions.

## 0.3.1

- Replace the 30-second unload window with a ten-minute grace period.
- Keep loaded service documents intact during switches; enable moderate resource caching.
- Dispatch sidebar actions promptly, coalesce superseded selections, and bound each batch.
- Avoid redundant visibility changes while selecting an already active service.
- Add switching-policy regression tests and automated Linux builds and releases.

## 0.3.0

- Replace the Firefox launcher with a native Rust application using GTK and WebKitGTK.
- Add a minimal dark sidebar with service logos, lazy loading, and safe idle sleep.
- Shut down app-owned engine workers when the window closes.

These are Linux previews. Real-account sign-in and messaging remain unverified;
providers may restrict embedded browsers.
