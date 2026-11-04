# Stack and distribution for backreel (minimal Windows 11 wallpaper utility)

State of tools as of November 2026. Every number is labelled **[Measured]** (run on this machine for this research), **[Published]** (someone else's measurement) or **[Claim]** (vendor or unverified statement).

## Which stack gives the smallest binary and lowest resource use? (size, cold start, working set)

### Takeaway
On a like-for-like test app, native stacks tie at about 23 ms start, about 2-3.7 MB private memory and about 14-17 MB working set. C and `no_std` Rust reach 8 KB, and Rust with `std` and a static CRT is 201 KB. Because a scheduled run-and-exit design costs zero at idle in any language, the real differences are download size, dependencies and peak memory for about 0.3 s a day. On peak memory, Go, .NET and especially webview stacks cost visibly more.

### Cited Findings

**Test method [Measured].** One equivalent test app per stack, built only with compilers and offline caches already on this machine.
- Machine: i9-13980HX, 32 GB, Windows 11 Home 10.0.26200, WebView2 156.0.4314.58.
- Toolchains: rustc 1.95.0 (MSVC target), MSVC 14.44, gcc 15.2 (MSYS2), Go 1.25.4, .NET SDK 10.0.204, .NET Framework 4.8 csc, Tauri 2 (offline cache), Electron 38.8.4 (cached zip).
- `noop`: start and exit at once (n=30 after 2 discarded warm-ups).
- `daily`: HTTPS GET of the Bing image JSON, parse the URL, download a 327,250-byte JPEG, write it to disk, read its dimensions (n=9, every run exit 0). WIC is used in the C and Rust apps, System.Drawing in C#, `image/jpeg` in Go.
- `settings`: a window with a combo box, three checkboxes and a button. Working set and private bytes sampled 3.5 s after input-idle (n=3).
- Harness: `bench.c`, using `GetProcessMemoryInfo` and QPC.

**Limits of these measurements.**
- Timings are warm start only. A true cold start needs a file-cache purge, which was not done, so **no cold-start numbers exist here**.
- `IDesktopWallpaper::SetWallpaper` is linked in but never ran (no change to the wallpaper was allowed), so its cost is not included.
- The prototypes have no config persistence, scheduling logic or error UI, so sizes are a lower bound for a real product.
- Working-set figures include shared system DLL pages. Private bytes is the fairer cost figure.

| Stack [Measured] | File size | noop median | daily peak WS / peak private | settings WS / private | Runtime needed |
|---|---|---|---|---|---|
| C, MSVC, no CRT | 8,192 B | 23.3 ms | 16.7 / 3.7 MB | 14.5 / 2.0 MB | none |
| Rust, `no_std`, windows-sys | 8,192 B | 23.3 ms | 16.7 / 3.7 MB | 14.4 / 2.0 MB | none |
| C, MSVC `/MD` | 16,384 B | 25.4 ms | 16.8 / 3.7 MB | 14.6 / 2.0 MB | VCRUNTIME140 + UCRT (imports confirmed) |
| C, mingw-w64 | 24,576 B | 24.4 ms | 16.8 / 3.8 MB | 14.5 / 2.1 MB | msvcrt.dll |
| C, MSVC `/MT` | 107,520 B | 24.5 ms | 16.9 / 3.8 MB | 14.5 / 2.1 MB | none |
| Rust `std`, dynamic CRT | 101,888 B | 23.4 ms | 16.9 / 3.7 MB | 14.6 / 2.1 MB | VCRUNTIME140 + UCRT (imports confirmed) |
| Rust `std`, `+crt-static` | 201,216 B | 23.9 ms | 16.9 / 3.7 MB | 14.6 / 2.1 MB | none |
| Rust `std`, `+crt-static`, serde_json | 224,768 B | 25.5 ms | 17.0 / 3.8 MB | 14.6 / 2.1 MB | none |
| Rust winsafe 0.0.19 (window + 1 button only) | 221,696 B | not run | not run | 13.4 / 2.0 MB | none |
| Go 1.25 (`-s -w`, net/http, encoding/json) | 6,110,208 B | 27.3 ms | 19.6 / 18.9 MB | 17.4 / 14.6 MB | none |
| C# on .NET Framework 4.8 (WinForms) | 6,656 B | 42.1 ms | 32.4 / 21.1 MB | 31.2 / 22.4 MB | in Windows |
| C# .NET 10 framework-dependent (WinForms) | 162,304 B apphost + 7,680 B dll | 66.4 ms | 46.3 / 12.0 MB | 46.5 / 10.4 MB | 76.0 MB + 94.3 MB shared runtimes on disk here |
| Tauri 2 hello (WebView2) | 2,530,816 B | not run | not run | tree of 7 processes: 344 MB WS / 155 MB private | WebView2 |
| Electron 38.8.4 hello | 324.1 MB unpacked, 130.1 MB zip | not run | not run | tree of 4 processes: 300 MB WS / 145 MB private | bundled |

- Daily wall-clock medians were 243-289 ms for native, 276 ms for Go, 318 ms for .NET Framework and 385 ms for .NET 10. The run is network-bound, and network outliers reached 2-3 s in most stacks. [Measured]
- Time to input-idle for the native settings window was 48-66 ms, Go 64 ms, net48 59 ms and .NET 10 92 ms. This metric is not comparable for webview apps, because their host is idle before the page renders. [Measured]
- Tauri and Electron process-tree figures are a single sample. Summed working sets double-count shared pages. [Measured]
- Tauri and Electron had near-equal memory here, which matches a report on [Tauri issue #5889](https://github.com/tauri-apps/tauri/issues/5889) that WebView2 memory was similar to Electron. The often-quoted large gap comes from a [macOS benchmark](https://dev.to/gethopp/tauri-vs-electron-performance-bundle-size-and-the-real-trade-offs-1el4).
- The Tauri test binary imported VCRUNTIME140, since it was built without `crt-static`, and its lockfile resolved 431 packages. [Measured]

**Published numbers for stacks not measured here.**
- Hello-world sizes on Windows, Aug 2024: Zig 9 kB, C (MSVC) 134 kB, Rust 234 kB, C# 945 kB, Go 2,174 kB. [Published: sizegame round 4](https://raw.githubusercontent.com/MichalStrehovsky/sizegame/master/README.md). These are console programs at default settings. A [different repo](https://github.com/jawadcode/helloworld_size) reports Zig at 5 kB on Windows.
- .NET Native AOT console: 3.65 MB on .NET 7 falling to 1.05 MB on .NET 10 [Published: blog](https://hn.jakechampion.name/reader/https://code.soundaranbu.com/state-of-nativeaot-net10). Another project gives [2,998,272 bytes](https://awesome.ecosyste.ms/projects/github.com%2Fascpixi%2Fsmolsharp) for a default hello world, version unstated.
- Native AOT with WinForms/WPF: [heise reports](https://www.heise.de/en/news/NET-10-0-is-Ready-11075047.html?seite=3) that it still does not work on .NET 10. Microsoft's docs list [no built-in COM on Windows](https://learn.microsoft.com/pt-pt/dotnet/core/deploying/native-aot) as a limitation, which matters because IDesktopWallpaper and WIC are COM.
- An [experimental 2021 WinForms AOT build](https://codevision.medium.com/status-of-winforms-in-nativeaot-4267644b1e60) averaged 37 MB.
- A [community WPF fork](https://github.com/lezzi/wpfaot) reports about 136 ms start versus 650-730 ms JIT on a .NET 10 preview. This is one unofficial project.
- WinUI 3 (App SDK 2.4.0, Native AOT, unpackaged): 131.7 MB raw, 48.9 MB after trimming (17.0 MB zip), 0.7 s cold and 0.3 s warm to first window. [Published: one developer's notes](https://raw.githubusercontent.com/Phoenix-/ForceAutoHDR.Net/main/notes/winui3-aot-payload-trim.md). That author warns trimming can break real apps. A [GitHub issue](https://github.com/microsoft/WindowsAppSDK/issues/5438) reports a 244 MB bin folder.
- Flutter: release `flutter_windows.dll` was [about 6.7 MB in SDK 3.7.12](https://gist.github.com/loic-sharma/e35a59f7c8d5605dd94cb27e5bc59ef2). This is the engine artifact, not an app total.
- Delphi: an old Stack Overflow answer gives [about 2.3 MB for an XE3 release build](https://kiwix.lema.org/stackoverflow.com_en_all_2023-11/questions/13598678/exe-size-is-too-big-in-delphi-xe2).
- Lazarus: [2008-2010 reports](https://forum.lazarus.freepascal.org/index.php/topic,9066.0.html) give about 11.5-12 MB, apparently with debug info. Current figures were not found.
- AutoHotkey: an [unofficial guide](https://cdn.jsdelivr.net/npm/windows-layout@1.0.7/COMPILE.md) gives about 2-3 MB uncompressed. [Claim]

### Inferences
- With a scheduled run-and-exit design, no process exists between runs, so idle cost is zero for every stack. The language only affects the brief daily run and the download.
- `no_std` buys about 190 KB of file size and nothing measurable at runtime: its memory and start time match `std`. The extra `unsafe` and hand-written runtime stubs are hard to justify unless the 8 KB figure is itself a goal.
- The roughly 7 MB `noop` working set and 14-17 MB during work are mostly Windows DLLs (user32, COM, WinHTTP, WIC), not language runtime. Go's and .NET's private memory is the true runtime premium: about 13-15 MB for Go and 8-20 MB for .NET.
- .NET Framework 4.8's 6.6 KB exe is real but misleading. It costs about 6-11x the private memory of native, though for a 0.3 s daily run that is arguably harmless.
- WinForms, WPF, WinUI 3, Flutter, Electron and Tauri are poor fits on size. Native AOT is unattractive here because the app's two key APIs are COM.

### Gaps
- No cold-start data for any stack, and I found no credible published Windows cold-start comparison.
- Not measured because the tools are absent: Zig, Native AOT (ILCompiler not in the offline cache), bflat (no numbers found at all), Delphi/FPC, AutoHotkey v2, WinUI 3, Flutter, native-windows-gui.
- No published working-set figures found for WinUI 3 hello, Flutter Windows, AutoHotkey v2, or a stated-method WebView2 measurement.

## What settings UI costs nothing at idle, and how close can a Win32 dialog get to Windows 11?

### Takeaway
A Win32 dialog in the same exe adds almost nothing: the complete test app is 8 KB to 201 KB, and it uses about 2 MB private only while open. Official support stops at dark title bars and Mica backdrops. Dark controls still need undocumented APIs, a library or custom drawing. An on-demand WebView2 window also costs zero at idle, but costs about 150 MB private while open.

### Cited Findings
- Native dialog, common controls v6 via manifest, in-memory template: included in the 8,192-byte C and `no_std` Rust binaries; 14.4 MB WS, 2.0 MB private while open. [Measured]
- WebView2 window via Tauri: 2.53 MB exe, about 155 MB private and 344 MB WS across 7 processes while open, and nothing when closed. [Measured, single sample]
- Plain config file: no UI code and no footprint. Inference only, nothing to measure.
- Dark title bars are officially supported through [`DWMWA_USE_IMMERSIVE_DARK_MODE`](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-windows-themes). That page says it does not cover how to repaint your app UI in dark colours.
- Mica for a Win32 window is official: [`DWMSBT_MAINWINDOW`](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwm_systembackdrop_type) corresponds to Mica on Windows 11, minimum build 22621.
- Dark common controls rely on undocumented uxtheme functions loaded by ordinal, such as [`SetPreferredAppMode` at 135](https://gist.github.com/rounk-ctrl/b04e5622e30e0d62956870d5c22b7017), and some controls need manual handling. These internals could break in future Windows builds.
- [darkmodelib](https://github.com/gmh5225/darkmodelib) is a C++ library covering common controls.
- In March 2026 a Microsoft executive said work was under way on broader dark mode, with [no firm commitments](https://www.windowslatest.com/2026/03/30/microsoft-confirms-windows-11-dark-mode-upgrade-with-plans-for-third-party-apps-and-registry-editor). A [June 2026 forum post](https://techcommunity.microsoft.com/discussions/windows11/windows-11s-dark-mode-isnt-fully-implemented-yet-the-interface-in-older-versions/4525552/replies/4525883) reports that legacy dialogs were still white.
- WebView2 Evergreen is described as [part of Windows 11](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution), though Microsoft still advises checking for it. Fixed Version is over 250 MB. `WebView2Loader` must ship with the app or be statically linked.
- The Evergreen runtime folder on this machine is 870.8 MB. [Measured]

### Inferences
- A true "Windows Settings look" means WinUI, at tens of MB. I found no lightweight official route.
- Simplest good option: a native dialog with an official dark title bar, optionally Mica, with owner-drawn or custom-coloured controls only if dark mode matters. Avoiding the ordinal APIs avoids a known break risk.
- A dialog that paints an opaque background probably will not show Mica behind its client area without custom painting. I did not verify this.

### Gaps
- The test dialog called the dark-mode and Mica attributes, but its visual result was not inspected.
- WebView2 hosted directly from C or Rust without Tauri was not measured, because the loader is not on this machine.
- No official Microsoft dark-theme API for common controls was found as of this research.

## Rust specifics: what does a realistic binary look like?

### Takeaway
Using only `windows-sys`, WinHTTP and WIC, this feature set measured 201 KB with `std` and a static CRT, and 225 KB with serde_json. Swapping in reqwest with rustls gives 1.76 MB even without UI or image decoding. Expect a shipping build to be somewhat larger than 225 KB, since the prototype omits config and scheduling.

### Cited Findings
- Profile used: `opt-level="z"`, `lto=true`, `codegen-units=1`, `panic="abort"`, `strip=true`, matching the [min-sized-rust](https://raw.githubusercontent.com/johnthagen/min-sized-rust/main/README.md) guidance. The individual contribution of each flag was not isolated.
- Static CRT adds 99,328 bytes (101,888 to 201,216). [Measured]
- serde_json with `Value` adds 23,552 bytes over a hand-written URL scan. [Measured]
- `no_std`/`no_main` on the MSVC target reached 8,192 bytes but needed hand-written `memcpy`, `memset`, `memcmp`, `_fltused` and a `__CxxFrameHandler3` stub. [Measured]
- A hand parser cost me a real bug in this test: an off-by-one on the key length silently produced an empty URL. Only exit-code checking caught it. [Measured]
- reqwest (blocking, rustls-tls) + serde_json: 1,764,352 bytes and 132 lockfile packages including ring. Its 9.8 MB WS, 2.6 MB private and 212 ms daily run are not like-for-like, because it skips WIC, COM and user32. [Measured]
- `windows-sys` has no wrappers for COM interfaces here, so WIC and IDesktopWallpaper were called through hand-written vtable indices. [Measured]
- Default MSVC Rust output imports VCRUNTIME140.dll and UCRT forwarders. With `+crt-static` those imports disappear. [Measured]
- min-sized-rust quotes about 8 KB for `no_std` and 51 KB for `build-std`, stripped, [on macOS](https://raw.githubusercontent.com/johnthagen/min-sized-rust/main/README.md), and warns UPX may trigger antivirus heuristics.

### Inferences
- Prefer `std` + `+crt-static` + `windows-sys` + WinHTTP. WinHTTP also delegates TLS to Windows rather than shipping a TLS stack.
- WIC may be unnecessary for the core path if the image is handed straight to the wallpaper API as a file. It is only needed for validation or resizing.
- Use serde_json rather than a hand scanner. It costs 23.5 KB, and multiple image sources mean real JSON handling, which is exactly where the hand parser failed.
- Avoid UPX.

### Gaps
- tinyjson, miniserde, native-windows-gui and the `image` crate were not measured: they or their dependencies are not in the offline cache, and nothing was downloaded. No size is claimed for them.
- `build-std` was not measured, because the only nightly here is GNU-target.
- The cost of a Task Scheduler registration code path was not measured.

## Distribution: Store, WinGet, Scoop, GitHub, zip, and effects of packaging

### Takeaway
Store MSIX is the only route that removes signing cost and SmartScreen warnings, and registration is free. Its catch is scheduling: the documented run-at-logon route is `windows.startupTask`, and Task Scheduler with packaged apps has reported problems. I found no requirement for package identity for wallpaper work. A portable exe on GitHub, WinGet and Scoop gives the simplest scheduling and smallest footprint.

### Cited Findings
- Registration: [no fees for individual or company accounts](https://learn.microsoft.com/en-us/windows/apps/publish/partner-center/open-a-developer-account) through storedeveloper.microsoft.com (page dated 29 Oct 2026). Older coverage citing a [$99 company fee](https://www.techradar.com/pro/microsoft-just-dropped-its-store-fees-for-windows-developers) is superseded by this page. Other entry paths show a legacy flow.
- Store MSIX: [free signing, re-signed by Microsoft, no SmartScreen warnings, worldwide](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options).
- Store MSI/EXE: [the installer and all PE files must be signed](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msi/app-package-requirements) with a Trusted Root Program chain, hosted on your own versioned HTTPS URL, silent, and not a downloader stub. So this path does not avoid buying a certificate.
- Store updates for EXE/MSI: sources conflict. [Build 2025 coverage](https://www.neowin.net/news/microsoft-store-finally-gets-last-updated-for-apps-direct-win32-updates-and-more/) describes Store-delivered updates, while [Microsoft's guidance](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msi/publish-update-to-your-app-on-store) says the Store does not provide them to existing users. No 2026 confirmation found.
- MSIX constraints: [AppData and HKCU writes are redirected to private per-app stores, the install directory is not writable, and starting utilities such as cmd or PowerShell should be avoided](https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-prepare). That page notes this could block Store submission, though its wording is Windows 10 S era.
- Startup: per [Advanced Installer](https://www.advancedinstaller.com/startup-programs-msix.html), Startup folder and Run keys do not work for MSIX, and `desktop:StartupTask` is the route. [Community reports](https://techcommunity.microsoft.com/discussions/msix-discussions/msix-will-not-launch-run-on-user-login-using-windows-startuptask-in-manifest-on-/1473940) describe it failing on some devices.
- Task Scheduler: a [forum report](https://techcommunity.microsoft.com/t5/msix/msix-app-execution-alias-from-cannot-be-selected-from-task/m-p/3805369) says an MSIX execution alias failed from Task Scheduler only, with a `cmd /C` workaround and a disputed explanation. Not official.
- Lock screen: the [`LockScreen.SetImageFileAsync` reference](https://learn.microsoft.com/en-us/UWP/api/windows.system.userprofile.lockscreen.setimagefileasync?view=winrt-19041) does not address unpackaged apps. Microsoft's [list of WinRT APIs unsupported in desktop apps](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/desktop-to-uwp-supported-api) did not match `LockScreen` or `UserProfile` in a text search; the only hit was the unrelated `LockApplicationHost`.
- The registry route needs [admin rights](https://gist.github.com/derekantrican/1b998d90d53fda607a4d6810dc4d2479), and sources disagree on whether it works on Pro ([works](https://www.systemcenterdudes.com/?p=211759) vs [blank on Pro](https://learn.microsoft.com/en-us/answers/questions/1390256/manage-custom-lockscreen-in-windows-professional-s)).
- WinGet: [`portable` and `zip` are valid installer types](https://www.mintlify.com/microsoft/winget-cli/concepts/manifests). [Policy](https://github.com/microsoft/winget-pkgs/blob/master/doc/Policies.md) requires install without user interaction, and a package flagged by any scan cannot be accepted regardless of legitimacy. That page does not address signing. New versions need [manual review](https://newreleases.io/project/github/lin-ycv/EverythingPowerToys/release/v0.85.1), so there is a delay.
- How others ship: TranslucentTB [relies on MSIX identity](https://github.com/TranslucentTB/TranslucentTB/issues/1001) and ships MSIX plus `.appinstaller`. EarTrumpet is [Store-distributed](https://github.com/walkingthicc/EarTrumpet) per a README fork. [Everything](https://www.wingetly.io/publishers/voidtools) and [ShareX](https://scout.appaloosa.io/en/apps/windows/winget:ShareX.ShareX) are on WinGet per third-party indexes.

### Inferences
- Package identity does not appear to be needed for setting the desktop wallpaper, a Task Scheduler entry or a local config file in an unpackaged build. It is only required if you take the MSIX route for its other benefits.
- Under MSIX, a daily timer is the weak point. A logon check ("has today's image been fetched?") is the straightforward design, but it will not refresh a machine that stays logged in for days.
- Two builds are feasible from one codebase: an MSIX for Store users and a portable exe for WinGet, Scoop and GitHub.

### Gaps
- No official Microsoft documentation found on registering scheduled tasks from packaged apps, or on background triggers for a full-trust MSIX app.
- Whether lock screen APIs work unpackaged is unconfirmed, and it could not be tested without changing a system setting.
- MSIX package overhead for a tiny exe was not measured.
- Scoop Extras criteria were not obtained (the page returned 404), and PowerToys' shipping method was not researched.

## Code signing, SmartScreen, Smart App Control and antivirus risk

### Takeaway
No paid option gives instant SmartScreen trust any more: EV lost that in 2024. Only Store MSIX avoids warnings outright. Artifact Signing, at about $9.99 a month, is the cheapest route for eligible developers, but individuals must be in the USA or Canada. Smart App Control can block unsigned code outright, so an unsigned portable exe will fail on those PCs unless it has good cloud reputation.

### Cited Findings
- Azure Trusted Signing is now [Artifact Signing](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart). Individuals [must be in the USA or Canada](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart), and identity details come from an Individual-type billing account.
- Organization countries conflict. The [quickstart, updated Oct 2026](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart), lists USA, Canada, EU, UK, Australia, New Zealand, Japan, South Korea, Singapore, Switzerland, Norway and Israel. The [code signing options page, Aug 2026](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options), lists only USA, Canada, EU and UK. The quickstart is the newer page.
- Validation can take [1 to 20 business days](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart) for organizations.
- Price: [starts at $9.99/month](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation) per Microsoft. [Azure's pricing page](https://azure.microsoft.com/en-us/pricing/details/artifact-signing/) shows 5,000 signatures on Basic and 100,000 on Premium, but rendered no prices to my fetch. Premium at $99.99 and $0.005 overage are [third-party figures](https://www.gdgsoft.com/faq/azure-trusted-signing-cost-effective-exe-code-signing).
- A paid Azure subscription appears necessary: [GDG Software](https://www.gdgsoft.com/faq/azure-trusted-signing-cost-effective-exe-code-signing) recommends Pay-As-You-Go, and a [reader comment on Melatonin](https://melatonin.dev/blog/code-signing-on-windows-with-azure-trusted-signing/) says you must upgrade to it first. Microsoft pages I read do not state this.
- Certificates last [three days](https://textslashplain.com/2025/03/12/authenticode-in-2025-azure-trusted-signing/), so timestamping is essential.
- No timeline for other countries per a [Q&A thread](https://learn.microsoft.com/en-us/answers/questions/2263040/trusted-signing-identity-validation-not-available).
- SignPath Foundation is [free for OSS](https://signpath.org/) with [conditions](https://signpath.org/terms): OSI licence, no proprietary components, a "Code signing policy" section on the project site, MFA, automated verifiable builds, and release approval.
- With SignPath, **the publisher shown is "SignPath Foundation", not the project**. Reputation is checked with no stated thresholds. Certificate type conflicts: [OV](https://github.com/mentaldesk/TuiCode/pull/70) vs [EV](https://prism-studio.readthedocs.io/en/v1.12.0/GITHUB_SIGNING.html), with Microsoft saying ["OV-level"](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options).
- OV: [$150-300/year](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options) per Microsoft, or [about $129 to over $530](https://my-ssl.com/learn/code-signing-certificate-price) on CA stores. EV: [$400+](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options), or about $349-669, with resellers from [about $279](https://signmycode.com/ev-code-signing). Keys must sit on [an HSM or hardware token](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options).
- Certificate validity was cut to [460 days from 1 March 2026](https://devclass.com/2026/01/14/code-signing-windows-apps-may-be-easier-and-more-secure-with-new-azure-artifact-service/). [Some sellers](https://certum.store/certum-ev-code-sigining.html) cite 459 days and 27 February, and I did not check the CA/B ballot text.
- SmartScreen: [EV no longer bypasses it](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation). There is no fixed threshold, but it "can take several weeks and hundreds of clean installs". Signed new files still warn but show the publisher, and reputation can carry across versions with a consistent identity. Unsigned files restart from zero each version. There is no manual submission route for consumer endpoints.
- Smart App Control: [blocks unsigned code unless the cloud service recognises it as trustworthy](https://textslashplain.com/2026/04/28/smart-app-control/), and it checks all loaded code including DLLs. Signatures must chain to Microsoft's Trusted Root Program, and RSA is the safest choice.
- SAC [applies to all executable files](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation), not just downloads. Since April 2026 it [can be turned back on](https://textslashplain.com/2026/04/28/smart-app-control/) in 25H2.
- Forum reports say [signed binaries can still be blocked](https://learn.microsoft.com/en-us/answers/questions/5791210/application-signed-and-blocked-by-smart-app-contro) when reputation is unknown, in an answer I could not confirm as official. Users [cannot allow one app](https://learn.microsoft.com/en-us/answers/questions/5823237/smart-app-control), per a forum answer.
- Antivirus: reports exist of Defender flagging [a compiled Rust hello world](https://github.com/rust-lang/rust/issues/88297) with a machine-learning Wacatac label, and [a GNU-target build but not MSVC](https://medium.com/opsops/is-windows-broken-7f8de8b8f3ad), which is one anecdote. [AutoHotkey's own FAQ](https://autohotkey.com/download/safe.htm) says false positives are common on new releases, and forum users link [packers to more detections](https://www.autohotkey.com/boards/viewtopic.php?p=379105).
- On this machine, Smart App Control reads as off (`VerifiedAndReputablePolicyState=0`), so unsigned test binaries ran without testing SAC. [Measured]

### Inferences
- Which route is cheapest depends on where the developer is based, which this research did not establish.
- A one-file design helps signing: SAC checks every module, so a single signed exe has nothing else to fail.
- Publishing an unsigned exe as the main download would be fragile: SmartScreen resets every release, and SAC can block it outright.
- A false positive would also block the WinGet route, since any flagged scan rejects the package.

### Gaps
- I found no data showing that tiny size, `no_std`, or downloader-plus-wallpaper behaviour raises detection rates. All antivirus evidence is anecdotal.
- No Microsoft statement found on whether Store-installed apps are exempt from SAC beyond a [forum answer](https://learn.microsoft.com/en-us/answers/questions/5791210/application-signed-and-blocked-by-smart-app-contro) saying so.
- No scan of the test binaries was run. Actual price of Artifact Signing could not be read from the pricing page itself.
- A search summary said billing starts at account creation even if validation fails, and that free or trial subscriptions are unsupported. I could not trace either claim to a page I read, so verify both before signing up.

## How can auto-update keep the footprint at zero?

### Takeaway
No resident updater is needed. Store MSIX and package managers handle updates outside the app, and a portable build can do one version check inside the daily run it already performs.

### Cited Findings
- [Store MSIX](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msi/app-package-requirements) is described as giving easier updates, plus free signing and CDN hosting.
- [`winget upgrade`](https://learn.microsoft.com/lb-lu/windows/package-manager/winget/upgrade) finds and installs available updates, but only runs when a user or a task invokes it. Winget manages apps it [installed or knows about](https://techygeekshome.info/update-windows-apps-with-winget/).
- [Winget-AutoUpdate](https://sourceforge.net/projects/winget-autoupdate.mirror/) is a third-party wrapper that schedules this daily.
- For Store EXE/MSI, update delivery is unclear (see the conflict noted under Distribution), so the [in-app route](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msi/publish-update-to-your-app-on-store) remains relevant.

### Inferences
- The simplest zero-footprint design: during the daily run, make one extra HTTPS request to a release feed, and only tell the user a new version exists. Let WinGet, Scoop or the Store do the replacing.
- If the app ever replaces itself, it should verify the new file's signature first and handle Windows' lock on a running exe. Self-update also adds code and is the behaviour most likely to look suspicious to scanners.
- A new unsigned build resets SmartScreen reputation each release, so frequent updates carry a real cost without signing.

### Gaps
- The cost of the extra request was not measured.
- Scoop's update mechanism was not sourced.
- I found no evidence on how antivirus products treat self-updating tiny executables.

## Overall recommendation

### Takeaway
Build a single-file Rust `std` exe with static CRT, `windows-sys`, WinHTTP, serde_json and a native dialog, run by Task Scheduler and exiting after each run. Expect a few hundred KB, with no resident process. Publish it to the Microsoft Store as MSIX if the scheduling constraint is acceptable, and also as a signed portable exe through GitHub Releases and WinGet.

### Cited Findings
- Measured prototype: 224,768 bytes with no runtime dependency, about 3.8 MB private for about 0.3 s a day, and about 2.1 MB private while the settings window is open. [Measured, table above]
- C with no CRT is 8 KB with the same runtime figures, so C is the choice only if absolute size outranks memory safety and convenience. [Measured]
- [Store MSIX](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options) is free and warning-free worldwide. Non-Store signing needs [Artifact Signing](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart) if eligible, [SignPath](https://signpath.org/terms) if fully open source and happy with its publisher name, otherwise an [OV certificate](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options).

### Inferences
- Priority order for the design: first no resident process, second no third-party TLS or image stack, third `std` over `no_std`.
- Skip EV, UPX, Electron, WinUI 3 and Native AOT for this app.
- If dark settings UI matters more than exe size, a config file plus a minimal dialog is lower risk than undocumented dark-mode ordinals.

### Gaps
- The recommendation has not been tested as a complete product: wallpaper application, scheduling and signing were all outside these measurements.
- The best signing route depends on the developer's country, whether the project will be open source, and whether a daily timer is required under MSIX.
- Local measurement artefacts live in the session scratchpad (`results.csv`, `results_daily2.csv`) and are temporary. The earlier `results_daily.csv` and the daily rows in `results.csv` are invalid, because they came from runs with a missing output folder and a parser bug. The numbers above use only `results_daily2.csv` for daily runs.
