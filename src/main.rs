//! backreel: a daily wallpaper with nothing left running.
//!
//! Task Scheduler runs `backreel --run`, which changes the wallpaper and exits.
//! With no arguments the same exe opens the settings window, and `--tray` runs
//! the optional tray icon.

#![windows_subsystem = "windows"]

mod desktop;
mod http;
mod imaging;
mod run;
mod schedule;
mod sources;
mod store;
mod tray;
mod ui;

use run::{Outcome, Trigger};
use std::process::ExitCode;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::Threading::{
    GetCurrentProcess, PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetProcessInformation,
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Initialises COM for the current thread for as long as it lives.
pub struct Com;

impl Com {
    pub fn init() -> Result<Self> {
        // SAFETY: balanced by `CoUninitialize` in `Drop`, on the same thread.
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
        Ok(Self)
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        // SAFETY: pairs with the successful `CoInitializeEx` in `init`.
        unsafe { CoUninitialize() };
    }
}

fn main() -> ExitCode {
    let argument = std::env::args().nth(1);
    let result = Com::init().and_then(|_com| match argument.as_deref() {
        None => ui::run(),
        Some("--run") => {
            run_in_background();
            change(Trigger::Scheduled)
        }
        Some("--next") => change(Trigger::Manual),
        Some("--tray") => tray::run(),
        Some("--install") => schedule::enable(),
        Some("--uninstall") => schedule::disable(),
        Some(other) => Err(format!("unknown argument: {other}").into()),
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            store::log(&format!("error: {error}"));
            ExitCode::FAILURE
        }
    }
}

fn change(trigger: Trigger) -> Result<()> {
    let outcome = run::run(trigger)?;
    if outcome != Outcome::AlreadyDone {
        store::log(&format!("{trigger:?}: {outcome:?}"));
    }
    Ok(())
}

/// Opts this process into EcoQoS: nobody is waiting on a scheduled run.
fn run_in_background() {
    let throttle = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    };
    // SAFETY: the pointer and size describe `throttle`. Failure only means normal priority.
    let _ = unsafe {
        SetProcessInformation(
            GetCurrentProcess(),
            ProcessPowerThrottling,
            (&raw const throttle).cast(),
            size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        )
    };
}
