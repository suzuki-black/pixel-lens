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
pub fn capture_area(cx: i32, cy: i32, size: u32) -> Result<CaptureResult, String> {
    let half = (size / 2) as i32;
    let left = cx - half;
    let top = cy - half;

    let pixels = capture_screen_region(left, top, size, size)?;

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
fn capture_screen_region(x: i32, y: i32, w: u32, h: u32) -> Result<Vec<u8>, String> {
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

#[cfg(target_os = "macos")]
fn capture_screen_region(x: i32, y: i32, w: u32, h: u32) -> Result<Vec<u8>, String> {
    use core_graphics::display::{CGDisplay, CGRect, CGPoint, CGSize};
    use core_graphics::image::CGImage;

    let rect = CGRect::new(
        &CGPoint::new(x as f64, y as f64),
        &CGSize::new(w as f64, h as f64),
    );
    let image: CGImage = CGDisplay::screenshot(rect, 0, 0, 0)
        .ok_or("CGDisplay::screenshot failed")?;

    let width = image.width();
    let height = image.height();
    let data = image.data();
    let bytes = data.bytes();

    // macOS returns BGRA; convert to RGBA
    let mut buf: Vec<u8> = bytes.to_vec();
    for chunk in buf.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }

    Ok(buf)
}

#[cfg(target_os = "linux")]
fn capture_screen_region(x: i32, y: i32, w: u32, h: u32) -> Result<Vec<u8>, String> {
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
fn capture_screen_region(_x: i32, _y: i32, _w: u32, _h: u32) -> Result<Vec<u8>, String> {
    Err("Unsupported platform".into())
}
