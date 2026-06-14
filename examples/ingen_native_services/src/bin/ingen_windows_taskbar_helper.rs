use std::ffi::{c_void, OsStr};
use std::io::{self, BufRead, Write};
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null;
use std::time::Instant;

const ABM_GETSTATE: u32 = 0x0000_0004;
const ABM_SETSTATE: u32 = 0x0000_000A;
const ABS_AUTOHIDE: usize = 0x0000_0001;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AppBarData {
    cb_size: u32,
    hwnd: *mut c_void,
    callback_message: u32,
    edge: u32,
    rect: Rect,
    l_param: isize,
}

impl Default for AppBarData {
    fn default() -> Self {
        Self {
            cb_size: size_of::<AppBarData>() as u32,
            hwnd: std::ptr::null_mut(),
            callback_message: 0,
            edge: 0,
            rect: Rect::default(),
            l_param: 0,
        }
    }
}

#[link(name = "shell32")]
extern "system" {
    fn SHAppBarMessage(message: u32, data: *mut AppBarData) -> usize;
}

#[link(name = "user32")]
extern "system" {
    fn FindWindowW(class_name: *const u16, window_name: *const u16) -> *mut c_void;
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(std::iter::once(0)).collect()
}

fn emit_json(ok: bool, command: &str, hidden: bool, current: usize, target: usize, elapsed_ms: u128, error: &str) {
    let escaped_error = error.replace('\\', "\\\\").replace('"', "\\\"");
    println!(
        "{{\"ok\":{},\"command\":\"{}\",\"hidden\":{},\"current\":{},\"target\":{},\"elapsedMs\":{},\"error\":\"{}\"}}",
        ok, command, hidden, current, target, elapsed_ms, escaped_error
    );
    let _ = io::stdout().flush();
}

fn set_taskbar_hidden(hidden: bool) -> Result<(usize, usize), String> {
    let class_name = wide("Shell_TrayWnd");
    let hwnd = unsafe { FindWindowW(class_name.as_ptr(), null()) };
    if hwnd.is_null() {
        return Err("Shell_TrayWnd was not found".to_string());
    }

    let mut data = AppBarData {
        hwnd,
        ..AppBarData::default()
    };
    let current = unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) };
    let target = if hidden {
        current | ABS_AUTOHIDE
    } else {
        current & !ABS_AUTOHIDE
    };

    data.l_param = target as isize;
    unsafe {
        SHAppBarMessage(ABM_SETSTATE, &mut data);
    }

    Ok((current, target))
}

fn handle_command(raw: &str) -> bool {
    let command = raw.trim().to_ascii_lowercase();
    if command.is_empty() {
        return true;
    }
    if command == "quit" || command == "exit" {
        return false;
    }

    let hidden = match command.as_str() {
        "hide" | "hidden" | "true" | "1" => true,
        "show" | "visible" | "false" | "0" => false,
        _ => {
            emit_json(false, &command, false, 0, 0, 0, "unknown command");
            return true;
        }
    };

    let started = Instant::now();
    match set_taskbar_hidden(hidden) {
        Ok((current, target)) => emit_json(true, &command, hidden, current, target, started.elapsed().as_millis(), ""),
        Err(error) => emit_json(false, &command, hidden, 0, 0, started.elapsed().as_millis(), &error),
    }
    true
}

fn main() {
    let stdin = io::stdin();
    println!("{{\"ok\":true,\"ready\":true,\"service\":\"ingen_windows_taskbar_helper\"}}");
    let _ = io::stdout().flush();
    for line in stdin.lock().lines() {
        match line {
            Ok(value) => {
                if !handle_command(&value) {
                    break;
                }
            }
            Err(error) => {
                emit_json(false, "read", false, 0, 0, 0, &error.to_string());
                break;
            }
        }
    }
}
