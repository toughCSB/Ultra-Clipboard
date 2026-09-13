//! Windows-only `Win+V` interception. A low-level keyboard hook consumes the
//! reserved hotkey so the system clipboard history panel does not open.

use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use tauri::AppHandle;
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::um::processthreadsapi::GetCurrentThreadId;
use winapi::um::winuser::{
    CallNextHookEx, GetAsyncKeyState, GetMessageW, PostThreadMessageW, SendInput,
    SetWindowsHookExW, UnhookWindowsHookEx, INPUT, INPUT_KEYBOARD, KBDLLHOOKSTRUCT, KEYBDINPUT,
    KEYEVENTF_KEYUP, LLKHF_INJECTED, MSG, VK_LWIN, VK_RWIN, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
    WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::window::{self, CLIPBOARD_WINDOW_LABEL};

const VK_V: u32 = 0x56;

const VK_DUMMY: u16 = 0xE8;

static ENABLED: AtomicBool = AtomicBool::new(false);
static V_CONSUMED: AtomicBool = AtomicBool::new(false);
static HOOK_THREAD_ID: Mutex<Option<u32>> = Mutex::new(None);
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

pub fn set_enabled(app: &AppHandle, enabled: bool) {
    if enabled {
        enable(app);
    } else {
        disable();
    }
}

fn enable(app: &AppHandle) {
    let _ = APP_HANDLE.set(app.clone());
    ENABLED.store(true, Ordering::Relaxed);

    if HOOK_THREAD_ID
        .lock()
        .expect("win_v hook thread id poisoned")
        .is_some()
    {
        return;
    }

    std::thread::spawn(|| unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), null_mut(), 0);
        if hook.is_null() {
            log::error!("SetWindowsHookExW(WH_KEYBOARD_LL) for win+v failed");
            return;
        }

        *HOOK_THREAD_ID
            .lock()
            .expect("win_v hook thread id poisoned") = Some(GetCurrentThreadId());

        let mut msg: MSG = std::mem::zeroed();

        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {}

        UnhookWindowsHookEx(hook);
        *HOOK_THREAD_ID
            .lock()
            .expect("win_v hook thread id poisoned") = None;
        V_CONSUMED.store(false, Ordering::Relaxed);
    });
}

fn disable() {
    ENABLED.store(false, Ordering::Relaxed);

    let tid = HOOK_THREAD_ID
        .lock()
        .expect("win_v hook thread id poisoned")
        .take();
    if let Some(tid) = tid {
        unsafe {
            PostThreadMessageW(tid, WM_QUIT, 0, 0);
        }
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 || !ENABLED.load(Ordering::Relaxed) {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    let kbd = &*(lparam as *const KBDLLHOOKSTRUCT);

    if kbd.flags & LLKHF_INJECTED != 0 {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    let vk = kbd.vkCode;
    if vk != VK_V {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    let msg = wparam as UINT;

    if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
        let win_down = (GetAsyncKeyState(VK_LWIN) as u16) & 0x8000 != 0
            || (GetAsyncKeyState(VK_RWIN) as u16) & 0x8000 != 0;
        if !win_down {
            return CallNextHookEx(null_mut(), code, wparam, lparam);
        }

        if V_CONSUMED.swap(true, Ordering::Relaxed) {
            return 1;
        }

        suppress_start_menu();
        schedule_toggle();

        return 1;
    }

    if (msg == WM_KEYUP || msg == WM_SYSKEYUP) && V_CONSUMED.swap(false, Ordering::Relaxed) {
        return 1;
    }

    CallNextHookEx(null_mut(), code, wparam, lparam)
}

fn suppress_start_menu() {
    let mut inputs: [INPUT; 2] = unsafe { std::mem::zeroed() };

    inputs[0].type_ = INPUT_KEYBOARD;
    unsafe {
        *inputs[0].u.ki_mut() = KEYBDINPUT {
            wVk: VK_DUMMY,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: 0,
        };
    }
    inputs[1].type_ = INPUT_KEYBOARD;
    unsafe {
        *inputs[1].u.ki_mut() = KEYBDINPUT {
            wVk: VK_DUMMY,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: 0,
        };
    }

    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_mut_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    if sent as usize != inputs.len() {
        log::warn!(
            "inject start-menu suppress key sent {sent}/{}",
            inputs.len()
        );
    }
}

fn schedule_toggle() {
    let Some(app) = APP_HANDLE.get() else {
        return;
    };

    let handle = app.clone();
    if let Err(err) = app.run_on_main_thread(move || {
        if let Err(err) = window::toggle_window(&handle, CLIPBOARD_WINDOW_LABEL) {
            log::warn!("toggle clipboard window via win+v failed: {err}");
        }
    }) {
        log::warn!("schedule win+v toggle failed: {err}");
    }
}
