//! The optional tray icon, the one part of backreel that stays resident.
//!
//! It only holds the icon. Every action starts the same exe as a new process,
//! so the graphics and network code never load here and memory stays small.

use crate::Result;
use crate::schedule;
use crate::store::{self, State};
use std::cell::Cell;
use std::mem::transmute;
use std::process::{Child, Command};
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::{
    GetModuleHandleW, GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, PCSTR, PCWSTR, w};

const CLASS_NAME: PCWSTR = w!("backreel.tray");
const ICON_ID: u32 = 1;
/// The shell reports clicks on the icon with this message.
const WM_ICON: u32 = WM_APP + 1;
/// Posted by the worker thread when a change finishes; wparam is 1 on success.
const WM_NEXT_DONE: u32 = WM_APP + 2;
/// Clicks on a version 4 icon, which `windows` doesn't define.
const NIN_SELECT: u32 = WM_USER;
const NIN_KEYSELECT: u32 = WM_USER + 1;
const MENU_NEXT: u32 = 1;
const MENU_SETTINGS: u32 = 2;
const MENU_REMOVE: u32 = 3;
/// How long to wait for a running tray to exit before giving up on it.
const STOP_TIMEOUT_MS: u32 = 3000;

/// Whether the icon is up right now. This, not the task, is what the user sees.
pub fn is_running() -> bool {
    // SAFETY: the class name is a static string.
    unsafe { FindWindowW(CLASS_NAME, None) }.is_ok()
}

/// Closes a running tray and waits for it to exit, which frees its exe for
/// replacement. Returns whether one was running.
pub fn stop() -> bool {
    // SAFETY: the window handle is only used for calls that fail harmlessly if
    // it has gone, and the process handle is closed before returning.
    unsafe {
        let Ok(window) = FindWindowW(CLASS_NAME, None) else { return false };
        let mut process_id = 0;
        GetWindowThreadProcessId(window, Some(&mut process_id));
        let process = OpenProcess(PROCESS_SYNCHRONIZE, false, process_id);
        let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
        if let Ok(process) = process {
            WaitForSingleObject(process, STOP_TIMEOUT_MS);
            let _ = CloseHandle(process);
        }
    }
    true
}

struct Tray {
    hwnd: HWND,
    /// Sent by Explorer whenever the taskbar is recreated, which drops every icon.
    taskbar_created: u32,
    /// `Cell` because the menu's modal loop re-enters `window_proc` mid-call.
    busy: Cell<bool>,
}

/// The `--tray` entry point: runs until the icon is removed.
pub fn run() -> Result<()> {
    // SAFETY: plain Win32 window creation; every pointer passed outlives its call.
    unsafe {
        if FindWindowW(CLASS_NAME, None).is_ok() {
            return Ok(());
        }
        let instance: HINSTANCE = GetModuleHandleW(None)?.into();
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 {
            return Err(windows::core::Error::from_thread().into());
        }
        // A real top-level window that is never shown: message-only windows
        // don't receive the broadcast that says the taskbar was recreated.
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            CLASS_NAME,
            w!("backreel"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            None,
        )?;

        let tray = Tray { hwnd, taskbar_created: RegisterWindowMessageW(w!("TaskbarCreated")), busy: Cell::new(false) };
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, &raw const tray as isize);
        follow_system_menu_theme(hwnd);
        // This fails at logon if Explorer isn't up yet; TaskbarCreated retries it.
        tray.show_icon();

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: GWLP_USERDATA is either 0 or the `Tray` on `run`'s stack, which
    // outlives the window. Only shared references to it are ever made.
    let tray = unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Tray).as_ref() };
    if let Some(tray) = tray {
        if message == tray.taskbar_created || message == WM_DPICHANGED {
            tray.show_icon();
            return LRESULT(0);
        }
        match message {
            WM_ICON => {
                tray.icon_event(wparam, lparam);
                return LRESULT(0);
            }
            WM_NEXT_DONE => {
                tray.next_done(wparam.0 == 1);
                return LRESULT(0);
            }
            WM_DESTROY => {
                tray.hide_icon();
                // SAFETY: called on the thread that owns the message loop.
                unsafe { PostQuitMessage(0) };
                return LRESULT(0);
            }
            _ => {}
        }
    }
    // SAFETY: forwards the unmodified message.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

impl Tray {
    fn icon_data(&self) -> NOTIFYICONDATAW {
        NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: ICON_ID,
            ..Default::default()
        }
    }

    /// Adds the icon, or refreshes it if it's already there.
    fn show_icon(&self) {
        let mut data = self.icon_data();
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
        data.uCallbackMessage = WM_ICON;
        data.hIcon = self.load_icon().unwrap_or_default();
        fill(&mut data.szTip, "backreel");
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        // SAFETY: `data` is fully initialised and outlives each call.
        unsafe {
            if !Shell_NotifyIconW(NIM_MODIFY, &data).as_bool() && Shell_NotifyIconW(NIM_ADD, &data).as_bool() {
                let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
            }
            if !data.hIcon.is_invalid() {
                let _ = DestroyIcon(data.hIcon);
            }
        }
    }

    fn hide_icon(&self) {
        // SAFETY: `data` identifies this window's icon and outlives the call.
        let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &self.icon_data()) };
    }

    /// Loads the icon frame drawn for the tray's size at this monitor's scale.
    fn load_icon(&self) -> Result<HICON> {
        // SAFETY: resource 1 is the app icon, embedded by build.rs.
        unsafe {
            let dpi = GetDpiForWindow(self.hwnd);
            let size = GetSystemMetricsForDpi(SM_CXSMICON, dpi);
            let instance: HINSTANCE = GetModuleHandleW(None)?.into();
            let icon = LoadImageW(Some(instance), PCWSTR(1 as _), IMAGE_ICON, size, size, LR_DEFAULTCOLOR)?;
            Ok(HICON(icon.0))
        }
    }

    fn icon_event(&self, wparam: WPARAM, lparam: LPARAM) {
        match lparam.0 as u32 & 0xFFFF {
            NIN_SELECT | NIN_KEYSELECT => self.open_settings(),
            WM_CONTEXTMENU => {
                // Version 4 icons pack the anchor point into wparam.
                let x = i32::from((wparam.0 & 0xFFFF) as i16);
                let y = i32::from(((wparam.0 >> 16) & 0xFFFF) as i16);
                if let Err(error) = self.show_menu(x, y) {
                    store::log(&format!("tray: {error}"));
                }
            }
            _ => {}
        }
    }

    fn show_menu(&self, x: i32, y: i32) -> Result<()> {
        let next_state = if self.busy.get() { MF_GRAYED } else { MF_ENABLED };
        // SAFETY: the menu is destroyed before returning, and `hwnd` is this window.
        let choice = unsafe {
            let menu = CreatePopupMenu()?;
            AppendMenuW(menu, MF_STRING | next_state, MENU_NEXT as usize, w!("Next wallpaper"))?;
            AppendMenuW(menu, MF_STRING, MENU_SETTINGS as usize, w!("Settings"))?;
            AppendMenuW(menu, MF_SEPARATOR, 0, None)?;
            AppendMenuW(menu, MF_STRING, MENU_REMOVE as usize, w!("Remove tray icon"))?;
            SetMenuDefaultItem(menu, MENU_SETTINGS, 0)?;

            // Without the foreground, the menu won't close on a click elsewhere.
            let _ = SetForegroundWindow(self.hwnd);
            let flags = TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_RIGHTALIGN | TPM_BOTTOMALIGN;
            let choice = TrackPopupMenu(menu, flags, x, y, None, self.hwnd, None);
            let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            let _ = DestroyMenu(menu);
            choice.0 as u32
        };
        match choice {
            MENU_NEXT => self.next(),
            MENU_SETTINGS => self.open_settings(),
            MENU_REMOVE => {
                schedule::remove_tray_task()?;
                // SAFETY: `hwnd` is this window, on its own thread.
                unsafe { DestroyWindow(self.hwnd)? };
            }
            _ => {}
        }
        Ok(())
    }

    fn open_settings(&self) {
        // SAFETY: no preconditions. Lets the new process take the foreground.
        let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
        if let Err(error) = launch(None) {
            store::log(&format!("tray: {error}"));
        }
    }

    fn next(&self) {
        self.busy.set(true);
        let hwnd = self.hwnd.0 as isize;
        std::thread::spawn(move || {
            let succeeded = launch(Some("--next")).and_then(|mut child| Ok(child.wait()?)).is_ok_and(|s| s.success());
            // SAFETY: posting to a window that has since closed fails harmlessly.
            let _ = unsafe { PostMessageW(Some(HWND(hwnd as _)), WM_NEXT_DONE, WPARAM(succeeded.into()), LPARAM(0)) };
        });
    }

    /// With no window open, a failure would otherwise pass in silence.
    fn next_done(&self, succeeded: bool) {
        self.busy.set(false);
        if succeeded {
            return;
        }
        let mut data = self.icon_data();
        data.uFlags = NIF_INFO;
        data.dwInfoFlags = NIIF_WARNING;
        fill(&mut data.szInfoTitle, "Couldn't change the wallpaper");
        fill(&mut data.szInfo, &State::load().last_error.unwrap_or_default());
        // SAFETY: `data` is fully initialised and outlives the call.
        let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
    }
}

fn launch(argument: Option<&str>) -> Result<Child> {
    let mut command = Command::new(std::env::current_exe()?);
    command.args(argument);
    Ok(command.spawn()?)
}

/// Copies `text` into a fixed UTF-16 field, truncating to keep the terminator.
fn fill(field: &mut [u16], text: &str) {
    let capacity = field.len() - 1;
    for (slot, unit) in field[..capacity].iter_mut().zip(text.encode_utf16().chain(std::iter::repeat(0))) {
        *slot = unit;
    }
    field[capacity] = 0;
}

/// Popup menus are always light unless told otherwise, and the only way to tell
/// them is through these unnamed uxtheme exports. Every browser and many apps
/// rely on them; if they ever vanish, the menu just stays light.
fn follow_system_menu_theme(hwnd: HWND) {
    const ALLOW_DARK_FOR_WINDOW: usize = 133;
    const SET_PREFERRED_APP_MODE: usize = 135;
    const FLUSH_MENU_THEMES: usize = 136;
    const APP_MODE_ALLOW_DARK: i32 = 1;
    type Export = unsafe extern "system" fn() -> isize;

    // SAFETY: each ordinal has had this signature since Windows 10 1903, and
    // the library stays loaded because the theme it sets lives inside it.
    unsafe {
        let Ok(uxtheme) = LoadLibraryExW(w!("uxtheme.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) else { return };
        let export = |ordinal: usize| GetProcAddress(uxtheme, PCSTR(ordinal as _));
        let (Some(set_mode), Some(allow), Some(flush)) =
            (export(SET_PREFERRED_APP_MODE), export(ALLOW_DARK_FOR_WINDOW), export(FLUSH_MENU_THEMES))
        else {
            return;
        };
        let _ = transmute::<Export, unsafe extern "system" fn(i32) -> i32>(set_mode)(APP_MODE_ALLOW_DARK);
        let _ = transmute::<Export, unsafe extern "system" fn(HWND, BOOL) -> BOOL>(allow)(hwnd, true.into());
        transmute::<Export, unsafe extern "system" fn()>(flush)();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_clears_the_rest_of_the_field() {
        let mut field = [7u16; 4];
        fill(&mut field, "a");
        assert_eq!(field, [u16::from(b'a'), 0, 0, 0]);
    }

    #[test]
    fn fill_truncates_and_keeps_the_terminator() {
        let mut field = [7u16; 3];
        fill(&mut field, "abcdef");
        assert_eq!(field, [u16::from(b'a'), u16::from(b'b'), 0]);
    }
}
