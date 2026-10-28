//! Public GTK 3 / WebKitGTK 4.1 C ABI used by the native Linux shell.
//!
//! Opaque objects stay behind C pointers. Calls and callbacks must run on the
//! GTK main thread. Callers must respect each API's transfer ownership, keep
//! callback userdata alive until disconnection, and never unwind across C.
//! GObject signals erase callback signatures: cast only an `extern "C"`
//! callback whose parameters/return exactly match the documented signal.
//!
//! Exact installed SONAMEs avoid development headers/pkg-config at build time;
//! the dynamic loader still resolves ordinary system shared libraries. These
//! declarations target the stable public ABI, not private WebKit internals.
//! API references: <https://docs.gtk.org/gtk3/> and
//! <https://webkitgtk.org/reference/webkit2gtk/stable/> (API version 4.1).
#![allow(dead_code)]

pub use std::ffi::{c_char, c_double, c_int, c_uint, c_ulong, c_void};

pub type Ptr = *mut c_void;
pub type Bool = c_int;
pub type GType = usize;
pub type GCallback = Option<unsafe extern "C" fn()>;
pub type GSourceFunc = Option<unsafe extern "C" fn(Ptr) -> Bool>;
pub type GDestroyNotify = Option<unsafe extern "C" fn(Ptr)>;
pub type GClosureNotify = Option<unsafe extern "C" fn(Ptr, Ptr)>;
pub type GAsyncReadyCallback = Option<unsafe extern "C" fn(Ptr, Ptr, Ptr)>;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Rgba {
    pub red: c_double,
    pub green: c_double,
    pub blue: c_double,
    pub alpha: c_double,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Rectangle {
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
}

#[repr(C)]
pub struct GError {
    pub domain: c_uint,
    pub code: c_int,
    pub message: *mut c_char,
}

pub const FALSE: Bool = 0;
pub const TRUE: Bool = 1;
pub const G_SOURCE_REMOVE: Bool = FALSE;
pub const G_SOURCE_CONTINUE: Bool = TRUE;
pub const G_PRIORITY_DEFAULT: c_int = 0;
pub const GTK_WINDOW_TOPLEVEL: c_int = 0;
pub const GTK_ORIENTATION_HORIZONTAL: c_int = 0;
pub const GTK_ORIENTATION_VERTICAL: c_int = 1;
pub const GTK_WIN_POS_CENTER: c_int = 1;
pub const GTK_RELIEF_NONE: c_int = 2;
pub const GTK_ALIGN_FILL: c_int = 0;
pub const GTK_ALIGN_START: c_int = 1;
pub const GTK_ALIGN_END: c_int = 2;
pub const GTK_ALIGN_CENTER: c_int = 3;
pub const GTK_STYLE_PROVIDER_PRIORITY_APPLICATION: c_uint = 600;
pub const GTK_DIALOG_MODAL: c_int = 1;
pub const GTK_DIALOG_DESTROY_WITH_PARENT: c_int = 2;
pub const GTK_MESSAGE_INFO: c_int = 0;
pub const GTK_MESSAGE_WARNING: c_int = 1;
pub const GTK_MESSAGE_QUESTION: c_int = 2;
pub const GTK_MESSAGE_ERROR: c_int = 3;
pub const GTK_BUTTONS_NONE: c_int = 0;
pub const GTK_BUTTONS_OK: c_int = 1;
pub const GTK_BUTTONS_CLOSE: c_int = 2;
pub const GTK_BUTTONS_CANCEL: c_int = 3;
pub const GTK_BUTTONS_YES_NO: c_int = 4;
pub const GTK_BUTTONS_OK_CANCEL: c_int = 5;
pub const GTK_RESPONSE_ACCEPT: c_int = -3;
pub const GTK_RESPONSE_OK: c_int = -5;
pub const GTK_RESPONSE_CANCEL: c_int = -6;
pub const GTK_RESPONSE_CLOSE: c_int = -7;
pub const GTK_RESPONSE_YES: c_int = -8;
pub const GTK_RESPONSE_NO: c_int = -9;
pub const GTK_FILE_CHOOSER_ACTION_OPEN: c_int = 0;
pub const GTK_FILE_CHOOSER_ACTION_SAVE: c_int = 1;
pub const GDK_SHIFT_MASK: c_uint = 1;
pub const GDK_CONTROL_MASK: c_uint = 1 << 2;
pub const GDK_MOD1_MASK: c_uint = 1 << 3;
pub const GDK_BUTTON_PRESS_MASK: c_int = 1 << 8;
pub const GDK_KEY_PRESS_MASK: c_int = 1 << 10;
pub const GDK_KEY_ESCAPE: c_uint = 0xff1b;
pub const GDK_KEY_F5: c_uint = 0xffc2;
pub const GDK_KEY_F6: c_uint = 0xffc3;
pub const WEBKIT_CACHE_MODEL_DOCUMENT_VIEWER: c_int = 0;
pub const WEBKIT_CACHE_MODEL_DOCUMENT_BROWSER: c_int = 2;
pub const WEBKIT_COOKIE_PERSISTENT_STORAGE_SQLITE: c_int = 1;
pub const WEBKIT_POLICY_DECISION_TYPE_NAVIGATION_ACTION: c_int = 0;
pub const WEBKIT_POLICY_DECISION_TYPE_NEW_WINDOW_ACTION: c_int = 1;
pub const WEBKIT_POLICY_DECISION_TYPE_RESPONSE: c_int = 2;
pub const WEBKIT_LOAD_STARTED: c_int = 0;
pub const WEBKIT_LOAD_REDIRECTED: c_int = 1;
pub const WEBKIT_LOAD_COMMITTED: c_int = 2;
pub const WEBKIT_LOAD_FINISHED: c_int = 3;
pub const WEBKIT_MEDIA_CAPTURE_STATE_NONE: c_int = 0;
pub const WEBKIT_SCRIPT_DIALOG_ALERT: c_int = 0;
pub const WEBKIT_SCRIPT_DIALOG_CONFIRM: c_int = 1;
pub const WEBKIT_SCRIPT_DIALOG_PROMPT: c_int = 2;
pub const WEBKIT_SCRIPT_DIALOG_BEFORE_UNLOAD_CONFIRM: c_int = 3;
pub const CAIRO_FORMAT_ARGB32: c_int = 0;

#[link(name = "libgtk-3.so.0", kind = "dylib", modifiers = "+verbatim")]
extern "C" {
    pub fn gtk_init_check(argc: *mut c_int, argv: *mut *mut *mut c_char) -> Bool;
    pub fn gtk_main();
    pub fn gtk_main_quit();
    pub fn gtk_events_pending() -> Bool;
    pub fn gtk_main_iteration_do(blocking: Bool) -> Bool;
    pub fn gtk_get_major_version() -> c_uint;
    pub fn gtk_get_minor_version() -> c_uint;
    pub fn gtk_get_micro_version() -> c_uint;
    pub fn gtk_window_new(kind: c_int) -> Ptr;
    pub fn gtk_window_set_title(window: Ptr, title: *const c_char);
    pub fn gtk_window_set_default_size(window: Ptr, width: c_int, height: c_int);
    pub fn gtk_window_set_wmclass(
        window: Ptr,
        wmclass_name: *const c_char,
        wmclass_class: *const c_char,
    );
    pub fn gtk_window_set_position(window: Ptr, position: c_int);
    pub fn gtk_window_set_decorated(window: Ptr, setting: Bool);
    pub fn gtk_window_set_transient_for(window: Ptr, parent: Ptr);
    pub fn gtk_window_set_destroy_with_parent(window: Ptr, setting: Bool);
    pub fn gtk_window_set_modal(window: Ptr, modal: Bool);
    pub fn gtk_window_present(window: Ptr);
    pub fn gtk_window_resize(window: Ptr, width: c_int, height: c_int);
    pub fn gtk_window_close(window: Ptr);
    pub fn gtk_box_new(orientation: c_int, spacing: c_int) -> Ptr;
    pub fn gtk_box_set_spacing(container: Ptr, spacing: c_int);
    pub fn gtk_box_reorder_child(container: Ptr, child: Ptr, position: c_int);
    pub fn gtk_orientable_set_orientation(orientable: Ptr, orientation: c_int);
    pub fn gtk_box_pack_start(
        container: Ptr,
        child: Ptr,
        expand: Bool,
        fill: Bool,
        padding: c_uint,
    );
    pub fn gtk_box_pack_end(container: Ptr, child: Ptr, expand: Bool, fill: Bool, padding: c_uint);
    pub fn gtk_button_new() -> Ptr;
    pub fn gtk_button_new_with_label(label: *const c_char) -> Ptr;
    pub fn gtk_button_set_relief(button: Ptr, relief: c_int);
    pub fn gtk_button_set_image(button: Ptr, image: Ptr);
    pub fn gtk_button_set_always_show_image(button: Ptr, always_show: Bool);
    pub fn gtk_button_clicked(button: Ptr);
    pub fn gtk_image_new_from_pixbuf(pixbuf: Ptr) -> Ptr;
    pub fn gtk_image_new_from_icon_name(icon_name: *const c_char, size: c_int) -> Ptr;
    pub fn gtk_label_new(text: *const c_char) -> Ptr;
    pub fn gtk_label_set_text(label: Ptr, text: *const c_char);
    pub fn gtk_label_set_xalign(label: Ptr, xalign: f32);
    pub fn gtk_label_set_line_wrap(label: Ptr, wrap: Bool);
    pub fn gtk_stack_new() -> Ptr;
    pub fn gtk_stack_set_homogeneous(stack: Ptr, homogeneous: Bool);
    pub fn gtk_stack_add_named(stack: Ptr, child: Ptr, name: *const c_char);
    pub fn gtk_stack_set_visible_child(stack: Ptr, child: Ptr);
    pub fn gtk_stack_set_visible_child_name(stack: Ptr, name: *const c_char);
    pub fn gtk_stack_set_transition_type(stack: Ptr, transition: c_int);
    pub fn gtk_stack_get_visible_child(stack: Ptr) -> Ptr;
    pub fn gtk_container_add(container: Ptr, widget: Ptr);
    pub fn gtk_container_remove(container: Ptr, widget: Ptr);
    pub fn gtk_container_set_border_width(container: Ptr, border_width: c_uint);
    pub fn gtk_widget_show_all(widget: Ptr);
    pub fn gtk_widget_show(widget: Ptr);
    pub fn gtk_widget_hide(widget: Ptr);
    pub fn gtk_widget_destroy(widget: Ptr);
    pub fn gtk_widget_set_size_request(widget: Ptr, width: c_int, height: c_int);
    pub fn gtk_widget_set_hexpand(widget: Ptr, expand: Bool);
    pub fn gtk_widget_set_vexpand(widget: Ptr, expand: Bool);
    pub fn gtk_widget_set_halign(widget: Ptr, align: c_int);
    pub fn gtk_widget_set_valign(widget: Ptr, align: c_int);
    pub fn gtk_widget_set_can_focus(widget: Ptr, can_focus: Bool);
    pub fn gtk_widget_set_sensitive(widget: Ptr, sensitive: Bool);
    pub fn gtk_widget_set_tooltip_text(widget: Ptr, text: *const c_char);
    pub fn gtk_widget_set_name(widget: Ptr, name: *const c_char);
    pub fn gtk_widget_get_name(widget: Ptr) -> *const c_char;
    pub fn gtk_widget_get_style_context(widget: Ptr) -> Ptr;
    pub fn gtk_widget_get_allocated_width(widget: Ptr) -> c_int;
    pub fn gtk_widget_get_allocated_height(widget: Ptr) -> c_int;
    pub fn gtk_widget_get_window(widget: Ptr) -> Ptr;
    pub fn gtk_widget_get_accessible(widget: Ptr) -> Ptr;
    pub fn gtk_widget_get_toplevel(widget: Ptr) -> Ptr;
    pub fn gtk_widget_get_visible(widget: Ptr) -> Bool;
    pub fn gtk_widget_get_realized(widget: Ptr) -> Bool;
    pub fn gtk_widget_grab_focus(widget: Ptr);
    pub fn gtk_widget_add_events(widget: Ptr, events: c_int);
    pub fn gtk_widget_draw(widget: Ptr, cairo: Ptr);
    pub fn gtk_css_provider_new() -> Ptr;
    pub fn gtk_css_provider_load_from_data(
        provider: Ptr,
        data: *const c_char,
        length: isize,
        error: *mut *mut GError,
    ) -> Bool;
    pub fn gtk_style_context_add_provider_for_screen(screen: Ptr, provider: Ptr, priority: c_uint);
    pub fn gtk_style_context_add_class(context: Ptr, class_name: *const c_char);
    pub fn gtk_style_context_remove_class(context: Ptr, class_name: *const c_char);
    pub fn gtk_settings_get_default() -> Ptr;
    pub fn gtk_menu_new() -> Ptr;
    pub fn gtk_menu_item_new_with_label(label: *const c_char) -> Ptr;
    pub fn gtk_separator_menu_item_new() -> Ptr;
    pub fn gtk_check_menu_item_new_with_label(label: *const c_char) -> Ptr;
    pub fn gtk_check_menu_item_set_active(menu_item: Ptr, active: Bool);
    pub fn gtk_check_menu_item_get_active(menu_item: Ptr) -> Bool;
    pub fn gtk_menu_shell_append(menu_shell: Ptr, child: Ptr);
    pub fn gtk_menu_popup_at_pointer(menu: Ptr, trigger_event: Ptr);
    pub fn gtk_message_dialog_new(
        parent: Ptr,
        flags: c_int,
        kind: c_int,
        buttons: c_int,
        message_format: *const c_char,
        ...
    ) -> Ptr;
    pub fn gtk_message_dialog_format_secondary_text(
        dialog: Ptr,
        message_format: *const c_char,
        ...
    );
    pub fn gtk_dialog_add_button(dialog: Ptr, text: *const c_char, response_id: c_int) -> Ptr;
    pub fn gtk_dialog_add_buttons(dialog: Ptr, first_button_text: *const c_char, ...);
    pub fn gtk_dialog_run(dialog: Ptr) -> c_int;
    pub fn gtk_file_chooser_native_new(
        title: *const c_char,
        parent: Ptr,
        action: c_int,
        accept_label: *const c_char,
        cancel_label: *const c_char,
    ) -> Ptr;
    pub fn gtk_file_chooser_set_current_name(chooser: Ptr, name: *const c_char);
    pub fn gtk_file_chooser_set_do_overwrite_confirmation(chooser: Ptr, do_overwrite: Bool);
    pub fn gtk_file_chooser_get_filename(chooser: Ptr) -> *mut c_char;
    pub fn gtk_native_dialog_run(dialog: Ptr) -> c_int;
    pub fn gtk_native_dialog_set_modal(dialog: Ptr, modal: Bool);
    pub fn gtk_native_dialog_destroy(dialog: Ptr);
}

#[link(name = "libgdk-3.so.0", kind = "dylib", modifiers = "+verbatim")]
extern "C" {
    pub fn gdk_screen_get_default() -> Ptr;
    pub fn gdk_display_get_default() -> Ptr;
    pub fn gdk_display_get_primary_monitor(display: Ptr) -> Ptr;
    pub fn gdk_display_get_monitor_at_window(display: Ptr, window: Ptr) -> Ptr;
    pub fn gdk_display_get_n_monitors(display: Ptr) -> c_int;
    pub fn gdk_display_get_monitor(display: Ptr, index: c_int) -> Ptr;
    pub fn gdk_monitor_get_workarea(monitor: Ptr, workarea: *mut Rectangle);
    pub fn gdk_event_get_button(event: Ptr, button: *mut c_uint) -> Bool;
    pub fn gdk_event_get_keyval(event: Ptr, keyval: *mut c_uint) -> Bool;
    pub fn gdk_event_get_state(event: Ptr, state: *mut c_uint) -> Bool;
    pub fn gdk_event_get_time(event: Ptr) -> c_uint;
    pub fn gdk_window_get_width(window: Ptr) -> c_int;
    pub fn gdk_window_get_height(window: Ptr) -> c_int;
    pub fn gdk_pixbuf_get_from_window(
        window: Ptr,
        src_x: c_int,
        src_y: c_int,
        width: c_int,
        height: c_int,
    ) -> Ptr;
}

#[link(
    name = "libgdk_pixbuf-2.0.so.0",
    kind = "dylib",
    modifiers = "+verbatim"
)]
extern "C" {
    pub fn gdk_pixbuf_new_from_file_at_scale(
        filename: *const c_char,
        width: c_int,
        height: c_int,
        preserve_aspect_ratio: Bool,
        error: *mut *mut GError,
    ) -> Ptr;
    pub fn gdk_pixbuf_savev(
        pixbuf: Ptr,
        filename: *const c_char,
        image_type: *const c_char,
        option_keys: *mut *mut c_char,
        option_values: *mut *mut c_char,
        error: *mut *mut GError,
    ) -> Bool;
}

#[link(name = "libgobject-2.0.so.0", kind = "dylib", modifiers = "+verbatim")]
extern "C" {
    pub fn g_object_ref(object: Ptr) -> Ptr;
    pub fn g_object_ref_sink(object: Ptr) -> Ptr;
    pub fn g_object_unref(object: Ptr);
    pub fn g_object_set(object: Ptr, first_property_name: *const c_char, ...);
    pub fn g_signal_connect_data(
        instance: Ptr,
        detailed_signal: *const c_char,
        callback: GCallback,
        data: Ptr,
        destroy_data: GClosureNotify,
        flags: c_uint,
    ) -> c_ulong;
    pub fn g_signal_handler_disconnect(instance: Ptr, handler_id: c_ulong);
    pub fn g_type_check_instance_is_a(instance: Ptr, iface_type: GType) -> Bool;
    pub fn g_type_name_from_instance(instance: Ptr) -> *const c_char;
}

#[link(name = "libglib-2.0.so.0", kind = "dylib", modifiers = "+verbatim")]
extern "C" {
    pub fn g_timeout_add_seconds(interval: c_uint, function: GSourceFunc, data: Ptr) -> c_uint;
    pub fn g_timeout_add(interval: c_uint, function: GSourceFunc, data: Ptr) -> c_uint;
    pub fn g_idle_add(function: GSourceFunc, data: Ptr) -> c_uint;
    pub fn g_idle_add_full(
        priority: c_int,
        function: GSourceFunc,
        data: Ptr,
        notify: GDestroyNotify,
    ) -> c_uint;
    pub fn g_source_remove(tag: c_uint) -> Bool;
    pub fn g_strdup(text: *const c_char) -> *mut c_char;
    pub fn g_free(memory: Ptr);
    pub fn g_error_free(error: *mut GError);
    pub fn g_get_user_data_dir() -> *const c_char;
    pub fn g_get_user_cache_dir() -> *const c_char;
    pub fn g_filename_to_uri(
        filename: *const c_char,
        hostname: *const c_char,
        error: *mut *mut GError,
    ) -> *mut c_char;
}

#[link(name = "libgio-2.0.so.0", kind = "dylib", modifiers = "+verbatim")]
extern "C" {
    pub fn g_app_info_launch_default_for_uri(
        uri: *const c_char,
        context: Ptr,
        error: *mut *mut GError,
    ) -> Bool;
    pub fn g_cancellable_new() -> Ptr;
    pub fn g_cancellable_cancel(cancellable: Ptr);
}

#[link(name = "libatk-1.0.so.0", kind = "dylib", modifiers = "+verbatim")]
extern "C" {
    pub fn atk_object_set_name(accessible: Ptr, name: *const c_char);
}

#[link(
    name = "libwebkit2gtk-4.1.so.0",
    kind = "dylib",
    modifiers = "+verbatim"
)]
extern "C" {
    pub fn webkit_get_major_version() -> c_uint;
    pub fn webkit_get_minor_version() -> c_uint;
    pub fn webkit_get_micro_version() -> c_uint;
    pub fn webkit_website_data_manager_new(first_option_name: *const c_char, ...) -> Ptr;
    pub fn webkit_website_data_manager_get_cookie_manager(manager: Ptr) -> Ptr;
    pub fn webkit_website_data_manager_set_persistent_credential_storage_enabled(
        manager: Ptr,
        enabled: Bool,
    );
    pub fn webkit_web_context_new_with_website_data_manager(manager: Ptr) -> Ptr;
    pub fn webkit_web_context_set_cache_model(context: Ptr, cache_model: c_int);
    pub fn webkit_web_context_get_cookie_manager(context: Ptr) -> Ptr;
    pub fn webkit_web_context_set_sandbox_enabled(context: Ptr, enabled: Bool);
    pub fn webkit_web_context_get_sandbox_enabled(context: Ptr) -> Bool;
    pub fn webkit_web_context_clear_cache(context: Ptr);
    pub fn webkit_cookie_manager_set_persistent_storage(
        manager: Ptr,
        filename: *const c_char,
        storage: c_int,
    );
    pub fn webkit_web_view_new_with_context(context: Ptr) -> Ptr;
    pub fn webkit_web_view_new_with_related_view(view: Ptr) -> Ptr;
    pub fn webkit_web_view_load_uri(view: Ptr, uri: *const c_char);
    pub fn webkit_web_view_load_html(view: Ptr, content: *const c_char, base_uri: *const c_char);
    pub fn webkit_web_view_get_uri(view: Ptr) -> *const c_char;
    pub fn webkit_web_view_get_title(view: Ptr) -> *const c_char;
    pub fn webkit_web_view_is_loading(view: Ptr) -> Bool;
    pub fn webkit_web_view_is_playing_audio(view: Ptr) -> Bool;
    pub fn webkit_web_view_get_camera_capture_state(view: Ptr) -> c_int;
    pub fn webkit_web_view_get_microphone_capture_state(view: Ptr) -> c_int;
    pub fn webkit_web_view_get_display_capture_state(view: Ptr) -> c_int;
    pub fn webkit_web_view_set_background_color(view: Ptr, rgba: *const Rgba);
    pub fn webkit_web_view_get_settings(view: Ptr) -> Ptr;
    pub fn webkit_web_view_get_zoom_level(view: Ptr) -> c_double;
    pub fn webkit_web_view_set_zoom_level(view: Ptr, zoom_level: c_double);
    pub fn webkit_settings_set_zoom_text_only(settings: Ptr, enabled: Bool);
    pub fn webkit_web_view_set_settings(view: Ptr, settings: Ptr);
    pub fn webkit_web_view_try_close(view: Ptr);
    pub fn webkit_web_view_reload(view: Ptr);
    pub fn webkit_web_view_go_back(view: Ptr);
    pub fn webkit_web_view_go_forward(view: Ptr);
    pub fn webkit_web_view_can_go_back(view: Ptr) -> Bool;
    pub fn webkit_web_view_can_go_forward(view: Ptr) -> Bool;
    pub fn webkit_web_view_stop_loading(view: Ptr);
    pub fn webkit_web_view_terminate_web_process(view: Ptr);
    pub fn webkit_web_view_get_session_state(view: Ptr) -> Ptr;
    pub fn webkit_web_view_restore_session_state(view: Ptr, state: Ptr);
    pub fn webkit_web_view_session_state_ref(state: Ptr) -> Ptr;
    pub fn webkit_web_view_session_state_unref(state: Ptr);
    pub fn webkit_web_view_get_context(view: Ptr) -> Ptr;
    pub fn webkit_web_view_get_website_data_manager(view: Ptr) -> Ptr;
    pub fn webkit_settings_new() -> Ptr;
    pub fn webkit_settings_set_enable_page_cache(settings: Ptr, enabled: Bool);
    pub fn webkit_settings_set_enable_dns_prefetching(settings: Ptr, enabled: Bool);
    pub fn webkit_settings_set_enable_hyperlink_auditing(settings: Ptr, enabled: Bool);
    pub fn webkit_settings_set_enable_smooth_scrolling(settings: Ptr, enabled: Bool);
    pub fn webkit_settings_set_enable_media(settings: Ptr, enabled: Bool);
    pub fn webkit_settings_set_enable_media_stream(settings: Ptr, enabled: Bool);
    pub fn webkit_settings_set_enable_mediasource(settings: Ptr, enabled: Bool);
    pub fn webkit_settings_set_enable_javascript(settings: Ptr, enabled: Bool);
    pub fn webkit_settings_set_enable_javascript_markup(settings: Ptr, enabled: Bool);
    pub fn webkit_settings_set_enable_html5_local_storage(settings: Ptr, enabled: Bool);
    pub fn webkit_settings_set_enable_developer_extras(settings: Ptr, enabled: Bool);
    pub fn webkit_settings_set_javascript_can_open_windows_automatically(
        settings: Ptr,
        enabled: Bool,
    );
    pub fn webkit_settings_set_media_playback_requires_user_gesture(settings: Ptr, required: Bool);
    pub fn webkit_settings_set_default_font_family(settings: Ptr, family: *const c_char);
    pub fn webkit_settings_set_default_font_size(settings: Ptr, size: c_uint);
    pub fn webkit_settings_set_default_monospace_font_size(settings: Ptr, size: c_uint);
    pub fn webkit_permission_request_allow(request: Ptr);
    pub fn webkit_permission_request_deny(request: Ptr);
    pub fn webkit_script_dialog_get_dialog_type(dialog: Ptr) -> c_int;
    pub fn webkit_script_dialog_get_message(dialog: Ptr) -> *const c_char;
    pub fn webkit_script_dialog_confirm_set_confirmed(dialog: Ptr, confirmed: Bool);
    pub fn webkit_script_dialog_close(dialog: Ptr);
    pub fn webkit_script_dialog_ref(dialog: Ptr) -> Ptr;
    pub fn webkit_script_dialog_unref(dialog: Ptr);
    pub fn webkit_user_media_permission_request_get_type() -> GType;
    pub fn webkit_user_media_permission_is_for_audio_device(request: Ptr) -> Bool;
    pub fn webkit_user_media_permission_is_for_video_device(request: Ptr) -> Bool;
    pub fn webkit_user_media_permission_is_for_display_device(request: Ptr) -> Bool;
    pub fn webkit_notification_permission_request_get_type() -> GType;
    pub fn webkit_geolocation_permission_request_get_type() -> GType;
    pub fn webkit_device_info_permission_request_get_type() -> GType;
    pub fn webkit_clipboard_permission_request_get_type() -> GType;
    pub fn webkit_policy_decision_use(decision: Ptr);
    pub fn webkit_policy_decision_ignore(decision: Ptr);
    pub fn webkit_policy_decision_download(decision: Ptr);
    pub fn webkit_navigation_policy_decision_get_navigation_action(decision: Ptr) -> Ptr;
    pub fn webkit_navigation_action_get_request(action: Ptr) -> Ptr;
    pub fn webkit_navigation_action_is_user_gesture(action: Ptr) -> Bool;
    pub fn webkit_response_policy_decision_is_mime_type_supported(decision: Ptr) -> Bool;
    pub fn webkit_uri_request_get_uri(request: Ptr) -> *const c_char;
    pub fn webkit_uri_response_get_suggested_filename(response: Ptr) -> *const c_char;
    pub fn webkit_download_get_request(download: Ptr) -> Ptr;
    pub fn webkit_download_get_response(download: Ptr) -> Ptr;
    pub fn webkit_download_cancel(download: Ptr);
    pub fn webkit_download_set_destination(download: Ptr, uri: *const c_char);
    pub fn webkit_download_set_allow_overwrite(download: Ptr, allowed: Bool);
    pub fn webkit_web_view_evaluate_javascript(
        view: Ptr,
        script: *const c_char,
        length: isize,
        world_name: *const c_char,
        source_uri: *const c_char,
        cancellable: Ptr,
        callback: GAsyncReadyCallback,
        data: Ptr,
    );
    pub fn webkit_web_view_evaluate_javascript_finish(
        view: Ptr,
        result: Ptr,
        error: *mut *mut GError,
    ) -> Ptr;
}

#[link(
    name = "libjavascriptcoregtk-4.1.so.0",
    kind = "dylib",
    modifiers = "+verbatim"
)]
extern "C" {
    pub fn jsc_value_to_string(value: Ptr) -> *mut c_char;
    pub fn jsc_value_to_json(value: Ptr, indent: c_uint) -> *mut c_char;
    pub fn jsc_value_to_boolean(value: Ptr) -> Bool;
}

#[link(name = "libcairo.so.2", kind = "dylib", modifiers = "+verbatim")]
extern "C" {
    pub fn cairo_image_surface_create(format: c_int, width: c_int, height: c_int) -> Ptr;
    pub fn cairo_create(surface: Ptr) -> Ptr;
    pub fn cairo_surface_write_to_png(surface: Ptr, filename: *const c_char) -> c_int;
    pub fn cairo_surface_destroy(surface: Ptr);
    pub fn cairo_destroy(cairo: Ptr);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn public_c_records_keep_their_abi_layout() {
        assert_eq!(size_of::<Rgba>(), 32);
        assert_eq!(offset_of!(Rgba, red), 0);
        assert_eq!(offset_of!(Rgba, green), 8);
        assert_eq!(offset_of!(Rgba, blue), 16);
        assert_eq!(offset_of!(Rgba, alpha), 24);
        assert_eq!(offset_of!(GError, domain), 0);
        assert_eq!(offset_of!(GError, code), 4);
        assert_eq!(offset_of!(GError, message), 8);
        assert_eq!(size_of::<GError>(), 8 + size_of::<Ptr>());
    }

    #[test]
    fn linked_runtime_exposes_the_expected_public_api_family() {
        // Version accessors neither initialize GTK nor create any processes.
        unsafe {
            assert_eq!(gtk_get_major_version(), 3);
            assert!(gtk_get_minor_version() >= 24);
            assert_eq!(webkit_get_major_version(), 2);
            assert!(webkit_get_minor_version() >= 40);
        }
    }
}
