# Windows mechanics for backreel: wallpaper APIs and zero-idle daily execution

Evidence labels used throughout:
- **[Local check]**: read-only observation on this machine (Windows 11 Home, 25H2, build 26200.9711) on 2026-11-04. One machine, one monitor, so treat as a data point, not a rule.
- **[Anecdote]**: forum, blog or community report with no official confirmation.
- Unlabelled claims with a learn.microsoft.com link are from official docs read in this session.
- Search-summarised sources are secondary. Where the search summary did not pin a claim to one page, the note says so.

## Objective: what is the lowest-footprint runtime architecture?

### Takeaway
Use no resident process at all. Register one per-user Scheduled Task (InteractiveToken, no elevation) that launches a small native Win32 exe, which does its work and exits. Use `IDesktopWallpaper` to set the wallpaper, WinHTTP (Schannel) for network and WIC for images. Open settings by running the same exe with no flag. There is no tray icon. Idle RAM and CPU attributable to backreel are zero, because the Task Scheduler service already runs anyway (Automatic start here).

### Cited Findings
- The Schedule service is already running with Automatic start, hosting 208 registered tasks, on this Home machine — [Local check].
- Microsoft's own Bing Wallpaper uses a resident process and a daily task: `BingWallpaper.exe` was resident at 192.3 MB working set / 109.2 MB private. This is the footprint backreel is competing against — [Local check].
- Services cannot directly interact with a user as of Vista, and all services run in session 0 — [Interactive Services](https://learn.microsoft.com/en-us/windows/win32/services/interactive-services).
- A non-admin can register a task under their own account with no password using the interactive or S4U logon type — [Security Contexts for Tasks](https://learn.microsoft.com/en-us/windows/win32/taskschd/security-contexts-for-running-tasks).

### Inferences
Recommended stack, each element justified in the sections below:

| Concern | Choice | Reason |
|---|---|---|
| Scheduling | Per-user Scheduled Task, CalendarTrigger + LogonTrigger, StartWhenAvailable | Zero idle cost, no admin, catches missed runs |
| Process | One native exe, `/SUBSYSTEM:WINDOWS`, `--run` flag for headless | No runtime, no console flash, one artefact |
| Wallpaper | `IDesktopWallpaper::SetWallpaper` per monitor; `SystemParametersInfoW` as fallback | Only documented per-monitor API |
| Images | WIC, pre-cropped to exact monitor pixels, unique filename per image | No bundled codecs; Windows does no scaling |
| Network | WinHTTP sync, `AUTOMATIC_PROXY`, ETag / If-None-Match | In-box TLS, no bundled stack |
| Metered | `INetworkCostManager::GetCost`, skip on non-`UNRESTRICTED` by default | Plain COM, no WinRT projection needed |
| Power | Task Priority 7 (default) + EcoQoS on startup | Scheduler supplies low I/O and memory priority for free |
| Storage | `%LOCALAPPDATA%\backreel` | Per-user, no admin |
| Tray | None | A tray icon requires a live process |

Target run profile: seconds per day of work, then exit.

### Gaps
- No end-to-end measurement of a real backreel run exists yet, because no prototype was built or run.

## Wallpaper APIs: IDesktopWallpaper vs SystemParametersInfoW vs WinRT

### Takeaway
`IDesktopWallpaper` is the only documented per-monitor API and should be primary. `SystemParametersInfoW` is a simple all-monitor fallback. The WinRT API adds nothing for the desktop except possibly the lock screen. Position is global, so pre-crop each image to the exact monitor size and use `DWPOS_FILL`. Windows appears to re-encode JPEGs into its own cache (community reports plus one local observation; not documented by Microsoft).

### Cited Findings

**IDesktopWallpaper**
- Minimum client is Windows 8, desktop apps only; header `shobjidl_core.h`. Methods include SetWallpaper, GetWallpaper, GetMonitorDevicePathAt/Count, GetMonitorRECT, SetPosition/GetPosition, SetSlideshow, SetSlideshowOptions, AdvanceSlideshow, GetStatus and Enable — [IDesktopWallpaper](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-idesktopwallpaper).
- `SetWallpaper(monitorID, path)` takes a full path; a NULL monitorID applies the image to all monitors — [SetWallpaper](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-idesktopwallpaper-setwallpaper).
- `GetWallpaper(NULL)` returns S_FALSE and an empty string if monitors differ or a slideshow is running. The string is also empty for a solid colour — [GetWallpaper](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-idesktopwallpaper-getwallpaper).
- `GetMonitorRECT` returns **S_FALSE when the monitor is not attached** and E_INVALIDARG when the ID is unknown. S_FALSE passes a `SUCCEEDED()` test, so check for S_OK explicitly — [GetMonitorRECT](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-idesktopwallpaper-getmonitorrect).
- Position values: CENTER 0, TILE 1, STRETCH 2, FIT 3, FILL 4, SPAN 5. FILL crops to avoid letterboxing; SPAN spreads one image across all monitors — [DESKTOP_WALLPAPER_POSITION](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/ne-shobjidl_core-desktop_wallpaper_position).
- `GetStatus` flags: DSS_ENABLED 0x01, DSS_SLIDESHOW 0x02 (a slideshow is configured), DSS_DISABLED_BY_REMOTE_SESSION 0x04 — [GetStatus](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-idesktopwallpaper-getstatus).
- COM setup: CLSID `{C2CF3110-460E-4fc1-B9D0-8A1C0C9CC4BD}` has an AppId `{8B30085D-A3E3-44e3-AE7F-B03A1340EBED}` and no InprocServer32 or LocalServer32 subkey — [Local check]. All getters worked from an STA thread — [Local check].
- Apartment model is contested. A forum post says it must be used from an STA thread — [dostips, Anecdote](https://www.dostips.com/forum/viewtopic.php?p=66196). WinDynamicDesktop calls it on the calling thread with no STA setup, while marking only its `IActiveDesktop` fallback "STA (required!)" — [WinDynamicDesktop WallpaperApi.cs](https://raw.githubusercontent.com/t1m0thyj/WinDynamicDesktop/master/src/WallpaperApi.cs). The Microsoft docs say nothing on this.
- WinDynamicDesktop sends undocumented message 0x52C to Progman before setting wallpaper to enable transitions, and ignores the return values — [WallpaperApi.cs](https://raw.githubusercontent.com/t1m0thyj/WinDynamicDesktop/master/src/WallpaperApi.cs).

**SystemParametersInfoW (SPI_SETDESKWALLPAPER, 0x0014)**
- Returns TRUE unless there is an error such as a missing file. SPI_GETDESKWALLPAPER describes a "bitmap file" path no longer than MAX_PATH. `SPIF_UPDATEINIFILE` writes to the user profile and `SPIF_SENDCHANGE` broadcasts WM_SETTINGCHANGE — [SystemParametersInfoW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfow).
- The usual call is `SystemParametersInfo(SPI_SETDESKWALLPAPER, 0, path, SPIF_UPDATEINIFILE | SPIF_SENDCHANGE)`. It sets one image and does not restore Slideshow or Spotlight — community scripts in [Microsoft Q&A](https://learn.microsoft.com/en-us/answers/a/1627500).
- Writing the `Wallpaper` registry value directly does not take effect until Explorer restarts — [AutoIt forum, Anecdote](https://autoitscript.com/forum/topic/180510-unable-to-change-desktop-wallpaper-windows-7-pro).

**WinRT UserProfilePersonalizationSettings**
- `TrySetWallpaperImageAsync(StorageFile)` returns bool and has no monitor parameter. Docs check `IsSupported()` first and use an `ms-appx:///Local/` file — [TrySetWallpaperImageAsync](https://learn.microsoft.com/en-us/uwp/api/windows.system.userprofile.userprofilepersonalizationsettings.trysetwallpaperimageasync).
- Documented bug-like constraint: **a new image must have a different file name than the previously set image, or the call fails** — [UserProfilePersonalizationSettings](https://learn.microsoft.com/en-us/uwp/api/windows.system.userprofile.userprofilepersonalizationsettings).
- A UWP app crashed with 0x80010012 until the image was first copied to LocalFolder — [mirrored Stack Overflow, Anecdote](https://windows-hexerror.linestarve.com/q/so35931399-uwp-release-build-crashes-but-not-in-visual-studio).
- This class does not appear in Microsoft's lists of WinRT APIs unsupported in desktop apps or requiring package identity (updated 2026-07-22), but that page says the list might not be complete — [WinRT APIs not supported in desktop apps](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/winrt-api-desktop-app-support).

**JPEG recompression and TranscodedWallpaper**
- Windows stores a converted copy as extensionless `TranscodedWallpaper` in `%AppData%\Microsoft\Windows\Themes`, and the desktop loads from it. A DWORD `JPEGImportQuality` under `HKCU\Control Panel\Desktop`, set to 100, is the commonly cited fix — [NinjaOne](https://www.ninjaone.com/blog/disable-wallpaper-compression-in-windows/), [The Windows Club](https://www.thewindowsclub.com/disable-windows-10-wallpaper-compression), all **[Anecdote]**: no Microsoft documentation for this value turned up.
- Community sources describe compression "by 15-20%"; one tester found 100 gave no visible gain and only a larger file (exact page not isolated by the search summary; see [Eleven Forum](https://www.elevenforum.com/t/disable-desktop-wallpaper-jpeg-quality-reduction-in-windows-11.10782), which returned 403 on direct fetch). A Japanese tester found sensible values are 60-100, and deleting the value restores default — [Mynavi](https://news.mynavi.jp/article/windows-402/).
- Observed here: source JPEG 3840x2160, 875,435 bytes; `TranscodedWallpaper` is a JPEG (magic `FF D8 FF E0`), also 3840x2160, 939,825 bytes, on a 2560x1440 monitor. So it was re-encoded (size differs) but **not** downscaled to the monitor. `JPEGImportQuality` is unset — [Local check]. Caveat: with several virtual desktops, the cache file may belong to a different image than the one compared, though the dimensions would match either way.
- A `TranscodedWallpaperCache` folder holds 4 hashed `TranscodedWallpaper_xxxxxxxx` files (0.9-2.3 MB) each with a 540-byte `.meta` file. This layout appears in none of the sources found — [Local check].

**File formats**
- MDM documentation specifies jpg, jpeg or png for desktop images — [Personalization CSP](https://learn.microsoft.com/en-us/windows/client-management/mdm/personalization-csp).
- HDR `.jxr` wallpapers appeared in preview build 23516 (August 2023) and are available in current releases via Settings — [Neowin](https://www.neowin.net/news/windows-11-now-supports-hdr-wallpapers-in-jxr-format/), [Pureinfotech](https://pureinfotech.com/set-hdr-jxr-wallpapers-windows-11/).

**Virtual desktops**
- `HKCU\...\Explorer\VirtualDesktops\Desktops\{GUID}` keys hold a per-desktop `Wallpaper` value. This machine has 292 such keys, many pointing at older Bing images, which suggests they accumulate — [Local check].
- Dev build 25197 added a Settings option to apply a wallpaper to all desktops; before that changes hit only the current desktop — [Neowin](https://www.Neowin.Net/news/windows-11-build-25197-finally-lets-you-change-wallpaper-on-all-desktops-at-once/).
- Per-desktop wallpaper is reachable only through the undocumented `IVirtualDesktopManagerInternal`. Wrapper libraries pin specific builds; winvd requires at least 26100.2605 — [winvd](https://docs.rs/crate/winvd/latest/source/src/interfaces.rs), [VirtualDesktop module](https://powershellgallery.com/packages/VirtualDesktop).
- A Microsoft Q&A answer says Windows doesn't support changing virtual-desktop registry keys — [Microsoft Q&A](https://learn.microsoft.com/en-us/answers/a/12251137); a June 2024 forum post deleted those values anyway as a fix — [Tech Community, Anecdote](https://techcommunity.microsoft.com/discussions/edgeinsiderdiscussions/desktop-slideshow-doesnt-work-with-multiple-virtual-desktops/4168345).

**Lock screen without packaging**
- `TrySetLockScreenImageAsync(StorageFile)` is the only per-user programmatic API found; the same different-filename rule applies — [UserProfilePersonalizationSettings](https://learn.microsoft.com/en-us/uwp/api/windows.system.userprofile.userprofilepersonalizationsettings).
- The Personalization CSP (`LockScreenImageUrl`, `DesktopImageUrl`) is supported on Enterprise and Education, on Pro only in SharedPC/BootToCloud configurations, is device-scoped, and **also prevents the user changing the image**. Home is not listed — [Personalization CSP](https://learn.microsoft.com/en-us/windows/client-management/mdm/personalization-csp).
- "Force a specific default lock screen" is device-scoped and sets a default for all users — [ADMX_ControlPanelDisplay](https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-admx-controlpaneldisplay).
- Package identity without full MSIX is possible through packaging with external location (build 19041+, `AllowExternalContent`). It needs a package signed by a certificate trusted on the target machine, in production a real CA or Azure Trusted Signing — [Grant package identity](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-nonpackaged-apps).

**Spotlight and slideshow**
- `HKCU\...\Explorer\Wallpapers\BackgroundType` records the mode: image, colour, slideshow or Spotlight — [Microsoft Q&A](https://learn.microsoft.com/en-us/answers/a/1627500). Community values: 0 picture, 1 solid colour, 3 Spotlight — [Eleven Forum tutorial, Anecdote](https://www.elevenforum.com/t/change-desktop-background-to-spotlight-picture-solid-color-or-slideshow-in-windows-11.909/latest). The search summary noted sources disagreed, so verify before relying on it. Value 2 for slideshow is implied, not confirmed.
- Spotlight has a second switch: `HKCU\...\DesktopSpotlight\Settings\EnabledState`, paired with BackgroundType in community guides — [Eleven Forum, Anecdote](https://www.elevenforum.com/t/enable-or-disable-windows-spotlight-on-desktop-background-in-windows-11.18246/latest). The value exists here and is 0 — [Local check].
- Spotlight can be policy-disabled through `Software\Policies\Microsoft\Windows\CloudContent` — [geekrewind, Anecdote](https://geekrewind.com/turn-off-windows-spotlight-on-the-desktop-background-in-windows-11/).
- "Slideshow is not supported with multiple desktops" is claimed in an AI-generated Microsoft Q&A answer citing a support article; the wording was not confirmed against that article and the original poster attributed the issue to multiple monitors — [Microsoft Q&A](https://learn.microsoft.com/en-us/answers/questions/5837638/windows-11-when-i-select-slideshow-desktop-backgro).
- Slideshow reverting to a static picture on desktop switch is reported with no documented cause — [Microsoft Q&A](https://learn.microsoft.com/en-us/answers/a/12251137).

**24H2/25H2/26H2 changes**
- Microsoft placed safeguard hold 52754008 on 24H2 in September 2024 for wallpaper customisation apps (wrong display, missing icons, virtual desktop issues) and began lifting it in April 2025. It never published the app list or root cause — [The Register](https://www.theregister.com/2025/04/14/windows_11_24h2_hold/), [BetaNews](https://betanews.com/article/microsoft-says-that-windows-11-24h2-now-plays-nicely-with-wallpaper-customization-applications), [Neowin](https://www.neowin.net/news/microsoft-finally-removes-a-six-month-old-windows-11-24h2-update-block/). The Microsoft release-health page itself was not read.
- 26H2 shipped on 29 September 2026 as an enablement package for 24H2/25H2 PCs; 26H1 (build 28000, new Arm hardware) has a different Windows core and does not receive it. No wallpaper-related changes appear in the coverage found — [Pureinfotech](https://pureinfotech.com/windows-11-26h2-released-force-install/), [IT-Connect](https://www.it-connect.tech/windows-11-26h2-is-here-whats-new-support-and-how-to-install-it/), [Windows Latest](https://www.windowslatest.com/2026/10/06/microsoft-reveals-whats-actually-new-in-windows-11-26h2-and-how-to-verify-it/).

### Inferences
- `SetPosition` takes no monitor ID (the enum docs and method list describe one display option), so one mode applies everywhere. If each image is pre-cropped to its monitor's exact pixels, FILL scales by 1.0 and the mode stops mattering.
- Call `CoInitializeEx(COINIT_APARTMENTTHREADED)` on the worker thread. It works here, and it satisfies the stricter of the conflicting claims at no cost.
- The missing server subkeys suggest the object is hosted out of process, likely by Explorer, which would mean it needs a running shell. Handle `CoCreateInstance` failure by exiting and leaving the day's run pending.
- Always give each image a unique filename (date + hash). The rule is only documented for WinRT, but it costs nothing and also gives atomic swaps.
- Verify after setting: call `GetWallpaper(monitorID)` and compare. `SPI_SETDESKWALLPAPER` reports TRUE fairly unconditionally, so success codes alone are weak evidence.
- Since re-encoding happens anyway, the best achievable quality is a high-quality source at exact resolution. Do not write `JPEGImportQuality` silently. It is an undocumented, machine-behaviour tweak, so offer it as an explicit opt-in if testing shows a benefit.
- Virtual desktops: use the documented API only, and accept that behaviour is whatever Windows does. Avoid `IVirtualDesktopManagerInternal`, since it needs per-build maintenance and hooks shell internals, the same territory as the apps caught by the 24H2 hold (the actual trigger for that hold is unknown).
- Spotlight or slideshow active: read `BackgroundType` and `GetStatus`. Ask once on first run rather than silently replacing the user's mode, and then treat the choice as settled.
- Lock screen: leave it out of v1. Try the WinRT call from an unpackaged process in a test before committing. If it fails, identity packaging adds signing cost that conflicts with the minimal-app goal.
- Do not use the 0x52C message. It is undocumented and only adds a cosmetic fade.

### Gaps
- Not documented or tested: whether `IDesktopWallpaper::SetWallpaper` affects all virtual desktops or only the current one. Testing was out of scope, because it would change the wallpaper.
- Not documented: what `SetWallpaper` does to an active slideshow or Spotlight. No authoritative source was found, and it needs a test on a spare profile.
- No confirmation that `TrySetLockScreenImageAsync` works or fails from an unpackaged exe.
- No confirmation that the same-path "no refresh" problem applies to `IDesktopWallpaper`. The only reference is a report of a change not appearing until a desktop click.
- Unknown whether PNG input avoids JPEG re-encoding in `TranscodedWallpaper`. No reliable source was found, and it needs a test.
- No official list of formats accepted by each wallpaper API was found. `SPI` docs say only "bitmap file", though JPEG works in practice as the Bing app's `.jpg` paths show ([Local check]).
- `SetPosition`'s signature was not fetched directly; the "global" claim rests on the method list, the enum page and memory.
- The purpose of `.meta` files in `TranscodedWallpaperCache` is unknown.

## Persistent process vs no process: idle cost, tray handling and scheduling

### Takeaway
A Scheduled Task has zero attributable idle cost; a tray app always pays something; a service is the wrong tool; a Run key alone cannot do daily work. A standard user can create the task without admin rights. The defaults are traps: battery settings default to *not running on battery*, so they must be set explicitly. StartWhenAvailable with a logon trigger catches missed runs.

### Cited Findings

**Idle cost comparison**

| Option | Idle RAM | Idle CPU | Admin | Daily work? | Notes |
|---|---|---|---|---|---|
| (a) Tray resident | Non-zero always | Near zero if blocked in message loop | No | Yes, own timer | Examples below |
| (b) Task + short-lived exe | Zero (no process) | Zero | No | Yes | Task definition only |
| (c) Service | Non-zero always | Near zero | Yes | Yes | Cannot touch user desktop directly |
| (d) Run key | Zero if exe exits | Zero | No (HKCU) | Logon only | No guaranteed timing |

- Resident examples on this machine (working set / private): dusty 36.9 / 18.1 MB, ditto-desktop 56.5 / 20.7 MB, ditto-daemon 65.9 / 33.2 MB, BingWallpaper 192.3 / 109.2 MB — [Local check]. These are not minimal Win32 tray apps; they show what real resident apps cost, not the floor.
- Services: Windows offers only indirect interaction: a separate GUI app launched with `CreateProcessAsUser`, plus IPC, and a per-session Run entry on multiuser systems — [Interactive Services](https://learn.microsoft.com/en-us/windows/win32/services/interactive-services).
- Run keys: command line limited to 260 characters, order indeterminate, and the system "may choose to delay" execution with no promptness guarantee — [Run and RunOnce](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys).

**Tray icon with nothing resident**
- Notify icon events arrive as the app-defined `uCallbackMessage`, so there must be a live window to receive them. `NIM_ADD`/`NIM_DELETE` manage the icon. On Windows 11 banner notifications from `Shell_NotifyIcon` are transient and do not stay in Notification Center — [Shell_NotifyIconW](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyiconw).
- Package identity is listed as the route to toast notifications, startup tasks and background tasks — [Grant package identity](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-nonpackaged-apps).

**Non-admin task creation**
- A non-admin registering under their own account with the S4U or interactive logon type needs no password. They cannot use Local Service or a group as the context — [Security Contexts](https://learn.microsoft.com/en-us/windows/win32/taskschd/security-contexts-for-running-tasks).
- From a low-privilege process you cannot register with `TASK_RUNLEVEL_HIGHEST`, but can with `TASK_RUNLEVEL_LUA`. You also cannot register as Local System or for a group — same source.
- S4U and password logon tasks only launch if the user has the Logon as Batch privilege, enabled by default for Administrators and Backup Operators — same source. So use InteractiveToken, not S4U.
- Users can read, update, delete and run only their own tasks — same source.
- With InteractiveToken and no credentials, the task uses the account that called `ITaskService::Connect` — same source.
- Conflict on logon triggers: a Russian Q&A reports standard users can create timer tasks but hit errors with `/SC ONLOGON` and `/ONSTART` — [Habr, Anecdote](https://qna.habr.com/q/1303758). Importing through `schtasks /create /xml` is reported as a workaround, and a Server Fault post saw COM `RegisterTask` give ACCESS_DENIED where XML import via schtasks worked — [Server Fault mirror, Anecdote](https://backiee.wasmer.app/https_serverfault_com/questions/482446/access-denied-when-creating-a-scheduled-task-with-powershell-and-the-com-schedul). No official statement covers this.
- "Prohibit New Task Creation" policy may not stop a standard user on Windows 11 — [Microsoft Q&A, Anecdote](https://learn.microsoft.com/en-gb/answers/questions/2198556/group-policy-prohibit-new-task-creation-is-applied).

**Triggers**
- A task can have up to 48 triggers, time-based or event-based — [Task Triggers](https://learn.microsoft.com/en-us/windows/win32/taskschd/task-triggers).
- Session state: `SessionStateChangeTrigger` has StateChange, UserId and Delay — [SessionStateChangeTrigger](https://learn.microsoft.com/en-us/windows/win32/taskschd/taskschedulerschema-sessionstatechangetrigger-triggergroup-element).
- Network: `Microsoft-Windows-NetworkProfile/Operational` Event ID 10000 = connected, 10001 = disconnected; attach with an "On an event" trigger — [winhelponline, Anecdote](https://www.winhelponline.com/blog/trigger-launch-program-script-connecting-to-specific-network/).
- That same guide says the task may not fire on Windows 10 Anniversary Update and later if a GUI network condition is set; use a custom XML trigger and no network condition — same source.
- Wake: System log, source Power-Troubleshooter, Event ID 1, is written on resume and can be an event trigger. It fires only after the machine has woken — [ITPro Today](https://itprotoday.com/it-infrastructure/q-my-machine-keeps-coming-out-of-sleep-how-can-i-find-out-why-), [Microsoft Q&A](https://learn.microsoft.com/en-us/answers/questions/1660115/regarding-waking-up-from-sleep-on-a-modern-standby).

**Settings and defaults** ([settingsType schema](https://learn.microsoft.com/en-us/windows/win32/taskschd/taskschedulerschema-settingstype-complextype))

| Element | Schema default | backreel value |
|---|---|---|
| DisallowStartIfOnBatteries | **true** | false |
| StopIfGoingOnBatteries | **true** | false |
| StartWhenAvailable | false | true |
| RunOnlyIfNetworkAvailable | false | false |
| WakeToRun | false | false |
| MultipleInstancesPolicy | IgnoreNew | IgnoreNew |
| Priority | 7 | 7 |
| RunOnlyIfIdle | false | false |
| Hidden | false | false |

- A task is stopped 72 hours after starting by default; change with ExecutionTimeLimit — [TaskSettings](https://learn.microsoft.com/en-us/windows/win32/taskschd/tasksettings).
- StartWhenAvailable "applies only to time-based tasks with an end boundary or time-based tasks that are set to repeat infinitely". Missed tasks are queued and start after a default **10-minute delay** — [StartWhenAvailable](https://learn.microsoft.com/en-us/windows/win32/taskschd/tasksettings-startwhenavailable).
- RunOnlyIfNetworkAvailable defaults to False and its docs give no more detail — [RunOnlyIfNetworkAvailable](https://learn.microsoft.com/en-us/windows/win32/taskschd/tasksettings-runonlyifnetworkavailable).

**A real example: Microsoft's Bing Wallpaper task** ([Local check])
- Task `\BingWallpaper_DailyCheck_<SID>` sits in the root folder with the SID in its name. Principal: user SID, `InteractiveToken`. Settings: both battery flags false, `ExecutionTimeLimit` PT5M, `IgnoreNew`, `StartWhenAvailable` true. Two daily `CalendarTrigger`s (08:29 and 17:17), no `Priority` element so it gets default 7. LastTaskResult 0.
- Its action launches `UnInstDaemon.exe --daily-app-recovery` from a GUID-named folder under `%TEMP%`.

**API and CLI**
- Registration: `ITaskService::Connect`, then `ITaskFolder::RegisterTask` (XML) or `RegisterTaskDefinition` — [Security Contexts](https://learn.microsoft.com/en-us/windows/win32/taskschd/security-contexts-for-running-tasks).
- On-demand run: `IRegisteredTask::Run`/`RunEx` — same source.

### Inferences
- Draft Task XML (**untested**: no task was created. Assembled from the schema and the Bing example; `RandomDelay`, `LogonTrigger/UserId` and EventTrigger `Delay` are from memory of the schema):

```xml
<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Daily wallpaper refresh</Description></RegistrationInfo>
  <Principals>
    <Principal id="Author">
      <UserId>S-1-5-21-...</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <WakeToRun>false</WakeToRun>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <ExecutionTimeLimit>PT5M</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Triggers>
    <CalendarTrigger>
      <StartBoundary>2026-11-05T09:00:00</StartBoundary>
      <RandomDelay>PT30M</RandomDelay>
      <ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay>
    </CalendarTrigger>
    <LogonTrigger>
      <UserId>S-1-5-21-...</UserId>
      <Delay>PT1M</Delay>
    </LogonTrigger>
    <!-- Optional, add after testing as a standard user -->
    <EventTrigger>
      <Subscription>&lt;QueryList&gt;&lt;Query Id="0" Path="Microsoft-Windows-NetworkProfile/Operational"&gt;&lt;Select Path="Microsoft-Windows-NetworkProfile/Operational"&gt;*[System[(EventID=10000)]]&lt;/Select&gt;&lt;/Query&gt;&lt;/QueryList&gt;</Subscription>
      <Delay>PT30S</Delay>
    </EventTrigger>
  </Triggers>
  <Actions Context="Author">
    <Exec>
      <Command>%LOCALAPPDATA%\Programs\backreel\backreel.exe</Command>
      <Arguments>--run</Arguments>
    </Exec>
  </Actions>
</Task>
```

- Use a real absolute path in `Command` if environment expansion proves unreliable. Do not put the exe under `%TEMP%` as Bing's task does: disk cleanup could silently break the schedule.
- Name the task `backreel_<SID>` in the root folder, as Bing does, so that users on a shared PC never collide.
- Make every run idempotent. The exe reads state and exits within milliseconds if today's wallpaper is already set. That is what makes several triggers safe.
- Minimal v1 triggers are Calendar + Logon. They cover the normal cases, including a laptop asleep at the scheduled time. Network and unlock triggers are additions for later.
- The 10-minute StartWhenAvailable delay is a reason to keep the Logon trigger. Bing's use of it on a plain daily trigger suggests "repeat infinitely" includes daily schedules with no end boundary.
- Installer fallback order for creating the task: COM `RegisterTask` (XML), then `schtasks /create /xml`, then an HKCU Run entry with a `--run` flag if creation is blocked. That Run entry still exits immediately, and gives logon-only refresh.
- A hand-written Win32 tray app would likely use only a few MB, which is low but not zero. The user's goal is "absolutely minimal", and the daily job does not need residency.

### Gaps
- No measurement of a minimal native Win32 tray process. Not built, since it would add a tray icon and exceed the read-only limit. The few-MB figure above is an estimate.
- Non-admin creation of LogonTrigger and EventTrigger tasks is unresolved: reports conflict and no official statement was found. It needs a test on a true standard account. The current session is non-elevated, but whether the account itself is a standard user was not established.
- Whether a standard user can create a task subfolder (for example `\backreel\`) was not verified. Bing uses root; Google's `\GoogleUserPEH\` folder exists here, but how it was created is unknown.
- Whether the "ghost tray icon" appears after a process exits without `NIM_DELETE` was not sourced from docs. Always delete explicitly if a tray icon is ever used.
- Marginal cost in the Schedule service of holding an extra event subscription is unmeasured, though it is presumably tiny.

## Does a daily-run scheduled exe avoid all resident cost, and how does the settings UI work?

### Takeaway
Yes. Between runs no backreel process exists. Use one exe: no flag opens settings on demand, `--run` does the headless job. Keep the UI code out of the headless path by delay-loading.

### Cited Findings
- Processes owning a window get QoS from their visibility and focus; other threads get a heuristic based on factors such as priority — [Quality of Service](https://learn.microsoft.com/en-us/windows/win32/procthread/quality-of-service).
- Task Scheduler's `MultipleInstancesPolicy` governs instances of the task only; it defaults to IgnoreNew — [settingsType](https://learn.microsoft.com/en-us/windows/win32/taskschd/taskschedulerschema-settingstype-complextype).
- `IRegisteredTask::Run` lets a user run tasks they created — [Security Contexts](https://learn.microsoft.com/en-us/windows/win32/taskschd/security-contexts-for-running-tasks).
- Per-Monitor V2 awareness is set through the application manifest and needs Windows 10 1703 — [High DPI](https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows).

### Inferences
- Single exe beats two exes. Code pages are loaded on demand, so unused UI code in the image costs little, and one binary halves install and update surface. A separate settings exe is only worthwhile if the UI needs a heavy toolkit.
- Link as `/SUBSYSTEM:WINDOWS` so that a scheduled run does not flash a console. Delay-load comctl32 and any other UI-only DLLs.
- Command surface: `backreel.exe` (settings), `--run` (scheduled), `--next` (manual change, ignores the once-a-day gate), `--install` / `--uninstall` (register or remove the task, clear the cache).
- Settings has a "Next wallpaper" button. It should run the work in-process or through `IRegisteredTask::Run`. Guard against a concurrent scheduled run with a named mutex, since IgnoreNew does not cover a manually launched exe.
- Where settings live: a Start menu shortcut replaces the tray icon. Add a Settings-window status line ("Last run: ..., Next: ...") from state and `IRegisteredTask::get_NextRunTime`.
- User-visible failures: record to a small log and show it the next time settings opens. If an alert is truly needed, a run can briefly add an icon, show a banner and `NIM_DELETE` before exiting. Prefer silence.

### Gaps
- Toast notifications from an unpackaged exe without a Start-menu AUMID were not researched in depth. The identity doc lists toast as an identity benefit; that does not prove unpackaged toast is impossible.

## Network: WinHTTP vs WinINet vs bundled TLS, metered detection, retries, conditional GET

### Takeaway
Use WinHTTP in synchronous mode with `WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY`. This departs from Microsoft's stated default of WinINet for client apps, deliberately: backreel needs no credential prompts, cookies, FTP or IE zones. Do not bundle TLS. Check cost with `INetworkCostManager` before downloading, and use ETag or Last-Modified for conditional requests.

### Cited Findings
- Microsoft: "you should use WinINet unless you plan to run within a service or service-like process that requires impersonation and session isolation". WinINet adds credential cache and prompts, FTP, IE zones and cookie jar; WinHTTP adds service support, session isolation and impersonation. Both support gzip/deflate decompression — [WinINet vs. WinHTTP](https://learn.microsoft.com/en-us/windows/win32/wininet/wininet-vs-winhttp).
- WinHTTP supports desktop client applications as well as services — [Windows HTTP Services](https://learn.microsoft.com/en-us/windows/win32/winhttp/winhttp-start-page).
- `WINHTTP_ACCESS_TYPE_DEFAULT_PROXY` is deprecated on 8.1+ and does not inherit browser settings. `WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY` uses system and per-user settings and handles failover — [WinHttpOpen](https://learn.microsoft.com/en-us/windows/win32/api/winhttp/nf-winhttp-winhttpopen).
- `WINHTTP_FLAG_SECURE_DEFAULTS` requires TLS 1.2+, disables fallback, **and implicitly sets async**. Microsoft strongly recommends async mode generally — same source.
- `WINHTTP_OPTION_ENABLE_HTTP_PROTOCOL` with `WINHTTP_PROTOCOL_FLAG_HTTP2` enables HTTP/2 per request; `WINHTTP_OPTION_DECOMPRESSION` enables gzip/deflate — [WinHTTP option flags](https://learn.microsoft.com/en-us/windows/win32/winhttp/option-flags).
- TLS 1.3 is supported from Windows 11 and Server 2022; enabling it on earlier Windows is "not a safe system configuration". Windows 10 22H2 tops out at TLS 1.2. TLS 1.0/1.1 are disabled by default starting with 2024 Insider builds — [Schannel protocols](https://learn.microsoft.com/en-us/windows/win32/secauthn/protocols-in-tls-ssl--schannel-ssp-).
- File sizes here: winhttp.dll 1,253,704; wininet.dll 2,750,048; urlmon.dll 1,932,648; schannel.dll 811,008 bytes — [Local check]. File size is not RAM use.
- `INetworkCostManager` (`GetCost`, `GetDataPlanStatus`, `SetDestinationAddresses`), Windows 8+, desktop, **no server support** — [INetworkCostManager](https://learn.microsoft.com/en-us/windows/win32/api/netlistmgr/nn-netlistmgr-inetworkcostmanager).
- Cost flags: UNKNOWN 0, UNRESTRICTED 0x1, FIXED 0x2, VARIABLE 0x4, OVERDATALIMIT 0x10000, CONGESTED 0x20000, ROAMING 0x40000, APPROACHINGDATALIMIT 0x80000; multiple bits may be set — [NLM_CONNECTION_COST](https://learn.microsoft.com/en-us/windows/win32/api/netlistmgr/ne-netlistmgr-nlm_connection_cost).
- WinRT `ConnectionCost` adds `BackgroundDataUsageRestricted`. Microsoft guidance: for Fixed gate large optional downloads, for Variable minimise, for OverDataLimit pause background sync, for Roaming suppress large automatic updates. Evaluate flags individually and re-check before sizable transfers — [ConnectionCost](https://learn.microsoft.com/en-us/uwp/api/windows.networking.connectivity.connectioncost).
- 304 Not Modified, If-None-Match and Retry-After are defined in [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110) (cited from memory, not re-fetched this session).

### Inferences
- Why WinHTTP over WinINet: smaller DLL, nothing of WinINet's feature surplus is needed, and it avoids sharing state with the IE-era per-user cache. The difference is real but modest. WinINet would also work, so this is a preference, not a correctness issue.
- Use sync mode for a one-shot downloader. Async callbacks add complexity without benefit for one or two requests. So skip `SECURE_DEFAULTS`, and set `WINHTTP_OPTION_SECURE_PROTOCOLS` to TLS 1.2 | 1.3 manually.
- A bundled TLS stack would add binary size and a duty to ship security fixes, with no gain, since Schannel is patched through Windows Update.
- Metered decision table:

| Condition | Action |
|---|---|
| UNRESTRICTED or UNKNOWN | Proceed |
| FIXED or VARIABLE | Skip by default, rotate from cache; user opt-in setting |
| ROAMING, OVERDATALIMIT, APPROACHINGDATALIMIT | Always skip unless `--next` |
| CONGESTED | Proceed |

- Offline: on connection failure, pick an unused cached image and mark today "pending". Then exit. Never loop or sleep waiting for a network, because that creates a resident process.
- Retry: 2-3 attempts with jittered backoff inside one run, capped at about a minute, honouring `Retry-After`. After that, exit and let the next trigger retry.
- Do not depend on `RunOnlyIfNetworkAvailable`, given the anecdotal trigger problem. Check in the app.
- Conditional GET: store `ETag` and `Last-Modified` per source URL in state, send `If-None-Match` / `If-Modified-Since`. A 304 means rotate from cache. Metadata JSON benefits most; image URLs usually change daily anyway.
- Download to `*.tmp` and rename on success. Cap maximum response size, verify `Content-Type`, and let WIC decode failure reject bad files.

### Gaps
- Actual RAM difference between WinHTTP and WinINet at runtime was not measured.
- The docs' comparison table does not state WinHTTP's caching behaviour. That WinHTTP has no persistent URL cache is from memory; the design assumes the app does its own conditional requests.
- Whether `INetworkCostManager` needs a COM apartment or message pump for a one-shot `GetCost` was not confirmed.
- `GetCost` behaviour with VPNs was not researched.

## Image work: WIC, monitor detection, fit strategy and codec availability

### Takeaway
WIC can do all decode, crop, scale and encode work with no bundled library. Enumerate decoders with the WIC component enumerator rather than the registry. Store extension codecs (WebP, HEIF, AVIF, JPEG XL) may be absent, so treat JPEG and PNG as the guaranteed baseline. Target physical pixels with a Per-Monitor V2 manifest.

### Cited Findings
- `HighQualityCubic` (value 4, Windows 10+) resizes its kernel to the scale factor, "suitable for downscaling by factors greater than 2". Fant averages all mapped pixels — [WICBitmapInterpolationMode](https://learn.microsoft.com/en-us/windows/win32/api/wincodec/ne-wincodec-wicbitmapinterpolationmode).
- `IWICBitmapSourceTransform` lets a codec scale natively during decoding. Inbox scaler, flip-rotator and format converter use it automatically **when placed immediately after a frame decode** — [IWICBitmapSourceTransform](https://learn.microsoft.com/en-us/windows/win32/api/wincodec/nn-wincodec-iwicbitmapsourcetransform).
- JPEG encoder options: `ImageQuality` (VT_R4, 0-1.0), `JpegYCrCbSubsampling` (VT_UI1), `BitmapTransform` — [Encoding overview](https://learn.microsoft.com/en-us/windows/win32/wic/-wic-creating-encoder).
- `WICDecodeMetadataCacheOnDemand` retains the metadata stream — [JPEG re-encode how-to](https://learn.microsoft.com/en-us/windows/win32/wic/-wic-codec-jpegmetadataencoding).
- WIC codecs are built in or are "extension codecs downloadable from the Microsoft Store" — [WIC codecs from Microsoft](https://learn.microsoft.com/en-us/windows/win32/wic/native-wic-codecs).
- HEIF is a Store extension. HEVC and AV1 "might not be available on all PCs"; check with `MFTEnumEx`. HEIF is a container: HEVC gives `.heic`, AV1 gives `.avif` — [HEIF codec](https://learn.microsoft.com/en-us/windows/win32/wic/heif-codec). That page's checks concern encoders; decoder behaviour without the codec should be tested.
- The Store JPEG XL extension works only on 24H2 or later — [PCWorld](https://www.pcworld.com/article/2625216/microsoft-adds-jpeg-xl-support-to-windows-11-with-a-quick-download.html).
- **Decoders found by WIC COM enumeration here:** BMP, GIF, ICO, CUR, JPEG, PNG, TIFF, DNG, WMPhoto (.wdp/.jxr), DDS, Microsoft HEIF (.heic/.heif/.avif and others), Microsoft WebP, Microsoft Raw, Microsoft JPEG XL — [Local check].
- **The registry view undercounts:** the decoder category `Instance` key under HKLM listed only Raw and JPEG XL, whereas the COM enumerator returned 14 decoders — [Local check].
- Installed packages here: WebpImageExtension 1.2.31, HEIFImageExtension 1.2.62, HEVCVideoExtension 2.5.48, AV1VideoExtension 2.0.35, RawImageExtension 2.5.42 — [Local check].
- DPI-unaware processes get virtualised values as if the screen were 96 DPI. PMv2 sees raw pixels, and the docs admit virtualisation behaviour is not fully documented — [High DPI](https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows).
- Observed: the monitor is 2560x1440 at 144 DPI (150%). WinForms reported 1707x960 from PowerShell, while `IDesktopWallpaper::GetMonitorRECT` and `EnumDisplaySettings` both reported 2560x1440 — [Local check]. Monitor ID format: `\\?\DISPLAY#GBT2709#5&2acf295c&0&UID4355#{e6f07b5f-...}`.
- The Bing app uses `_UHD` 3840x2160 JPEGs on this 1440p display, so the image is larger than the monitor and scaling happens at display time — [Local check].

### Inferences
- Pipeline: `CreateDecoderFromFilename(CacheOnDemand)` → `GetFrame(0)` → scaler → clipper → `IWICFormatConverter` (to 24bppBGR) → JPEG encoder with `ImageQuality` ~0.92-0.95 and 4:4:4 subsampling.
- Put the **scaler first**, directly after the frame decode. That keeps JPEG native scaled decoding, and clipping after scaling is simple arithmetic: scale so the image covers the target, then clip the excess.
- Memory: an uncompressed 3840x2160 BGRA frame is about 33 MB and 7680x4320 about 133 MB. A pulled WIC chain avoids holding full frames where it can. Reject images with absurd dimensions before decoding, using `GetSize`.
- Encode output once per monitor at exact size. For a monitor with the same resolution as an already-rendered image, reuse the file.
- Only re-encode when needed. If the source is JPEG and already exactly the monitor size, pass it through, avoiding a generation of loss before Windows' own re-encode.
- Monitor list: `IDesktopWallpaper` (`GetMonitorDevicePathCount/At` + `GetMonitorRECT`, only S_OK monitors), which directly gives the IDs needed by `SetWallpaper`. Also use a PMv2 manifest so that every other API agrees. Physical pixels matter, not DPI, so `GetDpiForMonitor` is not needed for rendering.
- Fit math for a 16:9 source under cover-crop: 21:9 keeps about 76% of height; 32:9 keeps 50%; portrait 9:16 keeps about 32% of width. So:
  - Compute `crop_loss = 1 - min(srcAR/dstAR, dstAR/srcAR)` per monitor.
  - Accept if loss is below ~30%; otherwise look for another candidate or a native-aspect or portrait variant from the source.
  - Portrait and super-ultrawide will often fail this, so prefer sources offering multiple aspect ratios. If nothing qualifies, take a centre crop anyway rather than failing.
  - Avoid upscaling beyond about 1.25-1.5x. Prefer a different image if the source is smaller.
  - Default to centre crop. Saliency detection isn't worth the code.
- Per-monitor images beat `DWPOS_SPAN`: span needs a canvas across the virtual screen and is awkward with mixed DPI, gaps or offset monitors. Offer span only if users ask.
- Codec policy: request JPEG from sources, with `Accept: image/jpeg, image/png`. Treat other formats as best-effort: try decoding and skip on failure. Enumerate decoders with `CreateComponentEnumerator(WICDecoder)` only if the UI needs to show them. Never bundle a codec.
- Handle EXIF orientation with a flip-rotator if the source supplies camera photos. Curated wallpaper sources rarely need it.

### Gaps
- Not verified whether WIC auto-applies EXIF orientation or converts ICC profiles to sRGB. Wide-gamut sources could look off; needs a test.
- Not confirmed which extensions are preinstalled on a clean Windows 11 Home or Pro install vs fetched from the Store over time. This machine is not a clean install.
- Exact failure HRESULT for a missing codec was not sourced. `WINCODEC_ERR_COMPONENTNOTFOUND` is expected from memory. `.avif` decode with AV1 extension absent was not tested.
- WIC behaviour with very large or malformed images was not researched, and codec security advisories were not reviewed. Downloaded files are untrusted input to the decoder.
- `EnumDisplayMonitors`/`GetDpiForMonitor` docs were not fetched individually; the design doesn't need them for rendering.
- `GetMonitorRECT` returning physical pixels from a non-aware process is one observation. Do not rely on it; the manifest is the fix.

## Power and impact: EcoQoS, priority, and machine wake

### Takeaway
Leave Task Priority at the default 7, which already gives below-normal CPU, low I/O and low memory priority. Add one `SetProcessInformation` call for EcoQoS at startup. Leave WakeToRun off. A short-lived run on a machine that sleeps needs catch-up triggers rather than wake timers.

### Cited Findings
- Task Priority table: **7 = BELOW_NORMAL_PRIORITY_CLASS, IoPriorityLow, MEMORY_PRIORITY_LOW**; 8 same with MEMORY_PRIORITY_VERY_LOW; 9-10 = IDLE class with IoPriorityVeryLow. Levels 7-8 are for background tasks, 4-6 interactive. The default is 7 — [Priority element](https://learn.microsoft.com/en-us/windows/win32/taskschd/taskschedulerschema-priority-settingstype-element).
- Editing a task in the Task Scheduler GUI can reset Priority to 7 — [Windows OS Hub, Anecdote](https://woshub.com/set-task-priority-windows-task-scheduler/). This is harmless for backreel, which uses 7.
- EcoQoS: `SetProcessInformation(GetCurrentProcess(), ProcessPowerThrottling, ...)` with Version = `PROCESS_POWER_THROTTLING_CURRENT_VERSION`, ControlMask and StateMask both `PROCESS_POWER_THROTTLING_EXECUTION_SPEED`. Prior to Windows 11 the level is LowQoS — [SetProcessInformation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setprocessinformation).
- Eco QoS "always selects most efficient CPU frequency and schedules to efficient cores", whereas Low/Utility do so on battery — [Quality of Service](https://learn.microsoft.com/en-us/windows/win32/procthread/quality-of-service).
- `PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION` makes the system ignore the process's timer resolution requests — [SetProcessInformation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setprocessinformation).
- `ProcessMemoryPriority` with `MEMORY_PRIORITY_LOW` is recommended for background operations — same source.
- Task Manager's Efficiency mode lowers base priority and sets EcoQoS — [Microsoft Performance Diagnostics blog](https://devblogs.microsoft.com/performance-diagnostics/?p=319).
- `PROCESS_MODE_BACKGROUND_BEGIN` lowers resource scheduling priorities for background work, is current-process only, and must not be used by processes interacting with the user; sharing resources across threads risks priority inversion. CPU priority alone is insufficient for I/O-heavy background work — [SetPriorityClass](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setpriorityclass).
- WakeToRun defaults to false; when it wakes the machine the screen may stay off — [WakeToRun](https://learn.microsoft.com/en-us/windows/win32/taskschd/tasksettings-waketorun), [settingsType](https://learn.microsoft.com/en-us/windows/win32/taskschd/taskschedulerschema-settingstype-complextype).
- Modern Standby: the always-on timer can wake the SoC without turning on the display, and from 24H2, if excessive drain is detected, most wake sources are disabled — [Modern Standby wake sources](https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/modern-standby-wake-sources).
- A user reports Modern Standby doesn't support waking with a wake timer — [Microsoft Q&A, Anecdote](https://learn.microsoft.com/en-us/answers/questions/1660115/regarding-waking-up-from-sleep-on-a-modern-standby).
- This machine: Standby (S0 Low Power Idle) Network Disconnected, Hibernate, Fast Startup; S1/S2 unavailable — [Local check].

### Inferences
- Recommended startup for `--run`: set EcoQoS, then proceed. Skip `PROCESS_MODE_BACKGROUND_BEGIN` and a manual `ProcessMemoryPriority` under the scheduler, since Priority 7 covers those. Apply `PROCESS_MODE_BACKGROUND_BEGIN` only if `--run` is launched from a Run key, where the scheduler's priorities don't apply. Do not use it for `--next`, because the user is waiting.
- EcoQoS slows work slightly on what is a seconds-long job, and that is acceptable. Do not use it in settings UI.
- Do not set `WakeToRun`. Waking a laptop to change a wallpaper nobody is looking at wastes battery. On this class of machine, a sleeping PC simply runs the catch-up logic on next logon or by StartWhenAvailable.
- With disconnected standby, the network is likely unavailable right after resume, hence the `Delay` on triggers and the graceful offline path.

### Gaps
- No measured power figure for EcoQoS on a job this size; the Windows Developer Blog URL tried for EcoQoS data returned 404.
- Not confirmed whether Task Scheduler runs `StartWhenAvailable` tasks during Modern Standby, or only on screen-on. No official statement was found.
- Whether I/O and memory priority from the task table are actually applied to Exec actions was not verified with a running task. The page says the process starts with a Priority Class value, and the table lists all columns.
- Windows Battery Saver interaction was not researched. Consider skipping downloads in Battery Saver as an app-level policy.

## Storage: cache, state, eviction, config

### Takeaway
Keep everything under `%LOCALAPPDATA%\backreel`. Evict at the end of each run, never deleting any image Windows may still reference. Use a small text config file.

### Cited Findings
- Windows retains the source file path: `Control Panel\Desktop\WallPaper` points at the original file, and per-desktop `Wallpaper` values point at older files — [Local check].
- The Bing app keeps its images at `%LOCALAPPDATA%\Packages\Microsoft.BingWallpaper_8wekyb3d8bbwe\LocalState\images\bing\` and uses `_UHD_bing.jpg` date-prefixed names — [Local check].

### Inferences
- Suggested layout:

```
%LOCALAPPDATA%\backreel\
  config.ini     user settings (sources, time, metered policy, fit rules)
  state.json     last run, per-source ETag, history, pending flag, monitor signature
  log.txt        small rolling log
  cache\original\   downloaded images
  cache\rendered\   per-monitor output, e.g. 20261104_3f2a_2560x1440.jpg
```

- Get the root with `SHGetKnownFolderPath(FOLDERID_LocalAppData)`. Per-user, writable without admin, and it does not roam.
- Eviction: at end of a run, keep files referenced by current state and `GetWallpaper` for each monitor, then keep the newest N rendered sets (around 14) under a byte cap (around 200 MB), with originals evicted first. Keep a few past images as the offline rotation pool.
- Deleting a file still referenced by a stale virtual desktop may leave that desktop on a cached or blank image. Since the keys can be numerous (292 here), do not try to scan them. Keeping the last 14 days of renders is a sufficient margin. Test what happens when a referenced file is removed.
- Format: `config.ini` is human-editable and needs no parser dependency. Keep state as simple key=value too if the language lacks a built-in JSON parser.
- Write atomically: write `*.tmp` then rename, so that a killed run cannot corrupt state. The 5-minute task limit can kill a run.
- Uninstall: delete the task, then the folder. Leave the current wallpaper alone, but Windows may lose its source file, so copy the current image to `%USERPROFILE%\Pictures` if users want to keep it.

### Gaps
- No official source was fetched for `FOLDERID_LocalAppData` semantics; the non-roaming statement is from memory.
- What Windows shows if a referenced source file is deleted (whether it continues using `TranscodedWallpaper`) was not tested, as this would change local state.
- No data on a typical cache budget. The numbers above are design guesses.

## Edge cases: hot-plug, RDP, user switching, policy, activation, Home vs Pro

### Takeaway
With no resident process there is no live reaction to display changes, so the design recomputes at the next trigger. Skip runs in remote sessions. Detect policy locks before doing any network work. Activation and edition have little effect on the APIs as far as evidence shows, though no direct API test on an unactivated install was found.

### Cited Findings

**Hot-plug**
- `GetMonitorRECT` distinguishes detached (S_FALSE) from unknown (E_INVALIDARG) — [GetMonitorRECT](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-idesktopwallpaper-getmonitorrect).
- Docking and undocking are listed as common DPI-change scenarios — [High DPI](https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows).

**RDP**
- Slideshows can be temporarily disabled by a remote session (`DSS_DISABLED_BY_REMOTE_SESSION`) — [GetStatus](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-idesktopwallpaper-getstatus).
- Detect with `GetSystemMetrics(SM_REMOTESESSION)`, but that misreports RemoteFX vGPU sessions as local; also compare `GlassSessionId` under `HKLM\System\CurrentControlSet\Control\Terminal Server` with the current session ID — [Detecting RDS](https://learn.microsoft.com/en-us/windows/win32/termserv/detecting-the-terminal-services-environment).
- `DisallowStartOnRemoteAppSession` prevents starts in RAIL sessions — [settingsType](https://learn.microsoft.com/en-us/windows/win32/taskschd/taskschedulerschema-settingstype-complextype).

**Fast user switching**
- Fast user switching is implemented using Terminal Services, so sessions are separate — [Interactive Services](https://learn.microsoft.com/en-us/windows/win32/services/interactive-services).
- Users can only manage their own tasks unless they are admins — [Security Contexts](https://learn.microsoft.com/en-us/windows/win32/taskschd/security-contexts-for-running-tasks).

**Group policy**
- "Prevent changing desktop background" is `NoChangingWallPaper` under `Software\Microsoft\Windows\CurrentVersion\Policies\ActiveDesktop` (user scope). Microsoft notes the "Desktop Wallpaper" setting must also be enabled to prevent changes — [ADMX_ControlPanelDisplay](https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-admx-controlpaneldisplay).
- `Wallpaper` and `WallpaperStyle` under `HKCU\...\Policies\System` take precedence over user settings, and the system ignores the user value while policy is active — Microsoft TechNet registry reference surfaced by search, for example [TechNet](https://technet.microsoft.com/en-us/library/cc959089.aspx) (older material; exact page not isolated).
- A forum user reported `NoChangingWallPaper = 1` did not work on its own — [Seven Forums, Anecdote](https://www.sevenforums.com/threads/desktop-background-specify-and-prevent-change.62048/page-2). That is consistent with Microsoft's note.
- Here: `Policies\ActiveDesktop` is empty, `Policies\System` holds no wallpaper values, and `HKLM\...\Policies\Microsoft\Windows\Personalization` has nothing — [Local check].

**Activation**
- Unactivated Windows greys out Personalization settings — [TechSpot](https://www.techspot.com/guides/1696-change-wallpaper-windows-not-activated/), [MakeUseOf](https://www.makeuseof.com/windows-11-change-wallpaper-without-activation/).
- Right-click "Set as background" and other workarounds still change the wallpaper — [Winaero, Anecdote](https://winaero.com/change-windows-10-desktop-wallpaper-without-activation/?amp), [winhelponline, Anecdote](https://www.winhelponline.com/blog/set-desktop-background-wallpaper-without-activation/). One source claims that an unactivated system can silently block changes — [geekchamp, Anecdote](https://geekchamp.com/?p=252995).
- This machine is licensed (`LicenseStatus` = 1), so it could not be tested — [Local check].

**Home vs Pro**
- Personalization CSP does not list Home — [Personalization CSP](https://learn.microsoft.com/en-us/windows/client-management/mdm/personalization-csp). ADMX policies list Pro, Enterprise, Education and IoT Enterprise, not Home — [ADMX_ControlPanelDisplay](https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-admx-controlpaneldisplay).
- Task Scheduler is fully available on Home: Schedule service running, 208 tasks, `InteractiveToken` tasks present — [Local check].
- The registry `ProductName` reads "Windows 10 Home" on this Windows 11 machine, with `DisplayVersion` 25H2 and `CurrentBuild` 26200 — [Local check].

**Windows 10 (secondary)**
- All wallpaper, Task Scheduler and WIC APIs discussed exist on Windows 10 (PMv2 from 1703, HighQualityCubic from Windows 10). Differences: no TLS 1.3, LowQoS instead of EcoQoS, no JPEG XL extension — sources above.
- Consumer ESU: originally ended 13 October 2026, reported as extended to 12 October 2027 in June 2026 — [Thurrott](https://www.thurrott.com/?p=337939), [TechRepublic](https://www.techrepublic.com/article/news-windows-10-esu-cheat-sheet/). Older pages still give 2026, and Microsoft's own page was not read.

### Inferences
- Hot-plug: compute a monitor signature (sorted IDs + rects). A logon or unlock trigger can compare it with state, and if it differs, re-render from cached originals with no network. Add `SessionStateChangeTrigger` `SessionUnlock` as an opt-in, since each unlock spawns a short process.
- Rotation and resolution changes show up as a changed rect, and are handled by the same check.
- Pre-flight order for `--run`:
  1. Gate on the once-a-day state.
  2. Bail if the session is remote (both checks).
  3. Bail if `NoChangingWallPaper` or a `Wallpaper` policy is set, in HKCU or HKLM, and log it.
  4. Check `BackgroundType` against the user's first-run choice.
  5. Check network cost.
- Running in a remote session could size wallpaper for the wrong display set, hence skipping and leaving the day pending.
- Fast user switching: per-user tasks, per-user data. Each user installs separately. Name tasks with the SID.
- Detect the OS by `RtlGetVersion` or build number (>= 22000 = Windows 11), never by `ProductName`.
- Policy-locked or failed set: don't fight it. Log, and show a status line in settings.
- Competing apps: Bing Wallpaper is installed, resident and on a daily task on this machine, so it will overwrite backreel's wallpaper. Testing here needs it paused. A first-run check that compares `GetWallpaper` with backreel's last path could warn that another app is managing the wallpaper.
- The 24H2 hold list was never published. Sticking to public APIs and avoiding Progman/WorkerW window manipulation is the sensible way to stay clear of that kind of issue, though the trigger is unknown.
- Test on 26H1 (build 28000) if Arm support matters, since it has a different core.

### Gaps
- No API-level evidence on `IDesktopWallpaper` or SPI on unactivated Windows; only UI workarounds and a conflicting anecdote.
- Whether SPI or `IDesktopWallpaper` returns an error or silently succeeds under `NoChangingWallPaper`/`Wallpaper` policy was not found.
- Whether a task with InteractiveToken runs in a disconnected (switched-away) session and whether `SetWallpaper` works there is undocumented in sources found. Needs a test.
- What a newly attached monitor shows before the next run was not researched.
- No system event suitable for a display-change trigger was identified.
- Whether gpedit-less Home honours registry-set policy values was not verified with a source.
- Behaviour when Explorer has crashed or a replacement shell is used was not found. The expected failure is `CoCreateInstance` failing.

## Optional local test: read-only results on this machine

### Takeaway
All checks were read-only. No task, wallpaper or system setting was changed. The key results are that WIC here has WebP, HEIF/AVIF and JPEG XL decoders, Windows re-encodes the JPEG but keeps its 4K size, and Microsoft's Bing Wallpaper task is a usable template.

### Cited Findings
All items below are **[Local check]**, 2026-11-04.
- OS: EditionID Core (Home), DisplayVersion 25H2, build 26200, UBR 9711. `ProductName` says Windows 10 Home.
- Session non-elevated. PowerShell thread was STA.
- Monitor: one, 2560x1440, AppliedDPI 144 (150%).
- Wallpaper: Bing JPEG, `WallpaperStyle` 10, `TileWallpaper` 0, `GetPosition` = 4 (FILL), `GetStatus` = 1 (DSS_ENABLED, no slideshow), `BackgroundType` 0, Spotlight `EnabledState` 0.
- Policies: none affecting wallpaper. `Policies\System` holds only `DisableRegistryTools`, `DisableCMD`, `DisableTaskMgr`, all 0. `CloudContent` policy absent.
- `SlideshowDirectoryPath1` is populated although no slideshow is active, so its presence does not imply a running slideshow.
- Tasks with StartWhenAvailable under this user include Bing Wallpaper, `dusty automatic clean` and `\GoogleUserPEH\` tasks, including one named `RunPlatformExperienceHelperOnUnlock`.
- Sleep: S0 Low Power Idle (Network Disconnected), Hibernate, Fast Startup.
- Method note: COM queries used a small C# helper loaded with `Add-Type` from the session scratchpad. Interface slots for setters were declared to preserve vtable order and never called.

### Inferences
- This machine is a good test bed for Modern Standby and DPI behaviour, but it is unrepresentative for multi-monitor, Pro and clean-install codec availability.

### Gaps
- Tests that were deliberately not run because they change state: `SetWallpaper` on virtual desktops, slideshow or Spotlight interaction, PNG vs JPEG transcoding, unpackaged lock-screen call, standard-user task registration, and real run cost measurement. These are the priority items for a prototype on a spare profile or VM.
