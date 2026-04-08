mod capture;
mod color;
mod cursor;

use color::{ColorEntry, ColorInfo};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

// ── Logging helper ───────────────────────────────────────────────────────────
macro_rules! log {
    ($($arg:tt)*) => {{
        eprintln!("[PixelLens] {}", format!($($arg)*));
    }};
}

// キャプチャエラーのログを最初の N 回だけ出す
static CAPTURE_ERR_COUNT: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);
const CAPTURE_ERR_LOG_MAX: u32 = 3;

pub struct AppState {
    pub color_dict: Mutex<Vec<ColorEntry>>,
    pub settings: Mutex<Settings>,
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
        }
    }
}

// ── Logging command (callable from JS) ───────────────────────────────────────

#[tauri::command]
fn js_log(level: String, msg: String) {
    eprintln!("[JS:{}] {}", level, msg);
}

// ── Tauri commands ───────────────────────────────────────────────────────────

#[tauri::command]
fn get_cursor_pos() -> Result<cursor::CursorPos, String> {
    let result = cursor::get_cursor_pos();
    match &result {
        Ok(pos) => log!("get_cursor_pos -> x={} y={}", pos.x, pos.y),
        Err(e)  => log!("get_cursor_pos ERROR: {}", e),
    }
    result
}

#[tauri::command]
fn capture_area(
    cx: i32,
    cy: i32,
    size: u32,
    state: State<AppState>,
) -> Result<PixelData, String> {
    let capture = capture::capture_area(cx, cy, size).map_err(|e| {
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
    log!("capture_area OK center=({},{},{}) hex={}", capture.center_r, capture.center_g, capture.center_b, color.hex);

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
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(settings: Settings, state: State<AppState>) -> Result<(), String> {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let color_dict_json = include_str!("../../ui/color-dictionary.json");
    let color_dict = color::load_dictionary(color_dict_json).unwrap_or_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .manage(AppState {
            color_dict: Mutex::new(color_dict),
            settings: Mutex::new(Settings::default()),
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
            #[cfg(not(target_os = "linux"))]
            setup_tray(app)?;
            setup_shortcut(app)?;
            log!("setup: shortcut registered");
            // 開発中は全プラットフォームでウィンドウを即時表示
            if let Some(w) = app.get_webview_window("main") {
                log!("setup: showing window");
                let _ = w.show();
                let _ = w.set_focus();
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
            js_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(not(target_os = "linux"))]
fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    // 右クリックメニューは「終了」のみ。左クリックはウィンドウトグル専用。
    let quit_item = MenuItem::with_id(app, "quit", "PixelLens を終了", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&quit_item])?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .menu_on_left_click(false)   // 左クリックではメニューを出さない
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
            } = event
            {
                toggle_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn setup_shortcut(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

    // Ctrl+Alt+C — ウィンドウ表示/非表示
    let show_shortcut = Shortcut::new(
        Some(Modifiers::CONTROL | Modifiers::ALT),
        Code::KeyC,
    );
    let app_handle = app.handle().clone();
    app.global_shortcut().on_shortcut(show_shortcut, move |_app, _shortcut, _event| {
        toggle_window(&app_handle);
    })?;

    // Ctrl+Shift+C — クイックコピー
    let copy_shortcut = Shortcut::new(
        Some(Modifiers::CONTROL | Modifiers::SHIFT),
        Code::KeyC,
    );
    let app_handle2 = app.handle().clone();
    app.global_shortcut().on_shortcut(copy_shortcut, move |_app, _shortcut, _event| {
        let _ = app_handle2.emit("quick-copy", ());
    })?;

    Ok(())
}

fn toggle_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}
