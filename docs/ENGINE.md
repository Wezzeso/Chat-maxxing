# Native architecture

The visible process is the Rust executable. GTK provides its window, sidebar,
image buttons, context menus, dialogs, and file selectors. WebKitGTK supplies
an embedded native WebKitWebView for each opened service. The app never starts
Firefox, loads an extension, or renders its shell as an HTML page.

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
