//! Native widgets and public WebKitGTK APIs. Every GUI operation runs on GTK's
//! main thread; callbacks queue actions so nested native dialogs cannot borrow
//! application state reentrantly. Each stored WebView has an explicit owned ref.
use crate::ffi::*;
use crate::layout::{self, Mode};
use crate::model::{should_sleep, IDLE_DELAY, SERVICES};
use crate::storage::{Config, InstanceLock, Storage};
use crate::Options;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};

macro_rules! c {
    ($s:literal) => {
        concat!($s, "\0").as_ptr().cast::<c_char>()
    };
}

const DARK: Rgba = Rgba {
    red: 0.082,
    green: 0.086,
    blue: 0.094,
    alpha: 1.0,
};
const CSS: &str = "
window#messages { background: #151618; color: #b7bbc3; }
#rail { background: #101113; border: 0; }
button.service, button.control { background: transparent; background-image: none;
 border: 0; border-radius: 6px; box-shadow: none; padding: 0; min-width: 44px; min-height: 44px; }
button.service:hover, button.control:hover { background: #23262b; }
 #rail.compact button.service, #rail.compact button.control {
 min-width: 44px; min-height: 44px; border-radius: 8px; }
button.service.selected { background: #292d33; }
button.service.kept { box-shadow: inset 0 -2px #bda669; }
button.control { color: #818894; }
label#empty { color: #686e79; font: 12px sans-serif; }
menu { background: #202328; color: #bdc2cb; }
";

// Applied only to the main WhatsApp view, in a separate JavaScript world.
const WHATSAPP_ADAPTER: &str = include_str!("whatsapp.js");

#[derive(Clone, Copy, PartialEq)]
enum CloseReason {
    Auto,
    Manual,
    Quit,
}

struct Slot {
    view: Ptr,
    uri: String,
    inactive: Option<Instant>,
    intent: Option<CloseReason>,
    blocked: bool,
    crashed: bool,
    creates: u64,
    loads: u64,
    focuses: u64,
}
impl Slot {
    fn new(index: usize) -> Self {
        Self {
            view: null_mut(),
            uri: SERVICES[index].url.into(),
            inactive: None,
            intent: None,
            blocked: false,
            crashed: false,
            creates: 0,
            loads: 0,
            focuses: 0,
        }
    }
}

enum Action {
    Open(usize),
    Sleep(usize, bool),
    SleepAll,
    Idle(usize),
    Changed(usize, Ptr),
    LoadStarted(usize, Ptr),
    Closed(usize, Ptr),
    Cancelled(usize, Ptr),
    Crash(usize, Ptr),
    Keep(usize, bool),
    Quit,
    Popup(Ptr, Ptr),
    PopupClosed(Ptr),
    PopupCancelled,
    QaTick,
    Resize(i32, i32),
    Adapt(usize, Ptr),
    QaPage(PageReply),
    Compact(bool),
}

struct PageProbe {
    view: Ptr,
    document: u64,
}
struct PageReply {
    probe: PageProbe,
    state: Option<Value>,
}

struct Bridge {
    app: RefCell<Option<App>>,
    queue: RefCell<VecDeque<Action>>,
    scheduled: Cell<bool>,
    busy: Cell<bool>,
    window: Cell<Ptr>,
    size: Cell<(i32, i32)>,
    timers: [Cell<u32>; 5],
    intents: [Cell<u8>; 5],
}
thread_local! {
    static BRIDGE: Bridge = Bridge {
        app: RefCell::new(None), queue: RefCell::new(VecDeque::new()),
        scheduled: Cell::new(false), busy: Cell::new(false), window: Cell::new(null_mut()),
        size: Cell::new((0, 0)),
        timers: std::array::from_fn(|_| Cell::new(0)),
        intents: std::array::from_fn(|_| Cell::new(0)),
    };
}

fn enqueue(action: Action) {
    BRIDGE.with(|bridge| {
        queue_action(&mut bridge.queue.borrow_mut(), action);
        schedule_dispatch(bridge);
    });
}

// Only combine selections/notifications within an ordering segment. Explicit
// sleep, close, permission, and quit actions remain barriers.
fn queue_action(queue: &mut VecDeque<Action>, action: Action) {
    let start = queue
        .iter()
        .rposition(|queued| {
            !matches!(
                queued,
                Action::Open(_) | Action::Changed(..) | Action::Resize(..) | Action::Adapt(..)
            )
        })
        .map_or(0, |index| index + 1);
    match &action {
        Action::Resize(width, height) => {
            if let Some(queued) = queue.iter_mut().skip(start).find(|queued| matches!(queued, Action::Resize(..))) {
                *queued = Action::Resize(*width, *height);
                return;
            }
        }
        Action::Open(_) => {
            let mut index = start;
            while index < queue.len() {
                if matches!(queue[index], Action::Open(_)) {
                    queue.remove(index);
                } else {
                    index += 1;
                }
            }
        }
        Action::Changed(index, view)
            if queue.iter().skip(start).any(|queued| {
                matches!(queued, Action::Changed(other, pointer) if other == index && pointer == view)
            }) =>
        {
            return;
        }
        Action::Adapt(index, view)
            if queue.iter().skip(start).any(|queued| {
                matches!(queued, Action::Adapt(other, pointer) if other == index && pointer == view)
            }) =>
        {
            return;
        }
        _ => {}
    }
    queue.push_back(action);
}

fn schedule_dispatch(bridge: &Bridge) {
    if !bridge.busy.get() && !bridge.scheduled.replace(true) {
        unsafe {
            g_idle_add_full(G_PRIORITY_DEFAULT, Some(dispatch), null_mut(), None);
        }
    }
}
unsafe extern "C" fn dispatch(_: Ptr) -> Bool {
    BRIDGE.with(|bridge| {
        bridge.scheduled.set(false);
        if bridge.busy.replace(true) {
            return;
        }
        let started = Instant::now();
        for _ in 0..16 {
            let action = bridge.queue.borrow_mut().pop_front();
            let Some(action) = action else { break };
            if let Some(app) = bridge.app.borrow_mut().as_mut() {
                app.handle(action);
            }
            if started.elapsed() >= Duration::from_millis(2) {
                break;
            }
        }
        bridge.busy.set(false);
        if !bridge.queue.borrow().is_empty() {
            schedule_dispatch(bridge);
        }
    });
    G_SOURCE_REMOVE
}

fn cs(value: &str) -> CString {
    CString::new(value.replace('\0', "")).expect("NUL removed")
}
fn path_cs(path: &Path) -> CString {
    CString::new(path.as_os_str().as_bytes()).expect("Filesystem path cannot contain NUL")
}
unsafe fn text(pointer: *const c_char) -> String {
    if pointer.is_null() {
        String::new()
    } else {
        CStr::from_ptr(pointer).to_string_lossy().into_owned()
    }
}
unsafe fn connect(object: Ptr, signal: *const c_char, callback: *const (), data: usize) {
    let callback = std::mem::transmute::<*const (), unsafe extern "C" fn()>(callback);
    g_signal_connect_data(object, signal, Some(callback), data as Ptr, None, 0);
}
fn cancel_timer(index: usize) {
    BRIDGE.with(|bridge| {
        let source = bridge.timers[index].replace(0);
        if source != 0 {
            unsafe {
                g_source_remove(source);
            }
        }
    });
}
unsafe extern "C" fn idle_due(data: Ptr) -> Bool {
    let index = data as usize;
    BRIDGE.with(|bridge| bridge.timers[index].set(0));
    enqueue(Action::Idle(index));
    G_SOURCE_REMOVE
}

struct App {
    window: Ptr,
    row: Ptr,
    controls: [Ptr; 2],
    spacer: Ptr,
    layout: Mode,
    rail: Ptr,
    stack: Ptr,
    empty: Ptr,
    buttons: [Ptr; 5],
    slots: [Slot; 5],
    active: Option<usize>,
    manager: Ptr,
    context: Ptr,
    popups: Vec<(Ptr, Ptr)>,
    storage: Storage,
    config: Config,
    _lock: InstanceLock,
    quitting: bool,
    destroyed: bool,
    test_mode: bool,
    qa: Option<Qa>,
    title: String,
}

impl App {
    unsafe fn new(storage: Storage, lock: InstanceLock, config: Config, options: &Options) -> Self {
        let settings = gtk_settings_get_default();
        g_object_set(
            settings,
            c!("gtk-application-prefer-dark-theme"),
            TRUE,
            c!("gtk-enable-animations"),
            FALSE,
            null::<c_char>(),
        );
        let provider = gtk_css_provider_new();
        let css = cs(CSS);
        let mut error = null_mut();
        if gtk_css_provider_load_from_data(provider, css.as_ptr(), -1, &mut error) == FALSE {
            eprintln!(
                "Native style: {}",
                if error.is_null() {
                    "could not load".into()
                } else {
                    text((*error).message)
                }
            );
            if !error.is_null() {
                g_error_free(error);
            }
        }
        gtk_style_context_add_provider_for_screen(
            gdk_screen_get_default(),
            provider,
            GTK_STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        g_object_unref(provider);

        let window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
        g_object_ref_sink(window);
        gtk_widget_set_name(window, c!("messages"));
        gtk_window_set_title(window, c!("Messages"));
        gtk_window_set_wmclass(window, c!("relay-native"), c!("RelayNative"));
        let initial = window_size((1100, 740), null_mut());
        gtk_window_set_default_size(window, initial.0, initial.1);
        gtk_widget_set_size_request(window, layout::MIN_WIDTH, layout::MIN_HEIGHT);
        gtk_window_set_position(window, GTK_WIN_POS_CENTER);
        let row = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 0);
        gtk_container_add(window, row);
        let rail = gtk_box_new(GTK_ORIENTATION_VERTICAL, 6);
        gtk_widget_set_name(rail, c!("rail"));
        gtk_container_set_border_width(rail, 6);
        gtk_widget_set_size_request(rail, 56, -1);
        gtk_box_pack_start(row, rail, FALSE, TRUE, 0);
        let stack = gtk_stack_new();
        gtk_stack_set_homogeneous(stack, FALSE);
        atk_object_set_name(gtk_widget_get_accessible(stack), c!("Content"));
        gtk_stack_set_transition_type(stack, 0);
        gtk_widget_set_hexpand(stack, TRUE);
        gtk_widget_set_vexpand(stack, TRUE);
        gtk_box_pack_start(row, stack, TRUE, TRUE, 0);
        let empty = gtk_label_new(c!("Select an app"));
        gtk_widget_set_name(empty, c!("empty"));
        gtk_stack_add_named(stack, empty, c!("empty"));
        let buttons = std::array::from_fn(|index| {
            let button = gtk_button_new();
            gtk_button_set_relief(button, GTK_RELIEF_NONE);
            gtk_widget_set_size_request(button, 44, 44);
            gtk_style_context_add_class(gtk_widget_get_style_context(button), c!("service"));
            gtk_widget_set_tooltip_text(button, cs(SERVICES[index].name).as_ptr());
            gtk_widget_set_name(button, cs(SERVICES[index].id).as_ptr());
            atk_object_set_name(
                gtk_widget_get_accessible(button),
                cs(SERVICES[index].name).as_ptr(),
            );
            let file = path_cs(
                &storage
                    .icons_dir
                    .join(format!("{}.svg", SERVICES[index].id)),
            );
            let pixbuf = gdk_pixbuf_new_from_file_at_scale(file.as_ptr(), 24, 24, TRUE, null_mut());
            let image = gtk_image_new_from_pixbuf(pixbuf);
            if !pixbuf.is_null() {
                g_object_unref(pixbuf);
            }
            gtk_button_set_image(button, image);
            gtk_button_set_always_show_image(button, TRUE);
            gtk_box_pack_start(rail, button, FALSE, FALSE, 0);
            connect(button, c!("clicked"), service_clicked as *const (), index);
            connect(
                button,
                c!("button-press-event"),
                service_menu as *const (),
                index,
            );
            button
        });
        let spacer = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
        gtk_box_pack_start(rail, spacer, TRUE, TRUE, 0);
        let mut controls = [null_mut(); 2];
        for (index, (name, label, callback)) in [
            (
                "weather-clear-night-symbolic",
                "Sleep apps",
                sleep_clicked as *const (),
            ),
            (
                "system-shutdown-symbolic",
                "Quit",
                quit_clicked as *const (),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let button = gtk_button_new();
            gtk_button_set_relief(button, GTK_RELIEF_NONE);
            gtk_style_context_add_class(gtk_widget_get_style_context(button), c!("control"));
            gtk_widget_set_size_request(button, 44, 44);
            gtk_widget_set_tooltip_text(button, cs(label).as_ptr());
            atk_object_set_name(gtk_widget_get_accessible(button), cs(label).as_ptr());
            gtk_button_set_image(button, gtk_image_new_from_icon_name(cs(name).as_ptr(), 4));
            gtk_button_set_always_show_image(button, TRUE);
            gtk_box_pack_start(rail, button, FALSE, FALSE, 0);
            connect(button, c!("clicked"), callback, 0);
            controls[index] = button;
        }
        connect(window, c!("size-allocate"), size_allocated as *const (), 0);
        connect(window, c!("delete-event"), delete_window as *const (), 0);
        connect(window, c!("destroy"), destroyed_window as *const (), 0);
        connect(window, c!("key-press-event"), key_pressed as *const (), 0);
        BRIDGE.with(|bridge| bridge.window.set(window));
        let mut app = Self {
            window,
            row,
            controls,
            spacer,
            layout: Mode::Sidebar,
            rail,
            stack,
            empty,
            buttons,
            slots: std::array::from_fn(Slot::new),
            active: None,
            manager: null_mut(),
            context: null_mut(),
            popups: Vec::new(),
            storage,
            config,
            _lock: lock,
            quitting: false,
            destroyed: false,
            test_mode: options.qa_report.is_some(),
            qa: options
                .qa_report
                .as_ref()
                .map(|report| Qa::new(report.clone(), options.snapshot.clone())),
            title: "Messages".into(),
        };
        app.reflow(initial.0, initial.1);
        app
    }

    unsafe fn reflow(&mut self, width: i32, height: i32) {
        let mode = Mode::for_size(width, height);
        if self.layout == mode {
            return;
        }
        self.layout = mode;
        let compact = mode == Mode::BottomBar;
        let style = gtk_widget_get_style_context(self.rail);
        if compact {
            gtk_style_context_add_class(style, c!("compact"));
        } else {
            gtk_style_context_remove_class(style, c!("compact"));
        }
        gtk_orientable_set_orientation(
            self.row,
            if compact {
                GTK_ORIENTATION_VERTICAL
            } else {
                GTK_ORIENTATION_HORIZONTAL
            },
        );
        gtk_orientable_set_orientation(
            self.rail,
            if compact {
                GTK_ORIENTATION_HORIZONTAL
            } else {
                GTK_ORIENTATION_VERTICAL
            },
        );
        gtk_box_set_spacing(self.rail, if compact { 0 } else { 6 });
        gtk_widget_set_size_request(
            self.rail,
            if compact { -1 } else { 56 },
            if compact { layout::BOTTOM_HEIGHT } else { -1 },
        );
        for button in &self.buttons {
            gtk_widget_set_size_request(*button, 44, 44);
            gtk_box_set_child_packing(self.rail, *button, compact as Bool, TRUE, 0, 0);
        }
        for widget in self.controls.iter().chain(std::iter::once(&self.spacer)) {
            gtk_widget_set_no_show_all(*widget, compact as Bool);
            if compact {
                gtk_widget_hide(*widget);
            } else {
                gtk_widget_show(*widget);
            }
        }
        gtk_box_reorder_child(self.row, self.rail, if compact { -1 } else { 0 });
    }

    unsafe fn ensure_context(&mut self) {
        if !self.context.is_null() {
            return;
        }
        let data = path_cs(&self.storage.data_dir.join("website-data"));
        let cache = path_cs(&self.storage.cache_dir.join("webkit"));
        self.manager = webkit_website_data_manager_new(
            c!("base-data-directory"),
            data.as_ptr(),
            c!("base-cache-directory"),
            cache.as_ptr(),
            null::<c_char>(),
        );
        self.context = webkit_web_context_new_with_website_data_manager(self.manager);
        webkit_web_context_set_sandbox_enabled(self.context, TRUE);
        webkit_web_context_set_cache_model(self.context, WEBKIT_CACHE_MODEL_DOCUMENT_BROWSER);
        let cookies = webkit_website_data_manager_get_cookie_manager(self.manager);
        let file = path_cs(&self.storage.data_dir.join("cookies.sqlite"));
        webkit_cookie_manager_set_persistent_storage(
            cookies,
            file.as_ptr(),
            WEBKIT_COOKIE_PERSISTENT_STORAGE_SQLITE,
        );
        connect(
            self.context,
            c!("download-started"),
            download_started as *const (),
            0,
        );
    }

    unsafe fn create_view(&mut self, index: usize) -> Ptr {
        self.ensure_context();
        let view = webkit_web_view_new_with_context(self.context);
        g_object_ref_sink(view);
        configure_view(view);
        if index == 0 {
            // Keep the named world alive between evaluations. A no-op anchor
            // cannot adapt related sign-in popups that share this manager.
            let allowed = [c!("https://web.whatsapp.com/*"), null()];
            let anchor = webkit_user_script_new_for_world(
                c!("void 0;"),
                1,
                1,
                c!("relay-whatsapp"),
                allowed.as_ptr(),
                null(),
            );
            webkit_user_content_manager_add_script(
                webkit_web_view_get_user_content_manager(view),
                anchor,
            );
            webkit_user_script_unref(anchor);
        }
        connect(view, c!("load-changed"), load_changed as *const (), index);
        connect(view, c!("notify::uri"), view_changed as *const (), index);
        connect(
            view,
            c!("notify::is-loading"),
            view_changed as *const (),
            index,
        );
        connect(
            view,
            c!("notify::is-playing-audio"),
            view_changed as *const (),
            index,
        );
        connect(
            view,
            c!("notify::camera-capture-state"),
            view_changed as *const (),
            index,
        );
        connect(
            view,
            c!("notify::microphone-capture-state"),
            view_changed as *const (),
            index,
        );
        connect(
            view,
            c!("notify::display-capture-state"),
            view_changed as *const (),
            index,
        );
        connect(view, c!("close"), view_closed as *const (), index);
        connect(view, c!("script-dialog"), script_dialog as *const (), index);
        connect(
            view,
            c!("web-process-terminated"),
            process_terminated as *const (),
            index,
        );
        gtk_stack_add_named(self.stack, view, cs(SERVICES[index].id).as_ptr());
        gtk_widget_show(view);
        self.slots[index].view = view;
        self.slots[index].creates += 1;
        view
    }

    unsafe fn open(&mut self, index: usize) {
        if self.quitting {
            return;
        }
        if self.active == Some(index)
            && !self.slots[index].view.is_null()
            && !self.slots[index].crashed
        {
            return;
        }
        let previous = self.active.filter(|old| *old != index);
        if let Some(old) = previous {
            self.slots[old].inactive = Some(Instant::now());
        }
        self.active = Some(index);
        cancel_timer(index);
        self.slots[index].intent = None;
        BRIDGE.with(|bridge| bridge.intents[index].set(0));
        self.slots[index].blocked = false;
        if self.slots[index].crashed {
            self.release_view(index);
        }
        if self.slots[index].view.is_null() {
            let view = self.create_view(index);
            if self.test_mode {
                let (html, base) = if index == 0 {
                    (
                        include_str!("../tests/fixtures/whatsapp.html").to_owned(),
                        "https://web.whatsapp.com/relay-fixture/",
                    )
                } else {
                    (format!("<!doctype html><meta name='color-scheme' content='dark'><title>Fixture {index}</title><body style='background:#151618;color:#bbc0c8'>Native embedded WebKit fixture {index}<script>localStorage.setItem('relay-test','persisted');</script>"),
                    "https://relay-fixture.invalid/")
                };
                webkit_web_view_load_html(view, cs(&html).as_ptr(), cs(base).as_ptr());
            } else {
                let uri = cs(&self.slots[index].uri);
                webkit_web_view_load_uri(view, uri.as_ptr());
            }
        }
        self.active = Some(index);
        gtk_stack_set_visible_child(self.stack, self.slots[index].view);
        gtk_widget_grab_focus(self.slots[index].view);
        self.slots[index].focuses += 1;
        if let Some(old) = previous {
            self.schedule(old);
        }
        self.paint();
        if index == 0 {
            enqueue(Action::Adapt(index, self.slots[index].view));
        }
    }

    unsafe fn adapt_whatsapp(&self, index: usize, view: Ptr) {
        if index != 0 || self.quitting || view.is_null() || self.slots[index].view != view {
            return;
        }
        // Never reduce page or text zoom. The adapter only changes pane layout.
        if (webkit_web_view_get_zoom_level(view) - 1.0).abs() > 0.001 {
            webkit_web_view_set_zoom_level(view, 1.0);
        }
        if webkit_web_view_is_loading(view) != FALSE
            || !SERVICES[0].owns_uri(&text(webkit_web_view_get_uri(view)))
        {
            return;
        }
        let script = format!(
            "{WHATSAPP_ADAPTER}; globalThis.relayWhatsApp?.enable({}); null;",
            self.config.compact_whatsapp
        );
        self.evaluate(view, &script, Some(evaluated), null_mut());
    }

    unsafe fn evaluate(&self, view: Ptr, script: &str, callback: GAsyncReadyCallback, data: Ptr) {
        webkit_web_view_evaluate_javascript(
            view,
            cs(script).as_ptr(),
            -1,
            c!("relay-whatsapp"),
            null(),
            null_mut(),
            callback,
            data,
        );
    }

    // Only the local self-test reads its own fixture field and identities.
    unsafe fn qa_page(&self, qa: &mut Qa) {
        if qa.page_pending || !self.test_mode {
            return;
        }
        qa.page_pending = true;
        let probe = Box::new(PageProbe {
            view: self.slots[0].view,
            document: self.slots[0].loads,
        });
        self.evaluate(probe.view, r#"(() => {
            const box = id => {
                const node = document.getElementById(id);
                if (!node) return {visible:false,width:0,x:0};
                const rect = node.getBoundingClientRect();
                const visible = rect.width > 0 && rect.height > 0 && getComputedStyle(node).visibility !== 'hidden';
                return {visible,width:rect.width,height:rect.height,x:rect.x,y:rect.y,right:rect.right,bottom:rect.bottom,scroll:node.scrollTop};
            };
            const input = document.getElementById('relay-fixture-draft');
            return JSON.stringify({viewport:innerWidth,rootWidth:document.documentElement.scrollWidth,
                list:box('side'),chat:box('main'),back:box('relay-wa-back'),
                backCount:document.querySelectorAll('#relay-wa-back').length,
                header:box('relay-chat-header'),composer:box('relay-composer'),messages:box('relay-message-scroll'),
                listScroll:document.getElementById('pane-side').scrollTop,
                focused:document.activeElement?.id,
                controls:[...document.querySelectorAll('#relay-chat-header button,#relay-composer button')].map(node=>{
                    const rect=node.getBoundingClientRect();return {x:rect.x,y:rect.y,width:rect.width,height:rect.height,right:rect.right,bottom:rect.bottom};}),
                navigation:box('relay-desktop-rail'),navigationButton:box('relay-wa-navigation'),scrim:box('relay-wa-nav-scrim'),
                font:input?parseFloat(getComputedStyle(input).fontSize):null,
                draft:input?.value,identity:document.getElementById('main')?.dataset.fixtureIdentity,
                adapter:globalThis.relayWhatsApp?.status()});
        })()"#, Some(page_measured), Box::into_raw(probe).cast());
    }

    unsafe fn paint(&mut self) {
        for index in 0..5 {
            let style = gtk_widget_get_style_context(self.buttons[index]);
            if self.active == Some(index) {
                gtk_style_context_add_class(style, c!("selected"));
            } else {
                gtk_style_context_remove_class(style, c!("selected"));
            }
            if self.config.keep_awake[index] {
                gtk_style_context_add_class(style, c!("kept"));
            } else {
                gtk_style_context_remove_class(style, c!("kept"));
            }
        }
        self.paint_title();
    }

    unsafe fn paint_title(&mut self) {
        let title = if let Some(index) = self.active {
            let uri = text(webkit_web_view_get_uri(self.slots[index].view));
            let origin = url::Url::parse(&uri)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned))
                .unwrap_or_else(|| SERVICES[index].name.into());
            if SERVICES[index].is_auth_uri(&uri) {
                format!("Sign in — {origin}")
            } else {
                format!("Messages — {origin}")
            }
        } else {
            "Messages".into()
        };
        if self.title != title {
            gtk_window_set_title(self.window, cs(&title).as_ptr());
            self.title = title;
        }
    }

    unsafe fn protected(&self, index: usize) -> bool {
        let slot = &self.slots[index];
        if slot.view.is_null() {
            return true;
        }
        let uri = text(webkit_web_view_get_uri(slot.view));
        webkit_web_view_is_loading(slot.view) != FALSE
            || webkit_web_view_is_playing_audio(slot.view) != FALSE
            || capturing(slot.view)
            || !self.test_mode && !SERVICES[index].owns_uri(&uri)
            || !self.popups.is_empty()
    }

    unsafe fn schedule(&mut self, index: usize) {
        if self.active == Some(index)
            || self.config.keep_awake[index]
            || self.slots[index].blocked
            || self.slots[index].view.is_null()
            || self.protected(index)
        {
            cancel_timer(index);
            return;
        }
        if BRIDGE.with(|bridge| bridge.timers[index].get() != 0) {
            return;
        }
        let Some(since) = self.slots[index].inactive else {
            return;
        };
        let remaining = IDLE_DELAY.saturating_sub(since.elapsed());
        let seconds = remaining
            .as_secs()
            .saturating_add(u64::from(remaining.subsec_nanos() != 0))
            .clamp(1, u32::MAX as u64) as u32;
        BRIDGE.with(|bridge| {
            bridge.timers[index].set(g_timeout_add_seconds(seconds, Some(idle_due), index as Ptr));
        });
    }

    unsafe fn request_sleep(&mut self, index: usize, manual: bool) {
        let view = self.slots[index].view;
        if view.is_null() || self.config.keep_awake[index] || self.protected(index) {
            return;
        }
        if self.active == Some(index) {
            if !manual {
                return;
            }
            self.active = None;
            if !self.destroyed {
                gtk_stack_set_visible_child(self.stack, self.empty);
            }
            self.paint();
        }
        self.slots[index].intent = Some(if manual {
            CloseReason::Manual
        } else {
            CloseReason::Auto
        });
        BRIDGE.with(|bridge| bridge.intents[index].set(if manual { 1 } else { 0 }));
        cancel_timer(index);
        webkit_web_view_try_close(view);
    }

    unsafe fn release_view(&mut self, index: usize) {
        cancel_timer(index);
        let slot = &mut self.slots[index];
        let view = slot.view;
        if view.is_null() {
            return;
        }
        let uri = text(webkit_web_view_get_uri(view));
        if SERVICES[index].is_allowed_navigation(&uri) {
            slot.uri = uri;
        }
        slot.view = null_mut();
        slot.intent = None;
        BRIDGE.with(|bridge| bridge.intents[index].set(0));
        slot.crashed = false;
        gtk_widget_destroy(view);
        g_object_unref(view);
        if self.active == Some(index) {
            self.active = None;
            if !self.destroyed {
                gtk_stack_set_visible_child(self.stack, self.empty);
            }
        }
    }

    unsafe fn quit_next(&mut self) {
        if let Some((_, view)) = self.popups.first() {
            webkit_web_view_try_close(*view);
        } else if let Some(index) = self.slots.iter().position(|slot| !slot.view.is_null()) {
            self.slots[index].intent = Some(CloseReason::Quit);
            BRIDGE.with(|bridge| bridge.intents[index].set(2));
            webkit_web_view_try_close(self.slots[index].view);
        } else {
            for (window, _) in self.popups.drain(..) {
                gtk_widget_destroy(window);
                g_object_unref(window);
            }
            self.destroyed = true;
            gtk_widget_destroy(self.window);
        }
    }

    unsafe fn handle(&mut self, action: Action) {
        if self.destroyed {
            return;
        }
        match action {
            Action::Open(index) => {
                self.open(index);
                if let Some(qa) = &mut self.qa {
                    if let Some((target, requested)) = qa.pending_switch.take() {
                        if target == index {
                            qa.switch_ms
                                .push(requested.elapsed().as_secs_f64() * 1000.0);
                        } else {
                            qa.errors.push("Wrong switch target dispatched".into());
                        }
                    }
                }
            }
            Action::Sleep(index, manual) => self.request_sleep(index, manual),
            Action::SleepAll => {
                for index in 0..5 {
                    self.request_sleep(index, true);
                }
            }
            Action::Idle(index) => {
                let slot = &self.slots[index];
                if !slot.view.is_null()
                    && !slot.blocked
                    && should_sleep(
                        self.active == Some(index),
                        self.config.keep_awake[index],
                        webkit_web_view_is_loading(slot.view) != FALSE,
                        webkit_web_view_is_playing_audio(slot.view) != FALSE,
                        capturing(slot.view),
                        !self.test_mode
                            && !SERVICES[index].owns_uri(&text(webkit_web_view_get_uri(slot.view))),
                        slot.inactive.map(|at| at.elapsed()).unwrap_or_default(),
                    )
                {
                    self.request_sleep(index, false);
                } else {
                    self.schedule(index);
                }
            }
            Action::Changed(index, view) => {
                if self.slots[index].view == view && !view.is_null() {
                    let uri = text(webkit_web_view_get_uri(self.slots[index].view));
                    if SERVICES[index].is_allowed_navigation(&uri) {
                        self.slots[index].uri = uri;
                    }
                    self.schedule(index);
                    if self.active == Some(index) {
                        self.paint_title();
                    }
                }
            }
            Action::LoadStarted(index, view) => {
                if self.slots[index].view == view {
                    self.slots[index].loads += 1;
                }
            }
            Action::Closed(index, view) => {
                if self.slots[index].view != view {
                    return;
                }
                let reason = self.slots[index].intent;
                if reason != Some(CloseReason::Quit) && self.active == Some(index) {
                    self.slots[index].intent = None;
                } else {
                    self.release_view(index);
                    self.paint();
                }
                if self.quitting {
                    self.quit_next();
                }
            }
            Action::Cancelled(index, view) => {
                if self.slots[index].view != view {
                    return;
                }
                self.slots[index].intent = None;
                BRIDGE.with(|bridge| bridge.intents[index].set(0));
                self.slots[index].blocked = true;
                self.quitting = false;
            }
            Action::Crash(index, view) => {
                if self.slots[index].view != view {
                    return;
                }
                self.slots[index].crashed = true;
                if self.quitting {
                    self.release_view(index);
                    self.quit_next();
                }
            }
            Action::Keep(index, value) => {
                self.config.keep_awake[index] = value;
                if let Err(error) = self.storage.save_config(&self.config) {
                    eprintln!("Preferences: {error}");
                }
                self.schedule(index);
                self.paint();
            }
            Action::Quit => {
                if !self.quitting {
                    self.quitting = true;
                    for index in 0..5 {
                        cancel_timer(index);
                    }
                    self.quit_next();
                }
            }
            Action::Popup(window, view) => self.popups.push((window, view)),
            Action::PopupClosed(window) => {
                if let Some(index) = self
                    .popups
                    .iter()
                    .position(|(existing, _)| *existing == window)
                {
                    self.popups.swap_remove(index);
                    g_object_unref(window);
                    if self.quitting {
                        self.quit_next();
                    } else {
                        for slot in 0..5 {
                            self.schedule(slot);
                        }
                    }
                }
            }
            Action::PopupCancelled => self.quitting = false,
            Action::QaTick => self.qa_tick(),
            Action::Resize(width, height) => self.reflow(width, height),
            Action::Adapt(index, view) => self.adapt_whatsapp(index, view),
            Action::QaPage(reply) => {
                if self.test_mode
                    && self.slots[0].view == reply.probe.view
                    && self.slots[0].loads == reply.probe.document
                {
                    if let Some(qa) = &mut self.qa {
                        qa.page_pending = false;
                        qa.page_state = reply.state;
                    }
                }
            }
            Action::Compact(value) => {
                self.config.compact_whatsapp = value;
                if let Err(error) = self.storage.save_config(&self.config) {
                    eprintln!("Preferences: {error}");
                }
                enqueue(Action::Adapt(0, self.slots[0].view));
            }
        }
    }

    unsafe fn snapshot(&self, path: &Path) -> Result<(), String> {
        let window = gtk_widget_get_window(self.window);
        let pixbuf = gdk_pixbuf_get_from_window(
            window,
            0,
            0,
            gdk_window_get_width(window),
            gdk_window_get_height(window),
        );
        if pixbuf.is_null() {
            return Err("Native window snapshot unavailable".into());
        }
        let mut error = null_mut();
        let result = gdk_pixbuf_savev(
            pixbuf,
            path_cs(path).as_ptr(),
            c!("png"),
            null_mut(),
            null_mut(),
            &mut error,
        );
        g_object_unref(pixbuf);
        if result == FALSE {
            let message = if error.is_null() {
                "PNG save failed".into()
            } else {
                text((*error).message)
            };
            if !error.is_null() {
                g_error_free(error);
            }
            return Err(message);
        }
        Ok(())
    }

    unsafe fn qa_tick(&mut self) {
        let Some(mut qa) = self.qa.take() else { return };
        if qa.started.elapsed() > Duration::from_secs(100) {
            qa.errors.push("Native self-test timed out".into());
            qa.stage = 5;
        }
        match qa.stage {
            0 => {
                qa.results.push(json!({"initialWebViews":self.slots.iter().filter(|slot|!slot.view.is_null()).count(),
                    "railWidth":gtk_widget_get_allocated_width(self.rail)+12,
                    "windowWidget":text(g_type_name_from_instance(self.window)),
                    "sidebarWidget":text(g_type_name_from_instance(self.rail)),
                    "buttonWidget":text(g_type_name_from_instance(self.buttons[0])),
                    "native":true,"engine":"WebKitGTK","firefox":false}));
                if let Some(path) = &qa.snapshot {
                    if let Err(error) = self.snapshot(path) {
                        qa.errors.push(error);
                    }
                }
                self.open(0);
                qa.stage = 1;
            }
            1 if !self.slots[0].view.is_null()
                && webkit_web_view_is_loading(self.slots[0].view) == FALSE =>
            {
                let title = text(webkit_web_view_get_title(self.slots[0].view));
                if title != "Fixture 0" {
                    qa.errors.push(format!("Embedded document title: {title}"));
                }
                qa.results.push(json!({"embeddedDocumentTitle":title,"viewWidget":text(g_type_name_from_instance(self.slots[0].view))}));
                self.open(1);
                qa.stage = 2;
            }
            2 if !self.slots[1].view.is_null()
                && webkit_web_view_is_loading(self.slots[1].view) == FALSE =>
            {
                self.request_sleep(0, true);
                qa.stage = 3;
            }
            3 if self.slots[0].view.is_null() => {
                qa.results.push(json!({"inactiveViewDestroyed":true,"otherViewAlive":!self.slots[1].view.is_null()}));
                self.open(0);
                qa.stage = 4;
            }
            4 if !self.slots[0].view.is_null()
                && webkit_web_view_is_loading(self.slots[0].view) == FALSE =>
            {
                qa.results.push(json!({"wakeRecreatedNativeView":true,
                    "sandboxEnabled":webkit_web_context_get_sandbox_enabled(self.context)!=FALSE}));
                self.open(2);
                qa.stage = 6;
            }
            6..=8 => {
                let index = (qa.stage - 4) as usize;
                if !self.slots[index].view.is_null()
                    && webkit_web_view_is_loading(self.slots[index].view) == FALSE
                {
                    if index < 4 {
                        self.open(index + 1);
                        qa.stage += 1;
                    } else {
                        qa.views = std::array::from_fn(|index| self.slots[index].view as usize);
                        qa.creates = std::array::from_fn(|index| self.slots[index].creates);
                        qa.loads = std::array::from_fn(|index| self.slots[index].loads);
                        self.open(0);
                        gtk_window_resize(self.window, 420, 640);
                        qa.stage = 13;
                    }
                }
            }
            13..=31 => {
                let (width, height) = match qa.stage {
                    17 | 18 => (320, 640),
                    19 => (1100, 320),
                    20 | 24 => (1100, 740),
                    29..=31 => (320, 360),
                    _ => (420, 640),
                };
                let view = self.slots[0].view;
                let expected = if Mode::for_size(width, height) == Mode::BottomBar {
                    (width, height - layout::BOTTOM_HEIGHT)
                } else {
                    (width - 56, height)
                };
                if gtk_widget_get_allocated_width(self.window) == width
                    && gtk_widget_get_allocated_height(self.window) == height
                    && gtk_widget_get_allocated_width(view) == expected.0
                    && gtk_widget_get_allocated_height(view) == expected.1
                {
                    if let Some(state) = qa.page_state.take() {
                        if (state["viewport"].as_f64().unwrap_or_default() - expected.0 as f64)
                            .abs()
                            > 2.0
                        {
                            self.qa_page(&mut qa);
                        } else {
                            self.check_warm_views(&mut qa);
                            let chat =
                                matches!(qa.stage, 14 | 16 | 17 | 18 | 21 | 23 | 28 | 29 | 31);
                            let list = matches!(qa.stage, 13 | 15 | 25 | 26 | 27 | 30);
                            let enabled = qa.stage != 22;
                            let visible =
                                |pane: &str| state[pane]["visible"].as_bool().unwrap_or(false);
                            if visible("chat") != !list || visible("list") != !chat {
                                qa.errors.push(format!(
                                    "Single-pane visibility at step {}: {state}",
                                    qa.stage
                                ));
                            }
                            if (chat || list)
                                && (state[if chat { "chat" } else { "list" }]["width"]
                                    .as_f64()
                                    .unwrap_or_default()
                                    - expected.0 as f64)
                                    .abs()
                                    > 2.0
                            {
                                qa.errors.push(format!(
                                    "Active pane does not fill viewport at step {}",
                                    qa.stage
                                ));
                            }
                            if state["adapter"].is_null() {
                                qa.errors
                                    .push("Named script world did not retain its adapter".into());
                            }
                            if state["backCount"].as_u64() != Some(u64::from(chat)) {
                                qa.errors.push(format!(
                                    "Duplicate or stale Back button at step {}",
                                    qa.stage
                                ));
                            }
                            if visible("back") != chat {
                                qa.errors
                                    .push(format!("Back button visibility at step {}", qa.stage));
                            }
                            if enabled
                                && state["rootWidth"].as_f64().unwrap_or(f64::MAX)
                                    > expected.0 as f64 + 2.0
                            {
                                qa.errors
                                    .push(format!("Page overflows at step {}: {state}", qa.stage));
                            }
                            if qa.stage > 13 && state["font"].as_f64().unwrap_or_default() < 18.0 {
                                qa.errors
                                    .push("Single pane shrank the composer font".into());
                            }
                            if qa.stage >= 15 && state["draft"].as_str() != Some("retained draft") {
                                qa.errors
                                    .push("Single-pane navigation lost the draft".into());
                            }
                            if (15..=17).contains(&qa.stage)
                                && state["identity"].as_str() != Some("1")
                            {
                                qa.errors
                                    .push("Back/reopen replaced the retained conversation".into());
                            }
                            if qa.stage >= 18 && state["identity"].as_str() != Some("2") {
                                qa.errors.push(
                                    "Adapter did not recover from SPA conversation replacement"
                                        .into(),
                                );
                            }
                            if chat {
                                let inside = |value: &Value| {
                                    value["x"].as_f64().unwrap_or(-1.0) >= -1.0
                                        && value["right"].as_f64().unwrap_or(f64::MAX)
                                            <= expected.0 as f64 + 1.0
                                        && value["y"].as_f64().unwrap_or(-1.0) >= -1.0
                                        && value["bottom"].as_f64().unwrap_or(f64::MAX)
                                            <= expected.1 as f64 + 1.0
                                };
                                if !inside(&state["header"])
                                    || !inside(&state["composer"])
                                    || (state["header"]["height"].as_f64().unwrap_or_default()
                                        - 56.0)
                                        .abs()
                                        > 1.0
                                    || (state["composer"]["bottom"].as_f64().unwrap_or_default()
                                        - expected.1 as f64)
                                        .abs()
                                        > 1.0
                                {
                                    qa.errors.push(format!(
                                        "Mobile header/composer is clipped at step {}: {state}",
                                        qa.stage
                                    ));
                                }
                                if state["controls"].as_array().is_none_or(|controls| {
                                    controls.iter().any(|item| {
                                        !inside(item)
                                            || item["width"].as_f64().unwrap_or_default() < 44.0
                                            || item["height"].as_f64().unwrap_or_default() < 44.0
                                    })
                                }) {
                                    qa.errors.push(format!(
                                        "Mobile tap targets are clipped or too small at step {}",
                                        qa.stage
                                    ));
                                }
                                if state["messages"]["height"].as_f64().unwrap_or_default() < 40.0 {
                                    qa.errors.push("Mobile message scroller collapsed".into());
                                }
                            }
                            if matches!(qa.stage, 15 | 30)
                                && state["focused"].as_str() != Some("relay-chat-row")
                            {
                                qa.errors
                                    .push("Back did not restore focus to the selected row".into());
                            }
                            if qa.stage == 15
                                && (state["listScroll"].as_f64().unwrap_or_default() - 30.0).abs()
                                    > 1.0
                            {
                                qa.errors.push("Back lost the list scroll position".into());
                            }
                            if qa.stage == 16
                                && (state["messages"]["scroll"].as_f64().unwrap_or_default()
                                    - 160.0)
                                    .abs()
                                    > 1.0
                            {
                                qa.errors
                                    .push("Reopening lost the conversation scroll position".into());
                            }
                            if enabled && width < 748 && visible("navigation") != (qa.stage == 26) {
                                qa.errors.push(format!(
                                    "Desktop navigation leaked into the mobile view at step {}",
                                    qa.stage
                                ));
                            }
                            if qa.stage == 26
                                && (!visible("scrim")
                                    || state["navigation"]["width"].as_f64() != Some(72.0))
                            {
                                qa.errors
                                    .push("WhatsApp navigation drawer did not open".into());
                            }
                            if qa.stage == 31 && state["focused"].as_str() != Some("relay-wa-back")
                            {
                                qa.errors
                                    .push("Keyboard selection did not focus Back".into());
                            }
                            if self.slots.iter().any(|slot| {
                                (webkit_web_view_get_zoom_level(slot.view) - 1.0).abs() > 0.001
                            }) {
                                qa.errors
                                    .push("Single-pane navigation changed native page zoom".into());
                            }
                            qa.results.push(json!({"singlePaneStep":qa.stage,"windowSize":[width,height],"page":state,"zoom":webkit_web_view_get_zoom_level(view)}));
                            if matches!(qa.stage, 14 | 17) {
                                if let Some(path) = &qa.snapshot {
                                    let name = if qa.stage == 14 {
                                        "whatsapp-chat-420.png"
                                    } else {
                                        "whatsapp-chat-320.png"
                                    };
                                    if let Err(error) = self.snapshot(&path.with_file_name(name)) {
                                        qa.errors.push(error);
                                    }
                                }
                            }
                            match qa.stage {
                                13 => self.evaluate(view,"document.getElementById('pane-side').scrollTop=30; document.getElementById('relay-chat-row').click(); null;",Some(evaluated),null_mut()),
                                15 | 27 => self.evaluate(view,"document.getElementById('relay-chat-row').click(); null;",Some(evaluated),null_mut()),
                                14 => self.evaluate(view,"document.getElementById('relay-fixture-draft').value='retained draft'; document.getElementById('relay-message-scroll').scrollTop=160; document.getElementById('relay-wa-back')?.click(); document.getElementById('relay-row-menu').click(); null;",Some(evaluated),null_mut()),
                                16 => gtk_window_resize(self.window,320,640),
                                17 => self.evaluate(view,"document.getElementById('relay-fixture-replace').click(); null;",Some(evaluated),null_mut()),
                                18 => gtk_window_resize(self.window,1100,320),
                                19 => gtk_window_resize(self.window,1100,740),
                                20 => gtk_window_resize(self.window,420,640),
                                21 => enqueue(Action::Compact(false)),
                                22 => enqueue(Action::Compact(true)),
                                23 => gtk_window_resize(self.window,1100,740),
                                24 => { self.evaluate(view,"globalThis.relayWhatsApp.showList(); null;",Some(evaluated),null_mut()); gtk_window_resize(self.window,420,640); }
                                25 => self.evaluate(view,"document.getElementById('relay-wa-navigation').click(); null;",Some(evaluated),null_mut()),
                                26 => self.evaluate(view,"document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true})); null;",Some(evaluated),null_mut()),
                                28 => gtk_window_resize(self.window,320,360),
                                29 => self.evaluate(view,"document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true})); null;",Some(evaluated),null_mut()),
                                30 => self.evaluate(view,"const row=document.getElementById('relay-chat-row'); row.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true})); row.click(); null;",Some(evaluated),null_mut()),
                                _ => {}
                            }
                            if qa.stage == 31 {
                                qa.results.push(json!({"singlePaneSteps":19,"singlePaneNavigation":qa.errors.is_empty(),
                                    "whatsappDraftPreserved":qa.errors.is_empty(),"normalZoomThroughout":qa.errors.is_empty(),
                                    "spaReplacementRecovered":qa.errors.is_empty(),"mobileControlsFit":qa.errors.is_empty(),"focusAndScrollRetained":qa.errors.is_empty(),"navigationDrawerWorks":qa.errors.is_empty(),"resizePreservedLoadedViews":qa.errors.is_empty(),
                                    "resizeExtraDocumentLoads":self.slots.iter().enumerate().map(|(index,slot)|slot.loads.saturating_sub(qa.loads[index])).sum::<u64>()}));
                                qa.stage = 9;
                            } else {
                                qa.stage += 1;
                            }
                        }
                    } else {
                        self.qa_page(&mut qa);
                    }
                }
            }
            9 => {
                self.check_warm_views(&mut qa);
                if let Some(index) = qa.last_target {
                    if self.active != Some(index)
                        || gtk_stack_get_visible_child(self.stack) != self.slots[index].view
                    {
                        qa.errors
                            .push("Native selection did not show the requested view".into());
                    }
                }
                if qa.pending_switch.is_some() {
                    qa.errors
                        .push("Native button selection was not dispatched".into());
                    qa.stage = 5;
                } else if qa.switch_step < 60 {
                    if qa.switch_step == 40 {
                        qa.focuses = std::array::from_fn(|index| self.slots[index].focuses);
                    }
                    let index = if qa.switch_step < 40 {
                        qa.switch_step % 5
                    } else {
                        4
                    };
                    qa.last_target = Some(index);
                    qa.pending_switch = Some((index, Instant::now()));
                    qa.switch_step += 1;
                    gtk_button_clicked(self.buttons[index]);
                } else {
                    if self
                        .slots
                        .iter()
                        .map(|slot| slot.focuses)
                        .collect::<Vec<_>>()
                        != qa.focuses
                    {
                        qa.errors
                            .push("Repeated current-app selections moved focus again".into());
                    }
                    qa.retention_started = Some(Instant::now());
                    qa.stage = 10;
                }
            }
            10 if qa
                .retention_started
                .is_some_and(|at| at.elapsed() >= Duration::from_secs(35)) =>
            {
                self.check_warm_views(&mut qa);
                let mut timings = qa.switch_ms.clone();
                timings.sort_by(f64::total_cmp);
                if timings.len() != 60 {
                    qa.errors.push(format!(
                        "Expected 60 native button timings, got {}",
                        timings.len()
                    ));
                }
                let percentile = |fraction: f64| {
                    timings
                        .get(((timings.len().saturating_sub(1)) as f64 * fraction).ceil() as usize)
                        .copied()
                        .unwrap_or_default()
                };
                qa.results.push(json!({
                    "warmSwitches":40,"sameServiceClicks":20,
                    "retainedAfter35Seconds":qa.errors.is_empty(),
                    "warmViewsRecreated":self.slots.iter().enumerate().map(|(index,slot)| slot.creates.saturating_sub(qa.creates[index])).sum::<u64>(),
                    "warmExtraDocumentLoads":self.slots.iter().enumerate().map(|(index,slot)| slot.loads.saturating_sub(qa.loads[index])).sum::<u64>(),
                    "nativeSelectionLatencyMs":{"median":percentile(0.5),"p95":percentile(0.95),"max":percentile(1.0)},
                    "timingScope":"native button signal to GTK visible-child selection; excludes screen presentation and network loading",
                    "idleUnloadSeconds":IDLE_DELAY.as_secs()
                }));
                self.slots[0].inactive = Some(Instant::now() - IDLE_DELAY - Duration::from_secs(1));
                enqueue(Action::Idle(0));
                qa.stage = 11;
            }
            11 if self.slots[0].view.is_null() => {
                qa.results.push(json!({"longIdleViewDestroyed":true}));
                enqueue(Action::Open(0));
                enqueue(Action::Open(1));
                enqueue(Action::Open(4));
                qa.stage = 12;
            }
            12 => {
                let passed = self.active == Some(4)
                    && self.slots[0].view.is_null()
                    && self.slots[0].creates == qa.creates[0];
                if !passed {
                    qa.errors
                        .push("Superseded selection created an intermediate cold view".into());
                }
                qa.results.push(json!({"latestSelectionWins":passed,"intermediateColdViewCreated":!self.slots[0].view.is_null()}));
                qa.stage = 5;
            }
            5 => {
                let report =
                    json!({"passed":qa.errors.is_empty(),"checks":qa.results,"errors":qa.errors});
                if let Err(error) = fs::write(
                    &qa.report,
                    serde_json::to_vec_pretty(&report).unwrap_or_default(),
                ) {
                    eprintln!("QA report: {error}");
                }
                self.qa = Some(qa);
                enqueue(Action::Quit);
                return;
            }
            _ => {}
        }
        self.qa = Some(qa);
        g_timeout_add(150, Some(qa_due), null_mut());
    }

    unsafe fn check_warm_views(&self, qa: &mut Qa) {
        for index in 0..5 {
            let slot = &self.slots[index];
            if slot.view as usize != qa.views[index]
                || slot.creates != qa.creates[index]
                || slot.loads != qa.loads[index]
                || slot.view.is_null()
                || webkit_web_view_is_loading(slot.view) != FALSE
            {
                qa.errors
                    .push(format!("Warm service {index} was recreated or reloaded"));
            }
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            for index in 0..5 {
                cancel_timer(index);
                self.release_view(index);
            }
            for (window, _) in self.popups.drain(..) {
                gtk_widget_destroy(window);
                g_object_unref(window);
            }
            if !self.destroyed {
                gtk_widget_destroy(self.window);
            }
            g_object_unref(self.window);
            if !self.context.is_null() {
                g_object_unref(self.context);
            }
            if !self.manager.is_null() {
                g_object_unref(self.manager);
            }
        }
    }
}

struct Qa {
    report: std::path::PathBuf,
    snapshot: Option<std::path::PathBuf>,
    started: Instant,
    stage: u8,
    results: Vec<Value>,
    errors: Vec<String>,
    views: [usize; 5],
    creates: [u64; 5],
    loads: [u64; 5],
    focuses: [u64; 5],
    switch_step: usize,
    last_target: Option<usize>,
    pending_switch: Option<(usize, Instant)>,
    switch_ms: Vec<f64>,
    retention_started: Option<Instant>,
    page_pending: bool,
    page_state: Option<Value>,
}
impl Qa {
    fn new(report: std::path::PathBuf, snapshot: Option<std::path::PathBuf>) -> Self {
        Self {
            report,
            snapshot,
            started: Instant::now(),
            stage: 0,
            results: Vec::new(),
            errors: Vec::new(),
            views: [0; 5],
            creates: [0; 5],
            loads: [0; 5],
            focuses: [0; 5],
            switch_step: 0,
            last_target: None,
            pending_switch: None,
            switch_ms: Vec::with_capacity(60),
            retention_started: None,
            page_pending: false,
            page_state: None,
        }
    }
}

unsafe fn capturing(view: Ptr) -> bool {
    webkit_web_view_get_camera_capture_state(view) != 0
        || webkit_web_view_get_microphone_capture_state(view) != 0
        || webkit_web_view_get_display_capture_state(view) != 0
}
unsafe fn configure_view(view: Ptr) {
    webkit_web_view_set_background_color(view, &DARK);
    let settings = webkit_web_view_get_settings(view);
    webkit_settings_set_zoom_text_only(settings, FALSE);
    webkit_settings_set_enable_page_cache(settings, FALSE);
    webkit_settings_set_enable_dns_prefetching(settings, FALSE);
    webkit_settings_set_enable_smooth_scrolling(settings, FALSE);
    webkit_settings_set_enable_media_stream(settings, TRUE);
    webkit_settings_set_enable_mediasource(settings, TRUE);
    webkit_settings_set_javascript_can_open_windows_automatically(settings, FALSE);
    webkit_settings_set_media_playback_requires_user_gesture(settings, TRUE);
    connect(view, c!("create"), popup_create as *const (), 0);
    connect(
        view,
        c!("permission-request"),
        permission_request as *const (),
        0,
    );
    connect(view, c!("decide-policy"), decide_policy as *const (), 0);
}
unsafe extern "C" fn size_allocated(_: Ptr, allocation: *const Rectangle, _: Ptr) {
    if let Some(allocation) = allocation.as_ref() {
        let size = (allocation.width, allocation.height);
        if BRIDGE.with(|bridge| bridge.size.replace(size) != size) {
            enqueue(Action::Resize(size.0, size.1));
        }
    }
}

unsafe fn window_size(preferred: (i32, i32), parent: Ptr) -> (i32, i32) {
    let display = gdk_display_get_default();
    if display.is_null() {
        return preferred;
    }
    let mut monitor = if !parent.is_null() && gtk_widget_get_realized(parent) != FALSE {
        gdk_display_get_monitor_at_window(display, gtk_widget_get_window(parent))
    } else {
        gdk_display_get_primary_monitor(display)
    };
    if monitor.is_null() && gdk_display_get_n_monitors(display) > 0 {
        monitor = gdk_display_get_monitor(display, 0);
    }
    if monitor.is_null() {
        return preferred;
    }
    let mut area = Rectangle::default();
    gdk_monitor_get_workarea(monitor, &mut area);
    layout::initial_size(area.width, area.height, preferred)
}

unsafe extern "C" fn service_clicked(_: Ptr, data: Ptr) {
    enqueue(Action::Open(data as usize));
}
unsafe extern "C" fn sleep_clicked(_: Ptr, _: Ptr) {
    enqueue(Action::SleepAll);
}
unsafe extern "C" fn quit_clicked(_: Ptr, _: Ptr) {
    enqueue(Action::Quit);
}
unsafe extern "C" fn delete_window(_: Ptr, _: Ptr, _: Ptr) -> Bool {
    enqueue(Action::Quit);
    TRUE
}
unsafe extern "C" fn destroyed_window(_: Ptr, _: Ptr) {
    BRIDGE.with(|bridge| bridge.window.set(null_mut()));
    gtk_main_quit();
}
unsafe extern "C" fn view_changed(view: Ptr, _: Ptr, data: Ptr) {
    enqueue(Action::Changed(data as usize, view));
}
unsafe extern "C" fn evaluated(source: Ptr, result: Ptr, _: Ptr) {
    let mut error = null_mut();
    let value = webkit_web_view_evaluate_javascript_finish(source, result, &mut error);
    if !value.is_null() {
        g_object_unref(value);
    }
    if !error.is_null() {
        g_error_free(error);
    }
}

unsafe extern "C" fn page_measured(source: Ptr, result: Ptr, data: Ptr) {
    let probe = *Box::from_raw(data.cast::<PageProbe>());
    let mut error = null_mut();
    let value = webkit_web_view_evaluate_javascript_finish(source, result, &mut error);
    let mut state = None;
    if !value.is_null() {
        let serialized = jsc_value_to_string(value);
        if !serialized.is_null() {
            state = serde_json::from_str(&text(serialized)).ok();
            g_free(serialized.cast());
        }
        g_object_unref(value);
    }
    if !error.is_null() {
        g_error_free(error);
    }
    enqueue(Action::QaPage(PageReply { probe, state }));
}
unsafe extern "C" fn load_changed(view: Ptr, event: c_int, data: Ptr) {
    if event == WEBKIT_LOAD_STARTED {
        enqueue(Action::LoadStarted(data as usize, view));
    } else if event == WEBKIT_LOAD_FINISHED && data as usize == 0 {
        enqueue(Action::Adapt(0, view));
    }
}
unsafe extern "C" fn view_closed(view: Ptr, data: Ptr) {
    enqueue(Action::Closed(data as usize, view));
}
unsafe extern "C" fn process_terminated(view: Ptr, _: c_int, data: Ptr) {
    enqueue(Action::Crash(data as usize, view));
}
unsafe extern "C" fn qa_due(_: Ptr) -> Bool {
    enqueue(Action::QaTick);
    G_SOURCE_REMOVE
}
unsafe extern "C" fn key_pressed(_: Ptr, event: Ptr, _: Ptr) -> Bool {
    let mut key = 0;
    let mut modifiers = 0;
    gdk_event_get_keyval(event, &mut key);
    gdk_event_get_state(event, &mut modifiers);
    if modifiers & GDK_CONTROL_MASK != 0 && (b'1' as u32..=b'5' as u32).contains(&key) {
        enqueue(Action::Open((key - b'1' as u32) as usize));
        TRUE
    } else {
        FALSE
    }
}
unsafe extern "C" fn service_menu(widget: Ptr, event: Ptr, data: Ptr) -> Bool {
    let mut button = 0;
    gdk_event_get_button(event, &mut button);
    if button != 3 {
        return FALSE;
    }
    let index = data as usize;
    let kept = BRIDGE
        .with(|bridge| {
            bridge
                .app
                .try_borrow()
                .ok()
                .and_then(|app| app.as_ref().map(|app| app.config.keep_awake[index]))
        })
        .unwrap_or(false);
    let menu = gtk_menu_new();
    g_object_ref_sink(menu);
    let keep = gtk_check_menu_item_new_with_label(c!("Keep awake"));
    gtk_check_menu_item_set_active(keep, kept as Bool);
    connect(keep, c!("toggled"), keep_toggled as *const (), index);
    gtk_menu_shell_append(menu, keep);
    if index == 0 {
        let fit = gtk_check_menu_item_new_with_label(c!("Mobile layout"));
        let enabled = BRIDGE
            .with(|bridge| {
                bridge
                    .app
                    .try_borrow()
                    .ok()
                    .and_then(|app| app.as_ref().map(|app| app.config.compact_whatsapp))
            })
            .unwrap_or(true);
        gtk_check_menu_item_set_active(fit, enabled as Bool);
        connect(fit, c!("toggled"), compact_toggled as *const (), 0);
        gtk_menu_shell_append(menu, fit);
    }
    let sleep = gtk_menu_item_new_with_label(c!("Sleep"));
    connect(sleep, c!("activate"), sleep_one as *const (), index);
    gtk_menu_shell_append(menu, sleep);
    connect(menu, c!("selection-done"), menu_done as *const (), 0);
    gtk_widget_show_all(menu);
    gtk_menu_popup_at_pointer(menu, event);
    gtk_widget_grab_focus(widget);
    TRUE
}
unsafe extern "C" fn keep_toggled(widget: Ptr, data: Ptr) {
    enqueue(Action::Keep(
        data as usize,
        gtk_check_menu_item_get_active(widget) != FALSE,
    ));
}
unsafe extern "C" fn compact_toggled(widget: Ptr, _: Ptr) {
    enqueue(Action::Compact(
        gtk_check_menu_item_get_active(widget) != FALSE,
    ));
}
unsafe extern "C" fn sleep_one(_: Ptr, data: Ptr) {
    enqueue(Action::Sleep(data as usize, true));
}
unsafe extern "C" fn menu_done(menu: Ptr, _: Ptr) {
    gtk_widget_destroy(menu);
    g_object_unref(menu);
}

unsafe fn question(title: &str, description: &str) -> bool {
    BRIDGE.with(|bridge| {
        let was_busy = bridge.busy.replace(true);
        let dialog = gtk_message_dialog_new(
            bridge.window.get(),
            GTK_DIALOG_MODAL | GTK_DIALOG_DESTROY_WITH_PARENT,
            GTK_MESSAGE_QUESTION,
            GTK_BUTTONS_YES_NO,
            c!("%s"),
            cs(description).as_ptr(),
        );
        gtk_window_set_title(dialog, cs(title).as_ptr());
        let allowed = gtk_dialog_run(dialog) == GTK_RESPONSE_YES;
        gtk_widget_destroy(dialog);
        bridge.busy.set(was_busy);
        if !was_busy && !bridge.queue.borrow().is_empty() {
            schedule_dispatch(bridge);
        }
        allowed
    })
}
unsafe extern "C" fn script_dialog(view: Ptr, dialog: Ptr, data: Ptr) -> Bool {
    if webkit_script_dialog_get_dialog_type(dialog) != 3 {
        return FALSE;
    }
    let index = data as usize;
    let auto = index < 5 && BRIDGE.with(|bridge| bridge.intents[index].get() == 0);
    let leave = !auto
        && question(
            "Unsaved changes",
            "Leave this page? Unsaved changes may be lost.",
        );
    // WebKit's native implementation maps Confirm=true to Leave Page.
    webkit_script_dialog_confirm_set_confirmed(dialog, leave as Bool);
    if !leave {
        if index < 5 {
            enqueue(Action::Cancelled(index, view));
        } else {
            enqueue(Action::PopupCancelled);
        }
    }
    TRUE
}
unsafe extern "C" fn permission_request(view: Ptr, request: Ptr, _: Ptr) -> Bool {
    let media =
        g_type_check_instance_is_a(request, webkit_user_media_permission_request_get_type())
            != FALSE;
    let notifications =
        g_type_check_instance_is_a(request, webkit_notification_permission_request_get_type())
            != FALSE;
    if !media && !notifications {
        webkit_permission_request_deny(request);
        return TRUE;
    }
    let uri = text(webkit_web_view_get_uri(view));
    let origin = url::Url::parse(&uri)
        .map(|url| url.origin().ascii_serialization())
        .unwrap_or_else(|_| "This page".into());
    let capability = if media {
        "use the requested camera, microphone, or screen"
    } else {
        "show notifications"
    };
    let allow = question(
        "Website permission",
        &format!("Allow {origin} to {capability}?"),
    );
    if allow {
        webkit_permission_request_allow(request);
    } else {
        webkit_permission_request_deny(request);
    }
    TRUE
}
unsafe extern "C" fn popup_create(parent: Ptr, _: Ptr, _: Ptr) -> Ptr {
    let root = BRIDGE.with(|bridge| bridge.window.get());
    if root.is_null() {
        return null_mut();
    }
    let view = webkit_web_view_new_with_related_view(parent);
    configure_view(view);
    let window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    g_object_ref_sink(window);
    gtk_window_set_title(window, c!("Sign in"));
    let initial = window_size((780, 680), root);
    gtk_window_set_default_size(window, initial.0, initial.1);
    gtk_widget_set_size_request(window, layout::MIN_WIDTH, layout::MIN_HEIGHT);
    gtk_window_set_transient_for(window, root);
    gtk_window_set_destroy_with_parent(window, TRUE);
    gtk_container_add(window, view);
    connect(
        view,
        c!("ready-to-show"),
        popup_ready as *const (),
        window as usize,
    );
    connect(
        view,
        c!("close"),
        popup_web_close as *const (),
        window as usize,
    );
    connect(
        view,
        c!("notify::uri"),
        popup_uri as *const (),
        window as usize,
    );
    connect(view, c!("script-dialog"), script_dialog as *const (), 5);
    connect(
        window,
        c!("delete-event"),
        popup_delete as *const (),
        view as usize,
    );
    connect(
        window,
        c!("destroy"),
        popup_destroyed as *const (),
        window as usize,
    );
    enqueue(Action::Popup(window, view));
    view
}
unsafe extern "C" fn popup_ready(_: Ptr, data: Ptr) {
    gtk_widget_show_all(data);
    gtk_window_present(data);
}
unsafe extern "C" fn popup_web_close(_: Ptr, data: Ptr) {
    gtk_widget_destroy(data);
}
unsafe extern "C" fn popup_delete(_: Ptr, _: Ptr, data: Ptr) -> Bool {
    webkit_web_view_try_close(data);
    TRUE
}
unsafe extern "C" fn popup_destroyed(_: Ptr, data: Ptr) {
    enqueue(Action::PopupClosed(data));
}
unsafe extern "C" fn popup_uri(view: Ptr, _: Ptr, data: Ptr) {
    let uri = text(webkit_web_view_get_uri(view));
    let title = url::Url::parse(&uri)
        .map(|url| url.origin().ascii_serialization())
        .unwrap_or_else(|_| "Sign in".into());
    gtk_window_set_title(data, cs(&title).as_ptr());
}
unsafe extern "C" fn decide_policy(_: Ptr, decision: Ptr, kind: c_int, _: Ptr) -> Bool {
    if kind == WEBKIT_POLICY_DECISION_TYPE_RESPONSE {
        if webkit_response_policy_decision_is_mime_type_supported(decision) == FALSE {
            webkit_policy_decision_download(decision);
            return TRUE;
        }
        return FALSE;
    }
    if kind != WEBKIT_POLICY_DECISION_TYPE_NAVIGATION_ACTION
        && kind != WEBKIT_POLICY_DECISION_TYPE_NEW_WINDOW_ACTION
    {
        return FALSE;
    }
    let action = webkit_navigation_policy_decision_get_navigation_action(decision);
    let request = webkit_navigation_action_get_request(action);
    let raw = webkit_uri_request_get_uri(request);
    let uri = text(raw);
    if SERVICES[0].is_allowed_navigation(&uri) || uri == "about:blank" || uri.starts_with("blob:") {
        return FALSE;
    }
    if webkit_navigation_action_is_user_gesture(action) != FALSE
        && (uri.starts_with("mailto:") || uri.starts_with("tel:") || uri.starts_with("http://"))
    {
        g_app_info_launch_default_for_uri(raw, null_mut(), null_mut());
    }
    webkit_policy_decision_ignore(decision);
    TRUE
}
unsafe extern "C" fn download_started(_: Ptr, download: Ptr, _: Ptr) {
    connect(
        download,
        c!("decide-destination"),
        download_destination as *const (),
        0,
    );
}
unsafe extern "C" fn download_destination(download: Ptr, suggested: *const c_char, _: Ptr) -> Bool {
    let root = BRIDGE.with(|bridge| bridge.window.get());
    if root.is_null() {
        webkit_download_cancel(download);
        return TRUE;
    }
    let chooser = gtk_file_chooser_native_new(
        c!("Save file"),
        root,
        GTK_FILE_CHOOSER_ACTION_SAVE,
        c!("Save"),
        c!("Cancel"),
    );
    gtk_native_dialog_set_modal(chooser, TRUE);
    gtk_file_chooser_set_current_name(chooser, suggested);
    gtk_file_chooser_set_do_overwrite_confirmation(chooser, TRUE);
    let response = gtk_native_dialog_run(chooser);
    if response == GTK_RESPONSE_ACCEPT {
        let filename = gtk_file_chooser_get_filename(chooser);
        if !filename.is_null() {
            let uri = g_filename_to_uri(filename, null(), null_mut());
            if !uri.is_null() {
                webkit_download_set_allow_overwrite(download, TRUE);
                webkit_download_set_destination(download, uri);
                g_free(uri.cast());
            } else {
                webkit_download_cancel(download);
            }
            g_free(filename.cast());
        } else {
            webkit_download_cancel(download);
        }
    } else {
        webkit_download_cancel(download);
    }
    gtk_native_dialog_destroy(chooser);
    g_object_unref(chooser);
    TRUE
}

pub fn run(options: Options) -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::prepare(options.data_dir.as_deref())?;
    let lock = InstanceLock::acquire(&storage.data_dir.join("relay.lock")).map_err(|error| {
        format!("The app is already running or its lock is unavailable: {error}")
    })?;
    let config = storage.load_config()?;
    unsafe {
        if gtk_init_check(null_mut(), null_mut()) == FALSE {
            return Err("A graphical desktop session is required.".into());
        }
        let app = App::new(storage, lock, config, &options);
        let window = app.window;
        BRIDGE.with(|bridge| *bridge.app.borrow_mut() = Some(app));
        gtk_widget_show_all(window);
        if options.qa_report.is_some() {
            g_timeout_add(350, Some(qa_due), null_mut());
        } else if let Some(index) = options.open {
            enqueue(Action::Open(index));
        }
        gtk_main();
        BRIDGE.with(|bridge| {
            bridge.window.set(null_mut());
            if let Some(mut app) = bridge.app.borrow_mut().take() {
                app.destroyed = true;
                drop(app);
            }
            bridge.queue.borrow_mut().clear();
        });
    }
    Ok(())
}

#[cfg(test)]
mod queue_tests {
    use super::*;

    #[test]
    fn latest_selection_skips_intermediate_cold_apps() {
        let mut queue = VecDeque::new();
        queue_action(&mut queue, Action::Open(0));
        queue_action(&mut queue, Action::Changed(0, null_mut()));
        queue_action(&mut queue, Action::Open(1));
        queue_action(&mut queue, Action::Open(4));
        assert_eq!(queue.len(), 2);
        assert!(matches!(queue.back(), Some(Action::Open(4))));
    }

    #[test]
    fn resize_events_coalesce_and_keep_lifecycle_order() {
        let mut queue = VecDeque::new();
        queue_action(&mut queue, Action::Resize(1100, 740));
        queue_action(&mut queue, Action::Open(1));
        queue_action(&mut queue, Action::Resize(320, 240));
        assert_eq!(queue.len(), 2);
        assert!(matches!(queue.front(), Some(Action::Resize(320, 240))));
        queue_action(&mut queue, Action::Quit);
        queue_action(&mut queue, Action::Resize(420, 640));
        assert_eq!(queue.len(), 4);
    }

    #[test]
    fn explicit_sleep_preserves_selection_order() {
        let mut queue = VecDeque::new();
        queue_action(&mut queue, Action::Open(0));
        queue_action(&mut queue, Action::SleepAll);
        queue_action(&mut queue, Action::Open(4));
        assert_eq!(queue.len(), 3);
        assert!(matches!(queue.front(), Some(Action::Open(0))));
        assert!(matches!(queue[1], Action::SleepAll));
    }

    #[test]
    fn notifications_coalesce_without_crossing_lifecycle_events() {
        let mut queue = VecDeque::new();
        for _ in 0..1000 {
            queue_action(&mut queue, Action::Changed(0, null_mut()));
        }
        assert_eq!(queue.len(), 1);
        queue_action(&mut queue, Action::Closed(0, null_mut()));
        queue_action(&mut queue, Action::Changed(0, null_mut()));
        assert_eq!(queue.len(), 3);
    }
}
