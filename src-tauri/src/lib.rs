mod capture;
mod color;
mod cursor;

use color::{ColorEntry, ColorInfo};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

// ── Logging helper ───────────────────────────────────────────────────────────
static LOG_PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

fn init_log() {
    // /private/tmp は macOS でシンボリックリンク先。GUIアプリからも書ける
    let path = std::path::PathBuf::from("/private/tmp/pixellens_debug.log");
    let _ = std::fs::write(&path, format!("[PixelLens] log started at {:?}\n",
        std::time::SystemTime::now()));
    LOG_PATH.set(path).ok();
}

pub fn write_log(msg: &str) {
    eprintln!("[PixelLens] {}", msg);
    if let Some(path) = LOG_PATH.get() {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(path) {
            let _ = writeln!(f, "[PixelLens] {}", msg);
        }
    }
}

macro_rules! log {
    ($($arg:tt)*) => {{
        write_log(&format!($($arg)*));
    }};
}

// キャプチャエラー / カーソルログを最初の N 回だけ出す
static CAPTURE_ERR_COUNT: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);
static CURSOR_ZERO_COUNT: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);
const CAPTURE_ERR_LOG_MAX: u32 = 3;
const CURSOR_ZERO_LOG_MAX: u32 = 1;

pub struct AppState {
    pub color_dict: Mutex<Vec<ColorEntry>>,
    pub settings: Mutex<Settings>,
    /// macOS CGWindowID of our main window (cached at startup, 0 = unknown).
    pub macos_window_id: Mutex<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub zoom_level: u8,
    pub use_jis_colors: bool,
    pub shortcut: String,
    pub copy_shortcut: String,
    pub copy_format: String,
    pub theme: String,
    pub show_grid: bool,
    pub language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            zoom_level: 10,
            use_jis_colors: true,
            shortcut: "CommandOrControl+Alt+C".to_string(),
            copy_shortcut: "Ctrl+Shift+C".to_string(),
            copy_format: "hex".to_string(),
            theme: "dark".to_string(),
            show_grid: true,
            language: "en".to_string(),
        }
    }
}

// ── Settings persistence ─────────────────────────────────────────────────────

fn settings_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("settings.json"))
}

fn load_settings_from_disk(app: &tauri::App) -> Settings {
    let handle = app.handle();
    if let Some(path) = settings_path(handle) {
        if let Ok(contents) = std::fs::read_to_string(&path) {
            if let Ok(s) = serde_json::from_str::<Settings>(&contents) {
                log!("settings loaded from {:?}", path);
                return s;
            }
        }
    }
    Settings::default()
}

fn save_settings_to_disk(app: &AppHandle, settings: &Settings) {
    if let Some(path) = settings_path(app) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(settings) {
            let _ = std::fs::write(&path, json);
            log!("settings saved to {:?}", path);
        }
    }
}

// ── Logging command (callable from JS) ───────────────────────────────────────

#[tauri::command]
fn js_log(level: String, msg: String) {
    write_log(&format!("[JS:{}] {}", level, msg));
}

// ── Tauri commands ───────────────────────────────────────────────────────────

#[tauri::command]
fn get_cursor_pos() -> Result<cursor::CursorPos, String> {
    let result = cursor::get_cursor_pos();
    match &result {
        Ok(pos) => {
            if pos.x == 0 && pos.y == 0 {
                let n = CURSOR_ZERO_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if n < CURSOR_ZERO_LOG_MAX {
                    log!("get_cursor_pos -> x=0 y=0 (以降の (0,0) ログは抑制)");
                }
            } else {
                // per-tick pos log omitted in production
            }
        }
        Err(e) => log!("get_cursor_pos ERROR: {}", e),
    }
    result
}

#[tauri::command]
fn capture_area(
    cx: i32,
    cy: i32,
    size: u32,
    state: State<AppState>,
    window: tauri::WebviewWindow,
) -> Result<PixelData, String> {
    // On macOS: decide whether to exclude the PixelLens window from the capture.
    // When the cursor is *outside* our window we exclude ourselves so we don't
    // occlude content behind us.  When the cursor is *inside* our window the
    // user may intentionally be sampling PixelLens's own colors, so we include
    // all windows (exclude_window_id = 0).
    let exclude_window_id: u32 = {
        #[cfg(target_os = "macos")]
        {
            let stored_id = *state.macos_window_id.lock().unwrap();
            // outer_position / outer_size are in PHYSICAL pixels (PhysicalPosition / PhysicalSize).
            // The cursor coordinates cx/cy from CGEvent.location() are in LOGICAL pixels (points).
            // On Retina (2× scale) we must divide the physical values by scale_factor before
            // comparing, otherwise the "cursor inside window?" check is always wrong.
            let scale = window.scale_factor().unwrap_or(1.0);
            let win_pos  = window.outer_position().ok();
            let win_size = window.outer_size().ok();
            let over_self = win_pos.zip(win_size)
                .map(|(pos, sz)| {
                    let lx0 = (pos.x as f64 / scale) as i32;
                    let ly0 = (pos.y as f64 / scale) as i32;
                    let lx1 = lx0 + (sz.width  as f64 / scale) as i32;
                    let ly1 = ly0 + (sz.height as f64 / scale) as i32;
                    cx >= lx0 && cx < lx1 && cy >= ly0 && cy < ly1
                })
                .unwrap_or(false);
            if over_self {
                0
            } else {
                stored_id
            }
        }
        #[cfg(not(target_os = "macos"))]
        { 0 }
    };

    let capture = capture::capture_area(cx, cy, size, exclude_window_id).map_err(|e| {
        let n = CAPTURE_ERR_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if n < CAPTURE_ERR_LOG_MAX {
            log!("capture_area ERROR: {}", e);
        } else if n == CAPTURE_ERR_LOG_MAX {
            log!("capture_area: 以降のエラーは抑制します (WSL2/Xwayland制限の可能性)");
        }
        e
    })?;
    let dict = state.color_dict.lock().unwrap();
    let color = ColorInfo::from_rgb(capture.center_r, capture.center_g, capture.center_b, &dict);
    Ok(PixelData {
        image_b64: capture.image_b64,
        width: capture.width,
        height: capture.height,
        color,
        cursor_x: cx,
        cursor_y: cy,
    })
}

#[tauri::command]
fn get_color_at(r: u8, g: u8, b: u8, state: State<AppState>) -> ColorInfo {
    let dict = state.color_dict.lock().unwrap();
    ColorInfo::from_rgb(r, g, b, &dict)
}

#[tauri::command]
fn hide_window(window: tauri::WebviewWindow) {
    let _ = window.hide();
}

#[tauri::command]
fn start_drag(window: tauri::WebviewWindow) -> Result<(), String> {
    window.start_dragging().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(settings: Settings, state: State<AppState>, app_handle: AppHandle) -> Result<(), String> {
    save_settings_to_disk(&app_handle, &settings);
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PixelData {
    pub image_b64: String,
    pub width: u32,
    pub height: u32,
    pub color: ColorInfo,
    pub cursor_x: i32,
    pub cursor_y: i32,
}

// ── App entry point ──────────────────────────────────────────────────────────

// macOS native tray: store AppHandle globally so the C callback can access it.
#[cfg(target_os = "macos")]
static APP_HANDLE_FOR_TRAY: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

/// C-compatible callback invoked by ObjC on left-click of the native tray icon.
#[cfg(target_os = "macos")]
extern "C" fn tray_toggle_window_cb() {
    if let Some(h) = APP_HANDLE_FOR_TRAY.get() {
        toggle_window(h);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_log();
    log!("=== PixelLens starting ===");
    let color_dict_json = include_str!("../../ui/color-dictionary.json");
    let color_dict = color::load_dictionary(color_dict_json).unwrap_or_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .manage(AppState {
            color_dict: Mutex::new(color_dict),
            settings: Mutex::new(Settings::default()), // overwritten in setup
            macos_window_id: Mutex::new(0),
        })
        .on_page_load(|window, payload| {
            use tauri::webview::PageLoadEvent;
            match payload.event() {
                PageLoadEvent::Started  => eprintln!("[PixelLens] WebView page load STARTED  url={}", payload.url()),
                PageLoadEvent::Finished => eprintln!("[PixelLens] WebView page load FINISHED url={}", payload.url()),
            }
            let _ = window;
        })
        .setup(|app| {
            log!("setup: begin");

            // Load persisted settings
            let saved = load_settings_from_disk(app);
            *app.state::<AppState>().settings.lock().unwrap() = saved;

            // macOS: use native NSStatusItem (Tauri tray API broken on macOS 26)
            #[cfg(target_os = "macos")]
            {
                APP_HANDLE_FOR_TRAY.set(app.handle().clone()).ok();
                unsafe { capture::sc_setup_native_tray(tray_toggle_window_cb) };
            }
            // Windows (and future non-Linux, non-macOS): use Tauri tray
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            setup_tray(app)?;
            setup_shortcut(app)?;
            log!("setup: shortcut registered");
            // macOS: log TCC state for diagnostics only.
            // CGPreflightScreenCaptureAccess() always returns false on macOS 26
            // because System Settings now controls a DIFFERENT TCC service.
            // We do NOT call CGRequestScreenCaptureAccess() here — it would
            // open System Settings on every launch (infinite loop).
            // SCKit handles its own permission errors at capture time.
            #[cfg(target_os = "macos")]
            {
                let preflight = unsafe {
                    capture::macos_ffi::CGPreflightScreenCaptureAccess()
                };
                log!("setup: CGPreflight(old TCC service) = {} (always false on macOS 26; irrelevant)", preflight);
            }

            if let Some(w) = app.get_webview_window("main") {
                log!("setup: showing window");
                let _ = w.show();
                let _ = w.set_focus();
                // macOS: cache the CGWindowID of our main window so we can
                // exclude it from CGWindowListCreateImageFromArray captures.
                #[cfg(target_os = "macos")]
                {
                    use objc::{msg_send, sel, sel_impl};

                    // Get the native NSWindow pointer from Tauri and read windowNumber.
                    // Using ns_window() is more reliable than NSApp.mainWindow which
                    // can be nil if the window hasn't received focus yet.
                    if let Ok(ns_win_ptr) = w.ns_window() {
                        let win_id: u32 = unsafe {
                            let ns_win = ns_win_ptr as *mut objc::runtime::Object;
                            let num: i64 = msg_send![ns_win, windowNumber];
                            num as u32
                        };
                        *app.state::<AppState>().macos_window_id.lock().unwrap() = win_id;
                        log!("setup: macOS window ID = {} (ns_window)", win_id);
                    } else {
                        // Fallback: try NSApp.mainWindow
                        let win_id = capture::get_main_window_id();
                        *app.state::<AppState>().macos_window_id.lock().unwrap() = win_id;
                        log!("setup: macOS window ID = {} (mainWindow fallback)", win_id);
                    }
                }
            }
            log!("setup: done");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_cursor_pos,
            capture_area,
            get_color_at,
            get_settings,
            save_settings,
            hide_window,
            start_drag,
            js_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// setup_tray is used on Windows (and future non-macOS, non-Linux platforms).
// macOS uses sc_setup_native_tray (native NSStatusItem) instead — Tauri's
// TrayIconBuilder is broken on macOS 26 (clicks not delivered without a menu,
// and attaching a menu prevents left-click toggle).
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let quit_item = MenuItem::with_id(app, "quit", "PixelLens を終了", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&quit_item])?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("PixelLens — 左クリックで表示/非表示")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event {
                toggle_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn setup_shortcut(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, ShortcutState, Shortcut};

    // Ctrl+Alt+C — ウィンドウ表示/非表示
    let show_shortcut = Shortcut::new(
        Some(Modifiers::CONTROL | Modifiers::ALT),
        Code::KeyC,
    );
    let app_handle = app.handle().clone();
    app.global_shortcut().on_shortcut(show_shortcut, move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            toggle_window(&app_handle);
        }
    })?;

    // Ctrl+Shift+C — クイックコピー
    let copy_shortcut = Shortcut::new(
        Some(Modifiers::CONTROL | Modifiers::SHIFT),
        Code::KeyC,
    );
    let app_handle2 = app.handle().clone();
    app.global_shortcut().on_shortcut(copy_shortcut, move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            let _ = app_handle2.emit("quick-copy", ());
        }
    })?;

    // Ctrl+Shift+Alt+C — カラーロック（Pick）トグル
    // カーソルが目的のピクセル上にある状態で押すと色を確定・ロック。
    // ロック中はコピーボタンがロックした色を返す（カーソル移動の影響を受けない）。
    // 再度押すとロック解除してリアルタイム表示に戻る。
    let lock_shortcut = Shortcut::new(
        Some(Modifiers::CONTROL | Modifiers::SHIFT | Modifiers::ALT),
        Code::KeyC,
    );
    let app_handle3 = app.handle().clone();
    app.global_shortcut().on_shortcut(lock_shortcut, move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            log!("shortcut: Ctrl+Shift+Alt+C → toggle-lock");
            let _ = app_handle3.emit("toggle-lock", ());
        }
    })?;

    Ok(())
}

fn toggle_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let is_minimized_raw = window.is_minimized();
        let is_visible_raw   = window.is_visible();
        let is_minimized = is_minimized_raw.as_ref().copied().unwrap_or(false);
        let is_visible   = is_visible_raw.as_ref().copied().unwrap_or(false);
        log!("toggle_window: is_minimized={:?}({}) is_visible={:?}({})",
            is_minimized_raw, is_minimized,
            is_visible_raw, is_visible);
        if is_visible && !is_minimized {
            log!("toggle_window: → hide");
            let r = window.hide();
            log!("toggle_window: hide result={:?}", r);
        } else {
            if is_minimized {
                log!("toggle_window: → unminimize");
                let r = window.unminimize();
                log!("toggle_window: unminimize result={:?}", r);
            }
            log!("toggle_window: → show + focus");
            let r1 = window.show();
            let r2 = window.set_focus();
            log!("toggle_window: show={:?} focus={:?}", r1, r2);
        }
    } else {
        log!("toggle_window: window 'main' not found!");
    }
}
