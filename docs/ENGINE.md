# Native architecture

The visible process is the Rust executable. GTK provides its window, sidebar,
image buttons, context menus, dialogs, and file selectors. WebKitGTK supplies
an embedded native WebKitWebView for each opened service. The app never starts
Firefox, loads an extension, or renders its shell as an HTML page.

The shell uses a 56 px sidebar at 640 × 460 logical pixels and above. Narrower
or shorter windows use a 48 px bottom bar with 36 px buttons; all seven controls
fit down to the 320 × 240 minimum. Initial windows and sign-in popups fit the
monitor workarea. GTK handles display scaling. Resize notifications are
coalesced, and layout changes run only when the mode changes. The same widgets
and WebViews are kept without another navigation. Provider websites control
their own page layouts.

WhatsApp uses a single-pane layout at viewport widths of 748 CSS pixels or
less. The chat list or the selected chat occupies the available width, with
WebKit zoom fixed at 100% and normal text sizing. Clicking a chat row reveals
the existing chat pane; an app-provided back arrow returns to the existing
list. The document and composer remain mounted to preserve their state and
drafts. Wide windows restore both panes. The native **Single pane** item in the
WhatsApp logo menu controls this behavior and is enabled by default. Its
`compact_whatsapp` setting replaces the earlier `fit_whatsapp` preference,
which is ignored.

The adapter runs only on the WhatsApp origin in an isolated JavaScript world.
A no-op registered user script keeps that world alive between evaluations;
related sign-in popups receive only the no-op, not the pane adapter.
CSS uses app-owned attributes. A structural `MutationObserver` coalesces work
through `requestAnimationFrame` only while the narrow WhatsApp view is visible;
it pauses when the service is hidden, the window is wide, or the option is
disabled. There is no polling, additional WebView, daemon, or app IPC service.
The adapter relies on undocumented provider DOM hooks (`#side`, `#pane-side`,
`#main`, and the chat header). Unknown layouts keep the regular provider page;
future WhatsApp changes may require adapter updates. Signed-in WhatsApp
behavior remains unverified.

At startup no WebView or web-renderer process is created. A shared persistent
website-data manager and WebKit context are created when the first service is
opened. SQLite cookies, local storage, and website databases stay in the native
app's private profile. Cookies are explicitly configured for persistence.

Service views load on demand. Inactive views become eligible to close after
ten minutes; their native widgets are destroyed and recreated when reopened.
Switching to a loaded view reuses its widget and document, preserving state
without another navigation. Selecting the current app does no focus or loading work.
Their last HTTPS location and website storage are retained. Keep awake,
loading, audio, capture, authentication, popups, and unsaved-work protection can
prevent unloading. Automatic sleep cancels a site's before-unload confirmation;
manual sleep and quitting ask through a native dialog.

The resource cache uses WebKit's moderate document-browser mode; navigation page
cache remains disabled. User actions run at normal GLib priority, with a bounded
dispatch budget. Superseded selections and duplicate view notifications are
coalesced, preserving ordering around sleep and quit. Inactive views do not
repaint the sidebar, and existing idle deadlines are kept instead of repeatedly
recreating timers.
There is no periodic memory sampling, tray keepalive, service, autostart entry,
or scheduled job. WebKit's rendering/network/sandbox helpers exist only as
needed by the visible app. GTK receives normal window-close events and the app
closes its views and popups before exiting.

TLS checks and the WebKit sandbox remain enabled. Media and notification access
requires a native confirmation identifying the website origin. Downloads and
uploads use native file dialogs. Related native popup windows retain WebKit's
opener/authentication semantics. The app does not read private message bodies,
capture account passwords, or spoof a Chrome/Firefox user agent.

WebKitGTK provides native Linux embedding through its public GTK APIs.
Provider restrictions still govern login compatibility;
opening a service is not proof of authenticated messaging. Google explicitly
restricts embedded OAuth agents. No credentials or OAuth registrations are
bundled.

## Primary references

- [Isolated user scripts](https://webkitgtk.org/reference/webkit2gtk/stable/ctor.UserScript.new_for_world.html)
- [WebKitGTK WebView](https://webkitgtk.org/reference/webkit2gtk/stable/class.WebView.html)
- [Website data manager](https://webkitgtk.org/reference/webkit2gtk/stable/class.WebsiteDataManager.html)
- [Persistent cookies](https://webkitgtk.org/reference/webkit2gtk/stable/method.CookieManager.set_persistent_storage.html)
- [Cache model](https://webkitgtk.org/reference/webkit2gtk/stable/method.WebContext.set_cache_model.html)
- [Cache models](https://webkitgtk.org/reference/webkit2gtk/2.42.0/enum.CacheModel.html)
- [GLib source priorities](https://docs.gtk.org/glib/func.idle_add_full.html)
- [Before-unload close checks](https://webkitgtk.org/reference/webkit2gtk/stable/method.WebView.try_close.html)
- [WebKitGTK native dialog implementation](https://github.com/WebKit/WebKit/blob/webkitgtk-2.52.6/Source/WebKit/UIProcess/API/gtk/WebKitScriptDialogImpl.cpp)
- [Native GTK widgets](https://docs.gtk.org/gtk3/)
- [Google OAuth policy](https://developers.google.com/identity/protocols/oauth2/policies#use-secure-browsers)
