use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPos {
    pub x: i32,
    pub y: i32,
}

#[cfg(target_os = "windows")]
pub fn get_cursor_pos() -> Result<CursorPos, String> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut point = POINT { x: 0, y: 0 };
    unsafe {
        GetCursorPos(&mut point).map_err(|e| e.to_string())?;
    }
    Ok(CursorPos {
        x: point.x,
        y: point.y,
    })
}

#[cfg(target_os = "macos")]
pub fn get_cursor_pos() -> Result<CursorPos, String> {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Failed to create event source".to_string())?;
    let event =
        CGEvent::new(source).map_err(|_| "Failed to create CGEvent".to_string())?;
    let loc = event.location();
    Ok(CursorPos {
        x: loc.x as i32,
        y: loc.y as i32,
    })
}

#[cfg(target_os = "linux")]
pub fn get_cursor_pos() -> Result<CursorPos, String> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConnectionExt;
    use x11rb::rust_connection::RustConnection;

    let (conn, screen_num) =
        RustConnection::connect(None).map_err(|e| e.to_string())?;
    let root = conn.setup().roots[screen_num].root;
    let reply = conn
        .query_pointer(root)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?;

    // WSL2/Xwayland では root_x/root_y が常に 0,0 になる場合がある
    // same_screen=false はカーソルが X ディスプレイ外にいることを示す
    if !reply.same_screen || (reply.root_x == 0 && reply.root_y == 0) {
        // ウィンドウ相対座標 (win_x, win_y) にフォールバック
        let wx = reply.win_x;
        let wy = reply.win_y;
        if wx != 0 || wy != 0 {
            return Ok(CursorPos { x: wx as i32, y: wy as i32 });
        }
    }

    Ok(CursorPos {
        x: reply.root_x as i32,
        y: reply.root_y as i32,
    })
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn get_cursor_pos() -> Result<CursorPos, String> {
    Err("Unsupported platform".to_string())
}
