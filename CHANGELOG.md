# Changelog

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
