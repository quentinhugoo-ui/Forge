use std::ffi::{c_void, OsStr};
use std::io::{self, BufRead, Write};
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null;
use std::time::Instant;

const ABM_GETSTATE: u32 = 0x0000_0004;
const ABM_SETSTATE: u32 = 0x0000_000A;
const ABS_AUTOHIDE: usize = 0x0000_0001;
const ERROR_MORE_DATA: i32 = 234;
const ERROR_SUCCESS: i32 = 0;
const HKEY_CURRENT_USER: *mut c_void = 0x8000_0001usize as *mut c_void;
const KEY_QUERY_VALUE: u32 = 0x0001;
const KEY_SET_VALUE: u32 = 0x0002;
const REG_BINARY: u32 = 3;
const SMTO_ABORTIFHUNG: u32 = 0x0002;
const WM_SETTINGCHANGE: u32 = 0x001A;
const HWND_BROADCAST: *mut c_void = 0xffffusize as *mut c_void;
const STUCK_RECTS_SETTINGS_INDEX: usize = 8;

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
    fn SendMessageTimeoutW(
        hwnd: *mut c_void,
        msg: u32,
        w_param: usize,
        l_param: *const u16,
        flags: u32,
        timeout: u32,
        result: *mut usize,
    ) -> usize;
}

#[link(name = "advapi32")]
extern "system" {
    fn RegOpenKeyExW(
        hkey: *mut c_void,
        sub_key: *const u16,
        options: u32,
        sam_desired: u32,
        result: *mut *mut c_void,
    ) -> i32;
    fn RegQueryValueExW(
        hkey: *mut c_void,
        value_name: *const u16,
        reserved: *mut u32,
        value_type: *mut u32,
        data: *mut u8,
        data_len: *mut u32,
    ) -> i32;
    fn RegSetValueExW(
        hkey: *mut c_void,
        value_name: *const u16,
        reserved: u32,
        value_type: u32,
        data: *const u8,
        data_len: u32,
    ) -> i32;
    fn RegCloseKey(hkey: *mut c_void) -> i32;
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(std::iter::once(0)).collect()
}

fn emit_json(
    ok: bool,
    command: &str,
    hidden: bool,
    current: usize,
    target: usize,
    elapsed_ms: u128,
    registry_path_found: bool,
    registry_byte: i32,
    error: &str,
) {
    let escaped_error = error.replace('\\', "\\\\").replace('"', "\\\"");
    println!(
        "{{\"ok\":{},\"command\":\"{}\",\"hidden\":{},\"current\":{},\"target\":{},\"elapsedMs\":{},\"registryPathFound\":{},\"registryByte\":{},\"error\":\"{}\"}}",
        ok, command, hidden, current, target, elapsed_ms, registry_path_found, registry_byte, escaped_error
    );
    let _ = io::stdout().flush();
}

fn broadcast_tray_settings_changed() {
    let message = wide("TraySettings");
    let mut result = 0usize;
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            message.as_ptr(),
            SMTO_ABORTIFHUNG,
            120,
            &mut result,
        );
    }
}

fn update_stuck_rects_auto_hide(hidden: bool) -> Result<(bool, i32), String> {
    let sub_key = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StuckRects3");
    let value_name = wide("Settings");
    let mut key: *mut c_void = std::ptr::null_mut();
    let open_status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            sub_key.as_ptr(),
            0,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            &mut key,
        )
    };
    if open_status != ERROR_SUCCESS {
        return Ok((false, -1));
    }

    let mut close_key = true;
    let result = (|| {
        let mut value_type = 0u32;
        let mut data_len = 0u32;
        let query_status = unsafe {
            RegQueryValueExW(
                key,
                value_name.as_ptr(),
                std::ptr::null_mut(),
                &mut value_type,
                std::ptr::null_mut(),
                &mut data_len,
            )
        };
        if query_status != ERROR_SUCCESS && query_status != ERROR_MORE_DATA {
            return Err(format!("RegQueryValueExW size failed: {}", query_status));
        }
        if value_type != REG_BINARY || data_len as usize <= STUCK_RECTS_SETTINGS_INDEX {
            return Ok((true, -1));
        }

        let mut settings = vec![0u8; data_len as usize];
        let read_status = unsafe {
            RegQueryValueExW(
                key,
                value_name.as_ptr(),
                std::ptr::null_mut(),
                &mut value_type,
                settings.as_mut_ptr(),
                &mut data_len,
            )
        };
        if read_status != ERROR_SUCCESS {
            return Err(format!("RegQueryValueExW data failed: {}", read_status));
        }

        let previous = settings[STUCK_RECTS_SETTINGS_INDEX];
        let next = if hidden {
            previous | ABS_AUTOHIDE as u8
        } else {
            previous & !(ABS_AUTOHIDE as u8)
        };
        settings[STUCK_RECTS_SETTINGS_INDEX] = next;
        let write_status = unsafe {
            RegSetValueExW(
                key,
                value_name.as_ptr(),
                0,
                REG_BINARY,
                settings.as_ptr(),
                data_len,
            )
        };
        if write_status != ERROR_SUCCESS {
            return Err(format!("RegSetValueExW failed: {}", write_status));
        }
        broadcast_tray_settings_changed();
        Ok((true, next as i32))
    })();

    if close_key {
        unsafe {
            RegCloseKey(key);
        }
        close_key = false;
    }
    let _ = close_key;
    result
}

fn set_taskbar_hidden(hidden: bool) -> Result<(usize, usize, bool, i32), String> {
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

    let (registry_path_found, registry_byte) = update_stuck_rects_auto_hide(hidden)?;
    Ok((current, target, registry_path_found, registry_byte))
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
            emit_json(false, &command, false, 0, 0, 0, false, -1, "unknown command");
            return true;
        }
    };

    let started = Instant::now();
    match set_taskbar_hidden(hidden) {
        Ok((current, target, registry_path_found, registry_byte)) => emit_json(
            true,
            &command,
            hidden,
            current,
            target,
            started.elapsed().as_millis(),
            registry_path_found,
            registry_byte,
            "",
        ),
        Err(error) => emit_json(
            false,
            &command,
            hidden,
            0,
            0,
            started.elapsed().as_millis(),
            false,
            -1,
            &error,
        ),
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
                emit_json(false, "read", false, 0, 0, 0, false, -1, &error.to_string());
                break;
            }
        }
    }
}
