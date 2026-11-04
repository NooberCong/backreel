//! The settings window. Custom-drawn, because Win32 common controls have no
//! supported dark mode. It only exists while open, so it costs nothing at idle.

mod canvas;
mod layout;

use crate::desktop::Desktop;
use crate::imaging::Imaging;
use crate::run::{self, Trigger};
use crate::sources::Category;
use crate::store::{Applied, Config, State};
use crate::{Com, Result, schedule, store, tray};
use canvas::{Align, CORNER, Canvas, Color, Font};
use layout::{Control, Rect};
use std::path::Path;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, HSTRING, PCWSTR, w};

const CLASS_NAME: PCWSTR = w!("backreel.settings");
/// Posted by the worker thread when a manual change finishes.
const WM_CHANGE_DONE: u32 = WM_APP + 1;
const STYLE: WINDOW_STYLE = WINDOW_STYLE(WS_OVERLAPPED.0 | WS_CAPTION.0 | WS_SYSMENU.0);
const WM_MOUSELEAVE: u32 = 0x02A3;

struct Palette {
    background: Color,
    surface: Color,
    surface_hover: Color,
    border: Color,
    text: Color,
    muted: Color,
    accent: Color,
    accent_hover: Color,
    on_accent: Color,
    accent_text: Color,
    warning: Color,
}

const DARK: Palette = Palette {
    background: Color::rgb(0x202020),
    surface: Color::rgb(0x2D2D2D),
    surface_hover: Color::rgb(0x383838),
    border: Color::rgb(0x404040),
    text: Color::rgb(0xFFFFFF),
    muted: Color::rgb(0xA8A8A8),
    accent: Color::rgb(0x60CDFF),
    accent_hover: Color::rgb(0x7AD6FF),
    on_accent: Color::rgb(0x00263A),
    accent_text: Color::rgb(0x9BE0FF),
    warning: Color::rgb(0xFFB454),
};

const LIGHT: Palette = Palette {
    background: Color::rgb(0xF3F3F3),
    surface: Color::rgb(0xFFFFFF),
    surface_hover: Color::rgb(0xF9F9F9),
    border: Color::rgb(0xD9D9D9),
    text: Color::rgb(0x1B1B1B),
    muted: Color::rgb(0x5F5F5F),
    accent: Color::rgb(0x005FB8),
    accent_hover: Color::rgb(0x1A73C7),
    on_accent: Color::rgb(0xFFFFFF),
    accent_text: Color::rgb(0x004A91),
    warning: Color::rgb(0xB35900),
};

struct App {
    hwnd: HWND,
    canvas: Canvas,
    imaging: Imaging,
    config: Config,
    state: State,
    daily: bool,
    tray: bool,
    busy: bool,
    dark: bool,
    hover: Option<Control>,
    /// Set only by keyboard use, so mouse users never see a focus ring.
    focus: Option<Control>,
    /// The file being shown and its uploaded bitmap.
    preview_path: Option<std::path::PathBuf>,
    preview: Option<ID2D1Bitmap>,
    /// A failed action that isn't recorded in `State`.
    notice: Option<String>,
}

pub fn run() -> Result<()> {
    // SAFETY: plain Win32 window creation; every pointer passed outlives its call.
    unsafe {
        if let Ok(existing) = FindWindowW(CLASS_NAME, None) {
            let _ = ShowWindowAsync(existing, SW_RESTORE);
            let _ = SetForegroundWindow(existing);
            return Ok(());
        }

        let instance: HINSTANCE = GetModuleHandleW(None)?.into();
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hIcon: LoadIconW(Some(instance), PCWSTR(1 as _))?,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 {
            return Err(windows::core::Error::from_thread().into());
        }

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            CLASS_NAME,
            w!("backreel"),
            STYLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            Some(instance),
            None,
        )?;

        let dark = uses_dark_theme();
        let mut app = Box::new(App {
            hwnd,
            canvas: Canvas::new(hwnd)?,
            imaging: Imaging::new()?,
            config: Config::load(),
            state: State::load(),
            daily: schedule::is_enabled(),
            tray: tray::is_running(),
            busy: false,
            dark,
            hover: None,
            focus: None,
            preview_path: None,
            preview: None,
            notice: None,
        });
        app.apply_theme();
        app.fit_window(GetDpiForWindow(hwnd))?;
        center_on_screen(hwnd);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, &raw mut *app as isize);
        let _ = ShowWindow(hwnd, SW_SHOW);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        // The window is gone, so `app` is no longer reachable from `window_proc`.
        drop(app);
    }
    Ok(())
}

extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: GWLP_USERDATA is either 0 or the `App` owned by `run`, which
    // outlives the window. Messages arrive on this one thread and do not nest
    // here except through `DefWindowProcW`, after this borrow ends.
    let app = unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App).as_mut() };
    let handled = app.and_then(|app| match app.handle(message, wparam, lparam) {
        Ok(result) => result,
        Err(error) => {
            store::log(&format!("ui: {error}"));
            None
        }
    });
    if let Some(result) = handled {
        return result;
    }
    if message == WM_DESTROY {
        // SAFETY: called on the thread that owns the message loop.
        unsafe { PostQuitMessage(0) };
        return LRESULT(0);
    }
    // SAFETY: forwards the unmodified message.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

impl App {
    /// Returns `Some` when the message is fully handled.
    fn handle(&mut self, message: u32, wparam: WPARAM, lparam: LPARAM) -> Result<Option<LRESULT>> {
        match message {
            WM_PAINT => self.paint()?,
            WM_ERASEBKGND => return Ok(Some(LRESULT(1))),
            WM_MOUSEMOVE => self.mouse_move(lparam),
            WM_MOUSELEAVE => self.set_hover(None),
            WM_LBUTTONUP => {
                self.focus = None;
                let (x, y) = self.to_dips(lparam);
                if let Some(control) = hit_test(x, y) {
                    self.activate(control);
                }
                self.invalidate();
            }
            WM_SETCURSOR if self.hover.is_some_and(|control| self.is_active(control)) => {
                // SAFETY: IDC_HAND is a system cursor.
                unsafe { SetCursor(LoadCursorW(None, IDC_HAND).ok()) };
                return Ok(Some(LRESULT(1)));
            }
            WM_KEYDOWN => self.key_down(wparam),
            WM_CHANGE_DONE => {
                self.busy = false;
                self.reload_state();
            }
            // The tray or a scheduled run may have acted while this window was open.
            WM_ACTIVATE if wparam.0 & 0xFFFF != WA_INACTIVE as usize && !self.busy => {
                self.tray = tray::is_running();
                if State::load() != self.state {
                    self.reload_state();
                }
                self.invalidate();
                return Ok(None);
            }
            WM_SETTINGCHANGE => {
                self.dark = uses_dark_theme();
                self.apply_theme();
                self.invalidate();
            }
            WM_DPICHANGED => {
                // SAFETY: Windows passes a valid suggested rectangle in lparam.
                let suggested = unsafe { &*(lparam.0 as *const RECT) };
                // SAFETY: `hwnd` is this window.
                unsafe {
                    SetWindowPos(
                        self.hwnd,
                        None,
                        suggested.left,
                        suggested.top,
                        suggested.right - suggested.left,
                        suggested.bottom - suggested.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )?;
                }
                self.preview = None;
                self.preview_path = None;
                self.invalidate();
            }
            WM_SIZE => {
                let (width, height) = (lparam.0 as u32 & 0xFFFF, (lparam.0 as u32 >> 16) & 0xFFFF);
                self.canvas.resize(width, height)?;
            }
            _ => return Ok(None),
        }
        Ok(Some(LRESULT(0)))
    }

    fn reload_state(&mut self) {
        self.state = State::load();
        self.preview = None;
        self.preview_path = None;
        self.invalidate();
    }

    fn dpi(&self) -> f32 {
        // SAFETY: `hwnd` is this window.
        unsafe { GetDpiForWindow(self.hwnd) as f32 }
    }

    fn to_dips(&self, lparam: LPARAM) -> (f32, f32) {
        let scale = 96.0 / self.dpi();
        let x = (lparam.0 & 0xFFFF) as i16;
        let y = ((lparam.0 >> 16) & 0xFFFF) as i16;
        (f32::from(x) * scale, f32::from(y) * scale)
    }

    fn invalidate(&self) {
        // SAFETY: `hwnd` is this window.
        let _ = unsafe { InvalidateRect(Some(self.hwnd), None, false) };
    }

    fn palette(&self) -> &'static Palette {
        if self.dark { &DARK } else { &LIGHT }
    }

    /// Matches the title bar to the window, so the two read as one surface.
    fn apply_theme(&self) {
        let dark = BOOL::from(self.dark);
        let caption = colorref(self.palette().background);
        // SAFETY: each pointer and size describes the local value it names.
        unsafe {
            let _ = DwmSetWindowAttribute(
                self.hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                (&raw const dark).cast(),
                size_of::<BOOL>() as u32,
            );
            let _ = DwmSetWindowAttribute(
                self.hwnd,
                DWMWA_CAPTION_COLOR,
                (&raw const caption).cast(),
                size_of::<COLORREF>() as u32,
            );
        }
    }

    fn fit_window(&self, dpi: u32) -> Result<()> {
        let scale = dpi as f32 / 96.0;
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: (layout::WIDTH * scale).round() as i32,
            bottom: (layout::HEIGHT * scale).round() as i32,
        };
        // SAFETY: `rect` is valid for read and write; `hwnd` is this window.
        unsafe {
            AdjustWindowRectExForDpi(&mut rect, STYLE, false, WINDOW_EX_STYLE::default(), dpi)?;
            SetWindowPos(
                self.hwnd,
                None,
                0,
                0,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )?;
        }
        Ok(())
    }

    fn mouse_move(&mut self, lparam: LPARAM) {
        let (x, y) = self.to_dips(lparam);
        self.set_hover(hit_test(x, y));
        let mut track = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: self.hwnd,
            dwHoverTime: 0,
        };
        // SAFETY: `track` is fully initialised.
        let _ = unsafe { TrackMouseEvent(&mut track) };
    }

    fn set_hover(&mut self, hover: Option<Control>) {
        if self.hover != hover {
            self.hover = hover;
            self.invalidate();
        }
    }

    fn key_down(&mut self, key: WPARAM) {
        let key = VIRTUAL_KEY(key.0 as u16);
        match key {
            VK_ESCAPE => {
                // SAFETY: `hwnd` is this window.
                let _ = unsafe { PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
            }
            VK_TAB => {
                // SAFETY: GetKeyState has no preconditions.
                let backwards = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                self.move_focus(backwards);
            }
            VK_SPACE | VK_RETURN => {
                if let Some(control) = self.focus {
                    self.activate(control);
                }
            }
            _ => return,
        }
        self.invalidate();
    }

    fn move_focus(&mut self, backwards: bool) {
        let order: Vec<Control> =
            layout::controls().into_iter().map(|(control, _)| control).filter(|c| self.is_active(*c)).collect();
        let position = self.focus.and_then(|focus| order.iter().position(|control| *control == focus));
        self.focus = match (position, backwards) {
            (None, false) => order.first().copied(),
            (None, true) => order.last().copied(),
            (Some(index), false) => order.get((index + 1) % order.len()).copied(),
            (Some(index), true) => order.get((index + order.len() - 1) % order.len()).copied(),
        };
    }

    /// Whether `control` currently does anything.
    fn is_active(&self, control: Control) -> bool {
        match control {
            Control::OpenPage => self.applied().is_some(),
            Control::Next => !self.busy,
            Control::Category(category) => self.category_available(category),
            Control::Source(_) | Control::Daily | Control::Tray => true,
        }
    }

    fn category_available(&self, category: Category) -> bool {
        self.config.sources.iter().any(|source| source.serves(category))
    }

    fn activate(&mut self, control: Control) {
        if !self.is_active(control) {
            return;
        }
        self.notice = None;
        let result = match control {
            Control::OpenPage => {
                self.open_page();
                Ok(())
            }
            Control::Next => {
                self.start_change();
                Ok(())
            }
            Control::Category(category) => {
                self.config.toggle_category(category);
                self.config.save()
            }
            Control::Source(source) => {
                self.config.toggle_source(source);
                self.config.save()
            }
            Control::Daily => self.toggle_daily(),
            Control::Tray => self.toggle_tray(),
        };
        if let Err(error) = result {
            store::log(&format!("ui: {error}"));
            self.notice = Some(error.to_string());
        }
    }

    fn open_page(&self) {
        if let Some(applied) = self.applied() {
            let url = HSTRING::from(applied.page_url.as_str());
            // SAFETY: the URL string outlives the call.
            unsafe { ShellExecuteW(Some(self.hwnd), w!("open"), &url, None, None, SW_SHOWNORMAL) };
        }
    }

    fn start_change(&mut self) {
        self.busy = true;
        let hwnd = self.hwnd.0 as isize;
        std::thread::spawn(move || {
            let result = Com::init().and_then(|_com| run::run(Trigger::Manual));
            if let Err(error) = result {
                store::log(&format!("next: {error}"));
            }
            // SAFETY: posting to a window that has since closed fails harmlessly.
            let _ = unsafe { PostMessageW(Some(HWND(hwnd as _)), WM_CHANGE_DONE, WPARAM(0), LPARAM(0)) };
        });
    }

    fn toggle_daily(&mut self) -> Result<()> {
        if self.daily {
            schedule::disable()?
        } else {
            schedule::enable()?
        }
        self.daily = schedule::is_enabled();
        Ok(())
    }

    fn toggle_tray(&mut self) -> Result<()> {
        if self.tray {
            schedule::disable_tray()?
        } else {
            schedule::enable_tray()?
        }
        self.tray = !self.tray;
        Ok(())
    }

    /// What backreel knows about the wallpaper on screen, if it set it.
    fn applied(&self) -> Option<&Applied> {
        let shown = self.preview_path.as_deref()?;
        self.state.current.iter().find(|applied| applied.path == shown)
    }

    /// Finds the file on the primary monitor and uploads it for display.
    fn load_preview(&mut self) -> Result<()> {
        let desktop = Desktop::new()?;
        let monitors = desktop.monitors()?;
        let Some(path) = monitors.first().and_then(|monitor| desktop.current(monitor)) else {
            return Ok(());
        };
        let bitmap = self.decode_preview(&path);
        // A wallpaper file can vanish while Windows still shows its cached copy.
        self.preview = bitmap.ok();
        self.preview_path = Some(path);
        Ok(())
    }

    fn decode_preview(&self, path: &Path) -> Result<ID2D1Bitmap> {
        let dpi = self.dpi();
        let scale = dpi / 96.0;
        let size = ((layout::PREVIEW.w * scale).round() as u32, (layout::PREVIEW.h * scale).round() as u32);
        let source = self.imaging.load_cover(path, size)?;
        self.canvas.create_bitmap(&self.imaging.to_pbgra(&source)?, dpi)
    }

    fn paint(&mut self) -> Result<()> {
        let mut paint = PAINTSTRUCT::default();
        // SAFETY: paired with `EndPaint` below.
        unsafe { BeginPaint(self.hwnd, &mut paint) };
        let result = self.draw();
        // SAFETY: pairs with `BeginPaint`.
        let _ = unsafe { EndPaint(self.hwnd, &paint) };
        result
    }

    fn draw(&mut self) -> Result<()> {
        let palette = self.palette();
        if self.canvas.begin(self.dpi(), palette.background)? {
            self.preview = None;
            self.preview_path = None;
        }
        if self.preview_path.is_none()
            && let Err(error) = self.load_preview()
        {
            store::log(&format!("preview: {error}"));
        }

        self.draw_now_showing(palette)?;
        self.draw_section("Categories", layout::CATEGORIES_LABEL, palette)?;
        self.draw_section("Sources", layout::SOURCES_LABEL, palette)?;
        for (control, rect) in layout::controls() {
            match control {
                Control::OpenPage => {}
                Control::Next => self.draw_next(rect, palette)?,
                Control::Category(category) => {
                    let selected = self.config.categories.contains(&category);
                    self.draw_chip(control, category.label(), selected, rect, palette)?;
                }
                Control::Source(source) => {
                    let selected = self.config.sources.contains(&source);
                    self.draw_chip(control, source.label(), selected, rect, palette)?;
                }
                Control::Daily => self.draw_daily(rect, palette)?,
                Control::Tray => {
                    self.canvas.text("Tray icon", layout::TRAY_LABEL, Font::Body, Align::Leading, palette.text)?;
                    self.draw_switch(rect, self.tray, palette)?;
                }
            }
            if self.focus == Some(control) {
                let ring = Rect::new(rect.x - 3.0, rect.y - 3.0, rect.w + 6.0, rect.h + 6.0);
                self.canvas.stroke(ring, CORNER + 3.0, palette.text, 2.0)?;
            }
        }

        if !self.canvas.end()? {
            self.invalidate();
        }
        Ok(())
    }

    fn draw_now_showing(&self, palette: &Palette) -> Result<()> {
        match &self.preview {
            Some(bitmap) => self.canvas.image(bitmap, layout::PREVIEW, CORNER + 2.0)?,
            None => self.canvas.fill(layout::PREVIEW, CORNER + 2.0, palette.surface)?,
        }
        let (title, credit) = match self.applied() {
            Some(applied) => (applied.title.as_str(), applied.credit.as_str()),
            None => ("Current wallpaper", "Press Next for a new one"),
        };
        let hovered = self.hover == Some(Control::OpenPage) && self.applied().is_some();
        let title_color = if hovered { palette.accent } else { palette.text };
        self.canvas.text(title, layout::TITLE, Font::Title, Align::Leading, title_color)?;
        self.canvas.text(credit, layout::CREDIT, Font::Small, Align::Leading, palette.muted)
    }

    fn draw_section(&self, title: &str, rect: Rect, palette: &Palette) -> Result<()> {
        self.canvas.text(title, rect, Font::Label, Align::Leading, palette.muted)
    }

    fn draw_next(&self, rect: Rect, palette: &Palette) -> Result<()> {
        let hovered = self.hover == Some(Control::Next);
        let (fill, text) = match (self.busy, hovered) {
            (true, _) => (palette.surface, palette.muted),
            (false, true) => (palette.accent_hover, palette.on_accent),
            (false, false) => (palette.accent, palette.on_accent),
        };
        self.canvas.fill(rect, CORNER, fill)?;
        let label = if self.busy { "Working" } else { "Next" };
        self.canvas.text(label, rect, Font::Body, Align::Center, text)
    }

    fn draw_chip(&self, control: Control, label: &str, selected: bool, rect: Rect, palette: &Palette) -> Result<()> {
        let hovered = self.hover == Some(control);
        let dim = if self.is_active(control) { 1.0 } else { 0.4 };
        // Selected chips stay quiet, leaving Next as the one primary action.
        let (fill, border, text) = match (selected, hovered && dim == 1.0) {
            (true, false) => (palette.accent.with_alpha(0.16), palette.accent, palette.accent_text),
            (true, true) => (palette.accent.with_alpha(0.24), palette.accent, palette.accent_text),
            (false, false) => (palette.surface, palette.border, palette.text),
            (false, true) => (palette.surface_hover, palette.border, palette.text),
        };
        self.canvas.fill(rect, CORNER, fill.with_alpha(fill.1 * dim))?;
        self.canvas.stroke(rect, CORNER, border.with_alpha(dim), 1.0)?;
        self.canvas.text(label, rect, Font::Body, Align::Center, text.with_alpha(dim))
    }

    fn draw_daily(&self, rect: Rect, palette: &Palette) -> Result<()> {
        let y = layout::DIVIDER_Y;
        self.canvas.line((layout::DAILY_LABEL.x, y), (rect.x + rect.w, y), palette.border)?;
        // Only a failure earns a second line. On its own, the label lines up with the switch.
        let error = self.notice.as_ref().or(self.state.last_error.as_ref());
        let label = if error.is_some() { layout::DAILY_LABEL } else { Rect { y: rect.y, ..layout::DAILY_LABEL } };
        self.canvas.text("Change daily", label, Font::Body, Align::Leading, palette.text)?;
        if let Some(error) = error {
            self.canvas.text(error, layout::DAILY_STATUS, Font::Small, Align::Leading, palette.warning)?;
        }
        self.draw_switch(rect, self.daily, palette)
    }

    fn draw_switch(&self, rect: Rect, on: bool, palette: &Palette) -> Result<()> {
        let radius = rect.h / 2.0;
        let center_y = rect.y + radius;
        if on {
            self.canvas.fill(rect, radius, palette.accent)?;
            self.canvas.circle(rect.x + rect.w - radius, center_y, 7.0, palette.on_accent)
        } else {
            self.canvas.stroke(rect, radius, palette.muted, 1.0)?;
            self.canvas.circle(rect.x + radius, center_y, 6.0, palette.muted)
        }
    }
}

fn hit_test(x: f32, y: f32) -> Option<Control> {
    layout::controls().into_iter().find(|(_, rect)| rect.contains(x, y)).map(|(control, _)| control)
}

fn colorref(color: Color) -> COLORREF {
    // COLORREF is 0x00BBGGRR.
    let rgb = color.0;
    COLORREF(((rgb & 0xFF) << 16) | (rgb & 0xFF00) | (rgb >> 16))
}

fn center_on_screen(hwnd: HWND) {
    let mut window = RECT::default();
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: the out-pointers are valid and `hwnd` is a live window.
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if GetWindowRect(hwnd, &mut window).is_ok() && GetMonitorInfoW(monitor, &mut info).as_bool() {
            let area = info.rcWork;
            let x = area.left + (area.right - area.left - (window.right - window.left)) / 2;
            let y = area.top + (area.bottom - area.top - (window.bottom - window.top)) / 2;
            let _ = SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }
}

/// `BACKREEL_THEME=light|dark` overrides the system setting, for checking both themes.
fn uses_dark_theme() -> bool {
    match std::env::var("BACKREEL_THEME").as_deref() {
        Ok("light") => return false,
        Ok("dark") => return true,
        _ => {}
    }
    let mut value = 1u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: `value` and `size` describe a writable u32.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&mut size),
        )
    };
    status == ERROR_SUCCESS && value == 0
}
