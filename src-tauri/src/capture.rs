use image::{ImageBuffer, Rgba};
use serde::{Deserialize, Serialize};

/// Raw RGBA pixel data with dimensions, encoded as base64 PNG for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureResult {
    /// base64-encoded PNG of the captured region
    pub image_b64: String,
    pub width: u32,
    pub height: u32,
    /// The pixel color at the exact center of the captured region
    pub center_r: u8,
    pub center_g: u8,
    pub center_b: u8,
}

/// Capture a square region of `size` pixels centered on (cx, cy).
///
/// `exclude_window_id` — macOS CGWindowID of our own window.
///   * `> 0` : cursor is **outside** the PixelLens window → exclude PixelLens
///             from the capture so it doesn't obscure content behind it.
///   * `0`   : cursor is **inside** the PixelLens window → include all windows
///             so the caller can sample PixelLens's own colors.
pub fn capture_area(cx: i32, cy: i32, size: u32, exclude_window_id: u32) -> Result<CaptureResult, String> {
    let half = (size / 2) as i32;
    let left = cx - half;
    let top = cy - half;

    let pixels = capture_screen_region(left, top, size, size, exclude_window_id)?;

    // Encode to PNG base64
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(size, size, pixels.clone())
            .ok_or("Failed to create image buffer")?;

    let mut png_bytes: Vec<u8> = Vec::new();
    {
        use image::codecs::png::PngEncoder;
        use image::ImageEncoder;
        let encoder = PngEncoder::new(&mut png_bytes);
        encoder
            .write_image(
                img.as_raw(),
                size,
                size,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| e.to_string())?;
    }

    use base64::Engine;
    let image_b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);

    // Center pixel
    let center_idx = ((size / 2 * size + size / 2) * 4) as usize;
    let (cr, cg, cb) = if center_idx + 2 < pixels.len() {
        (pixels[center_idx], pixels[center_idx + 1], pixels[center_idx + 2])
    } else {
        (0, 0, 0)
    };

    Ok(CaptureResult {
        image_b64,
        width: size,
        height: size,
        center_r: cr,
        center_g: cg,
        center_b: cb,
    })
}

// ── Platform implementations ────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn capture_screen_region(x: i32, y: i32, w: u32, h: u32, _exclude_window_id: u32) -> Result<Vec<u8>, String> {
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
        GetDIBits, GetDC, ReleaseDC, SelectObject, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;

    unsafe {
        let hwnd = GetDesktopWindow();
        let hdc_screen = GetDC(hwnd);
        if hdc_screen.is_invalid() {
            return Err("GetDC failed".into());
        }

        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let hbmp = CreateCompatibleBitmap(hdc_screen, w as i32, h as i32);
        let _old = SelectObject(hdc_mem, hbmp);

        BitBlt(hdc_mem, 0, 0, w as i32, h as i32, hdc_screen, x, y, SRCCOPY)
            .map_err(|e| e.to_string())?;

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32), // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            ..Default::default()
        };

        let pixel_count = (w * h * 4) as usize;
        let mut buf: Vec<u8> = vec![0u8; pixel_count];

        GetDIBits(
            hdc_mem,
            hbmp,
            0,
            h,
            Some(buf.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        // Windows returns BGRA; convert to RGBA
        for chunk in buf.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }

        let _ = DeleteObject(hbmp);
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(hwnd, hdc_screen);

        Ok(buf)
    }
}

// ── macOS ─────────────────────────────────────────────────────────────────────
// Screen capture on macOS uses ScreenCaptureKit (SCKit).
// On macOS 26, SCKit requires authorization via SCContentSharingPicker.
// Call sc_show_picker() once (e.g. via tray menu) to present the picker;
// after the user selects a display, captures succeed via SCKit.
//
// CGWindowListCreateImage was removed: it is unavailable in the macOS 26 SDK
// and only returned the desktop wallpaper (old TCC service cannot be granted).
//
// The ObjC shim in capture_helper.m implements the capture logic and is
// compiled by build.rs via the `cc` crate.

#[cfg(target_os = "macos")]
pub mod macos_ffi {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        /// Returns true if the caller has the OLD screen-recording TCC permission.
        /// NOTE: On macOS 26, System Settings controls a DIFFERENT service,
        /// so this always returns false regardless of user settings.
        /// Used for diagnostic logging only.
        pub fn CGPreflightScreenCaptureAccess() -> bool;
    }
}

// FFI declarations for capture_helper.m (compiled by build.rs via `cc` crate).
#[cfg(target_os = "macos")]
extern "C" {
    /// Capture `rect` (logical coords) via ScreenCaptureKit (primary) or
    /// CGWindowListCreateImage (fallback).
    /// exclude_win_id: CGWindowID of PixelLens own window (> 0) to exclude it
    ///   from the composite, or 0 to include all windows.
    /// Returns a malloc'd RGBA buffer of size (*out_w * *out_h * 4), or NULL.
    fn sc_capture_rect_rgba(
        x: f64, y: f64, w: f64, h: f64,
        exclude_win_id: u32,
        out_w: *mut u32,
        out_h: *mut u32,
    ) -> *mut u8;

    /// Free a buffer returned by sc_capture_rect_rgba.
    fn sc_free_buffer(buf: *mut u8);

    /// Present SCContentSharingPicker so the user can authorize screen capture.
    /// On macOS 26, this is the ONLY way to get TCC authorization for SCKit.
    /// After the user picks a display, SCKit is re-enabled automatically.
    /// Safe to call from any thread (dispatches to main queue internally).
    pub fn sc_show_picker();
}

/// Returns the NSWindow number (z-order ID) of the app's main window.
#[cfg(target_os = "macos")]
pub fn get_main_window_id() -> u32 {
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let app: *mut objc::runtime::Object =
            msg_send![class!(NSApplication), sharedApplication];
        let win: *mut objc::runtime::Object = msg_send![app, mainWindow];
        if win.is_null() {
            return 0;
        }
        let num: i64 = msg_send![win, windowNumber];
        num as u32
    }
}

#[cfg(target_os = "macos")]
fn capture_screen_region(x: i32, y: i32, w: u32, h: u32, exclude_window_id: u32) -> Result<Vec<u8>, String> {
    // Log every capture (rate-limited to once per 60 frames to avoid flooding).
    static CGWL_OK_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = CGWL_OK_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let should_log = n < 5 || n % 60 == 0;
    if should_log {
        let has_perm = unsafe { macos_ffi::CGPreflightScreenCaptureAccess() };
        crate::write_log(&format!("capture[{}] perm(old-TCC)={} rect=({},{},{},{}) excl={}",
            n, has_perm, x, y, w, h, exclude_window_id));
    }

    let mut actual_w: u32 = 0;
    let mut actual_h: u32 = 0;

    let ptr = unsafe {
        sc_capture_rect_rgba(
            x as f64, y as f64, w as f64, h as f64,
            exclude_window_id,
            &mut actual_w, &mut actual_h,
        )
    };

    if ptr.is_null() {
        crate::write_log("SCKit: sc_capture_rect_rgba returned NULL (not authorized? Use tray menu to grant permission)");
        return Err("SCKit capture returned NULL — use tray menu to grant Screen Recording permission".into());
    }

    if should_log {
        crate::write_log(&format!("capture[{}]: OK actual={}x{} logical={}x{}", n, actual_w, actual_h, w, h));
    }

    // Wrap the malloc'd buffer in a Vec for safe handling.
    let byte_len = (actual_w * actual_h * 4) as usize;
    let buf: Vec<u8> = unsafe {
        let slice = std::slice::from_raw_parts(ptr, byte_len);
        let v = slice.to_vec();
        sc_free_buffer(ptr);
        v
    };

    // Retina: scale physical pixels down to logical size if needed.
    if actual_w == w && actual_h == h {
        return Ok(buf);
    }
    let img = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(actual_w, actual_h, buf)
        .ok_or("macOS CGWL: failed to create ImageBuffer for scaling")?;
    let scaled = image::imageops::resize(&img, w, h, image::imageops::FilterType::Nearest);
    Ok(scaled.into_raw())
}

#[cfg(target_os = "linux")]
fn capture_screen_region(x: i32, y: i32, w: u32, h: u32, _exclude_window_id: u32) -> Result<Vec<u8>, String> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};
    use x11rb::rust_connection::RustConnection;

    let (conn, screen_num) = RustConnection::connect(None)
        .map_err(|e| format!("X11 connect: {e}"))?;

    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let sw = screen.width_in_pixels as i32;
    let sh = screen.height_in_pixels as i32;

    // クリッピング: 画面範囲外を切り詰め
    let x0 = x.max(0).min(sw - 1);
    let y0 = y.max(0).min(sh - 1);
    let cap_w = ((w as i32).min(sw - x0)).max(1) as u16;
    let cap_h = ((h as i32).min(sh - y0)).max(1) as u16;

    let reply = conn
        .get_image(ImageFormat::Z_PIXMAP, root, x0 as i16, y0 as i16, cap_w, cap_h, !0u32)
        .map_err(|e| format!("GetImage req: {e}"))?
        .reply()
        .map_err(|e| format!("GetImage reply: {e}"))?;

    // bits_per_pixel を pixmap formats から取得
    let bpp = conn
        .setup()
        .pixmap_formats
        .iter()
        .find(|f| f.depth == reply.depth)
        .map(|f| f.bits_per_pixel)
        .unwrap_or(32);

    let data = &reply.data;
    let mut rgba = vec![0u8; w as usize * h as usize * 4];

    match bpp {
        32 => {
            // BGRX (little-endian) → RGBA
            let row_bytes = cap_w as usize * 4;
            for row in 0..(cap_h as usize).min(h as usize) {
                for col in 0..(cap_w as usize).min(w as usize) {
                    let s = row * row_bytes + col * 4;
                    let d = (row * w as usize + col) * 4;
                    if s + 3 < data.len() {
                        rgba[d]     = data[s + 2]; // R
                        rgba[d + 1] = data[s + 1]; // G
                        rgba[d + 2] = data[s];     // B
                        rgba[d + 3] = 255;
                    }
                }
            }
        }
        24 => {
            // BGR + padding per scanline (32-bit pad)
            let pad = conn.setup().bitmap_format_scanline_pad as usize;
            let row_bits = cap_w as usize * 24;
            let row_bytes = (row_bits + pad - 1) / pad * (pad / 8);
            for row in 0..(cap_h as usize).min(h as usize) {
                for col in 0..(cap_w as usize).min(w as usize) {
                    let s = row * row_bytes + col * 3;
                    let d = (row * w as usize + col) * 4;
                    if s + 2 < data.len() {
                        rgba[d]     = data[s + 2]; // R
                        rgba[d + 1] = data[s + 1]; // G
                        rgba[d + 2] = data[s];     // B
                        rgba[d + 3] = 255;
                    }
                }
            }
        }
        _ => return Err(format!("Unsupported bpp: {bpp}")),
    }

    Ok(rgba)
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn capture_screen_region(_x: i32, _y: i32, _w: u32, _h: u32, _exclude_window_id: u32) -> Result<Vec<u8>, String> {
    Err("Unsupported platform".into())
}
