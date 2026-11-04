//! The Windows desktop: monitors and their wallpapers.

use crate::Result;
use std::path::{Path, PathBuf};
use windows::Win32::System::Com::{CLSCTX_LOCAL_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Shell::{DWPOS_FILL, DesktopWallpaper, IDesktopWallpaper};
use windows::core::{HSTRING, PWSTR};

#[derive(Clone, Debug)]
pub struct Monitor {
    /// Windows' stable device path.
    pub id: String,
    pub width: u32,
    pub height: u32,
}

pub struct Desktop {
    wallpaper: IDesktopWallpaper,
}

impl Desktop {
    pub fn new() -> Result<Self> {
        // SAFETY: COM is initialised on this thread by the caller.
        let wallpaper = unsafe { CoCreateInstance(&DesktopWallpaper, None, CLSCTX_LOCAL_SERVER)? };
        Ok(Self { wallpaper })
    }

    /// The monitors currently attached, in Windows' order.
    pub fn monitors(&self) -> Result<Vec<Monitor>> {
        let mut monitors = Vec::new();
        // SAFETY: indices are bounded by the reported count.
        unsafe {
            for index in 0..self.wallpaper.GetMonitorDevicePathCount()? {
                let path = self.wallpaper.GetMonitorDevicePathAt(index)?;
                let id = take_string(path);
                // Detached entries fail here or report S_FALSE, which the wrapper
                // cannot distinguish, so an empty rectangle is treated as detached too.
                let Ok(rect) = self.wallpaper.GetMonitorRECT(&HSTRING::from(&id)) else { continue };
                let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
                if width > 0 && height > 0 {
                    monitors.push(Monitor { id, width: width as u32, height: height as u32 });
                }
            }
        }
        Ok(monitors)
    }

    /// Sets `path` on one monitor and confirms Windows now reports it.
    pub fn set(&self, monitor: &Monitor, path: &Path) -> Result<()> {
        let id = HSTRING::from(&monitor.id);
        // SAFETY: both strings outlive the calls.
        unsafe {
            self.wallpaper.SetPosition(DWPOS_FILL)?;
            self.wallpaper.SetWallpaper(&id, &HSTRING::from(path))?;
        }
        match self.current(monitor) {
            Some(current) if current == path => Ok(()),
            current => Err(format!("wallpaper did not take effect; Windows reports {current:?}").into()),
        }
    }

    /// The wallpaper file Windows reports for `monitor`.
    pub fn current(&self, monitor: &Monitor) -> Option<PathBuf> {
        // SAFETY: the monitor string outlives the call; the result is freed by `take_string`.
        let path = unsafe { self.wallpaper.GetWallpaper(&HSTRING::from(&monitor.id)) }.ok()?;
        let path = take_string(path);
        (!path.is_empty()).then(|| PathBuf::from(path))
    }
}

/// Copies a COM-allocated string and frees the original.
fn take_string(value: PWSTR) -> String {
    // SAFETY: `value` is a valid NUL-terminated string that this function owns.
    unsafe {
        let text = value.to_string().unwrap_or_default();
        CoTaskMemFree(Some(value.0.cast()));
        text
    }
}
