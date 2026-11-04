//! Registers backreel with Task Scheduler, so nothing needs to stay resident.

use crate::{Result, store, tray};
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::core::{HSTRING, Interface};

const TASK_NAME: &str = "backreel";
const TRAY_TASK: &str = "backreel tray";

/// Runs just after midnight, then hourly: a run that fails for lack of network
/// retries an hour later, and once the day's change is done each run exits in
/// milliseconds. (`RestartOnFailure` is no help here: it was tested and does
/// not react to a non-zero exit code.) The logon trigger covers a PC that was
/// off overnight.
fn daily_task_xml(exe: &Path, user: &str) -> String {
    let triggers = format!(
        "<CalendarTrigger>
      <Repetition><Interval>PT1H</Interval><Duration>P1D</Duration></Repetition>
      <StartBoundary>2026-01-01T00:05:00</StartBoundary>
      <ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay>
    </CalendarTrigger>
    <LogonTrigger><Delay>PT1M</Delay><UserId>{}</UserId></LogonTrigger>",
        escape_xml(user)
    );
    task_xml(&triggers, "PT5M", 7, exe, "--run")
}

/// Runs for the whole session, at normal priority because the settings window
/// inherits it.
///
/// The tray is a task rather than a Run key because security software treats an
/// unsigned exe that writes a Run key and launches a fresh copy of itself as
/// malware: Kaspersky was seen freezing both processes.
fn tray_task_xml(exe: &Path, user: &str) -> String {
    let triggers = format!("<LogonTrigger><UserId>{}</UserId></LogonTrigger>", escape_xml(user));
    task_xml(&triggers, "PT0S", 5, exe, "--tray")
}

/// All battery conditions are off because their defaults would silently skip laptops.
fn task_xml(triggers: &str, time_limit: &str, priority: u8, exe: &Path, argument: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Part of backreel, the daily wallpaper.</Description></RegistrationInfo>
  <Triggers>
    {triggers}
  </Triggers>
  <Principals>
    <Principal id="Author"><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <ExecutionTimeLimit>{time_limit}</ExecutionTimeLimit>
    <Priority>{priority}</Priority>
  </Settings>
  <Actions Context="Author"><Exec><Command>{exe}</Command><Arguments>{argument}</Arguments></Exec></Actions>
</Task>
"#,
        exe = escape_xml(&exe.to_string_lossy()),
    )
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn schtasks(args: &[&str]) -> Result<Output> {
    Ok(Command::new("schtasks.exe").args(args).creation_flags(CREATE_NO_WINDOW.0).output()?)
}

fn checked(output: Output) -> Result<()> {
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("schtasks: {}", String::from_utf8_lossy(&output.stderr).trim()).into())
    }
}

fn task_exists(name: &str) -> bool {
    schtasks(&["/Query", "/TN", name]).is_ok_and(|output| output.status.success())
}

fn register(name: &str, xml: &str) -> Result<()> {
    let xml_path = store::home().join("task.xml");
    fs::write(&xml_path, utf16_with_bom(xml))?;
    let result = schtasks(&["/Create", "/TN", name, "/XML", &xml_path.to_string_lossy(), "/F"]);
    let _ = fs::remove_file(&xml_path);
    checked(result?)
}

fn remove(name: &str) -> Result<()> {
    if task_exists(name) { checked(schtasks(&["/Delete", "/TN", name, "/F"])?) } else { Ok(()) }
}

fn start(name: &str) -> Result<()> {
    checked(schtasks(&["/Run", "/TN", name])?)
}

pub fn is_enabled() -> bool {
    task_exists(TASK_NAME)
}

/// Installs the running exe into backreel's folder, then schedules that copy,
/// so the task keeps working if the original file is moved or deleted.
pub fn enable() -> Result<()> {
    let exe = install_exe()?;
    register(TASK_NAME, &daily_task_xml(&exe, &std::env::var("USERNAME")?))?;
    create_shortcut(&exe)
}

pub fn disable() -> Result<()> {
    remove(TASK_NAME)?;
    match fs::remove_file(shortcut_path()?) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

/// Shows the tray icon now and at every logon.
pub fn enable_tray() -> Result<()> {
    tray::stop();
    let exe = install_exe()?;
    register(TRAY_TASK, &tray_task_xml(&exe, &std::env::var("USERNAME")?))?;
    start(TRAY_TASK)
}

pub fn disable_tray() -> Result<()> {
    tray::stop();
    remove_tray_task()
}

pub fn remove_tray_task() -> Result<()> {
    remove(TRAY_TASK)
}

/// Copies the running exe into backreel's folder. A running tray locks the old
/// copy, so it is closed for the swap and comes back on the new version.
fn install_exe() -> Result<PathBuf> {
    let current = std::env::current_exe()?;
    let dir = store::home();
    fs::create_dir_all(&dir)?;
    let installed = dir.join("backreel.exe");
    if !same_file(&current, &installed) {
        let tray_was_running = tray::stop();
        fs::copy(&current, &installed)?;
        if tray_was_running {
            start(TRAY_TASK)?;
        }
    }
    Ok(installed)
}

fn same_file(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

fn utf16_with_bom(text: &str) -> Vec<u8> {
    std::iter::once(0xFEFF).chain(text.encode_utf16()).flat_map(u16::to_le_bytes).collect()
}

fn shortcut_path() -> Result<PathBuf> {
    let appdata = std::env::var_os("APPDATA").ok_or("APPDATA is not set")?;
    Ok(PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs\backreel.lnk"))
}

fn create_shortcut(exe: &Path) -> Result<()> {
    // SAFETY: COM is initialised by the caller and the strings outlive the calls.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(&HSTRING::from(exe))?;
        link.cast::<IPersistFile>()?.Save(&HSTRING::from(shortcut_path()?.as_path()), true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_xml_escapes_paths_and_users() {
        let xml = daily_task_xml(Path::new(r"C:\A & B\backreel.exe"), "R&D<user>");
        assert!(xml.contains(r"C:\A &amp; B\backreel.exe"));
        assert!(xml.contains("R&amp;D&lt;user&gt;"));
    }

    #[test]
    fn task_never_waits_on_battery_state() {
        let xml = daily_task_xml(Path::new("x"), "u");
        assert!(xml.contains("<DisallowStartIfOnBatteries>false"));
        assert!(xml.contains("<StopIfGoingOnBatteries>false"));
    }

    #[test]
    fn tray_task_has_no_time_limit_and_normal_priority() {
        let xml = tray_task_xml(Path::new("x"), "u");
        assert!(xml.contains("<ExecutionTimeLimit>PT0S"));
        assert!(xml.contains("<Priority>5"));
        assert!(xml.contains("<Arguments>--tray"));
    }

    #[test]
    fn xml_is_utf16_le_with_bom() {
        assert_eq!(utf16_with_bom("a"), [0xFF, 0xFE, b'a', 0]);
    }
}
