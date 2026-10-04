//! Windows integration.
//!
//! * Discovery enumerates the shell's AppsFolder, the same list Start shows
//!   under "All apps". It covers classic (Win32) and Store (MSIX) apps alike.
//! * Icons come from `IShellItemImageFactory`, the shell's own renderer.
//! * Launching goes through `ShellExecuteExW`, which reports real errors
//!   (missing target, access denied, cancelled UAC prompt).

use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use roshan_core::{AppTarget, LaunchError, Launched};
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::Globalization::{GetUserDefaultUILanguage, LCIDToLocaleName};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits,
    GetObjectW, HBITMAP, HGDIOBJ, ReleaseDC,
};
use windows::Win32::Storage::EnhancedStorage::PKEY_Link_TargetParsingPath;
use windows::Win32::System::Com::{
    CLSIDFromString, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx,
    CoTaskMemFree, CoUninitialize,
};
use windows::Win32::System::Threading::GetProcessId;
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, IShellItem2,
    IShellItemImageFactory, KF_FLAG_DEFAULT, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC,
    SEE_MASK_NOCLOSEPROCESS, SHCreateItemFromParsingName, SHELLEXECUTEINFOW, SHGetKnownFolderItem,
    SHGetKnownFolderPath, SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING, SIIGBF_ICONONLY,
    ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{GUID, HSTRING, Interface, PCWSTR, PWSTR};

use crate::icon_cache::encode_png;
use crate::{DiscoveredApp, ICON_SIZE, Icon, OpenTarget};

const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Initializes COM for the current thread for as long as it is alive.
struct Com(bool);

impl Com {
    fn init() -> Self {
        // S_FALSE (already initialized) also needs a matching uninitialize;
        // RPC_E_CHANGED_MODE means another model is active, which works too.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        Com(hr.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

/// Copies a COM-allocated string and frees it.
unsafe fn take_pwstr(p: PWSTR) -> String {
    let s = unsafe { p.to_string() }.unwrap_or_default();
    unsafe { CoTaskMemFree(Some(p.0 as *const c_void)) };
    s
}

// ---------------------------------------------------------------- discovery

pub fn discover_apps() -> Vec<DiscoveredApp> {
    let _com = Com::init();
    unsafe { enumerate_apps_folder() }.unwrap_or_default()
}

unsafe fn enumerate_apps_folder() -> windows::core::Result<Vec<DiscoveredApp>> {
    let folder: IShellItem =
        unsafe { SHGetKnownFolderItem(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None) }?;
    let items: IEnumShellItems = unsafe { folder.BindToHandler(None, &BHID_EnumItems) }?;
    let mut apps = Vec::new();
    loop {
        let mut batch: [Option<IShellItem>; 32] = Default::default();
        let mut fetched = 0u32;
        let hr = unsafe { items.Next(&mut batch, Some(&mut fetched)) };
        for item in batch.iter().take(fetched as usize).flatten() {
            if let Some(app) = unsafe { to_app(item) } {
                apps.push(app);
            }
        }
        if hr.is_err() || fetched < batch.len() as u32 {
            break;
        }
    }
    Ok(apps)
}

unsafe fn to_app(item: &IShellItem) -> Option<DiscoveredApp> {
    let name = unsafe { take_pwstr(item.GetDisplayName(SIGDN_NORMALDISPLAY).ok()?) };
    let id = unsafe { take_pwstr(item.GetDisplayName(SIGDN_PARENTRELATIVEPARSING).ok()?) };
    // For shortcuts the shell resolves the file the entry launches.
    let target = item
        .cast::<IShellItem2>()
        .ok()
        .and_then(|item| unsafe { item.GetString(&PKEY_Link_TargetParsingPath) }.ok())
        .map(|p| unsafe { take_pwstr(p) })
        .filter(|t| !t.is_empty());
    if name.trim().is_empty() || !is_launchable_app(&name, &id, target.as_deref()) {
        return None;
    }
    let detail = target.or_else(|| resolve_known_folder_path(&id));
    Some(DiscoveredApp {
        name,
        target: AppTarget::WindowsApp { id },
        detail,
    })
}

/// Programs that Start-menu shortcuts use to show a document rather than
/// being the application themselves.
const DOCUMENT_VIEWERS: &[&str] = &["notepad", "write", "wordpad", "hh", "winhlp32"];
const DOCUMENT_EXTENSIONS: &[&str] = &[
    "txt", "pdf", "htm", "html", "chm", "hlp", "rtf", "md", "url", "doc", "docx",
];

fn lower_ext_and_stem(path: &str) -> (String, String) {
    let path = Path::new(path);
    let lower = |s: Option<&std::ffi::OsStr>| {
        s.map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    };
    (lower(path.extension()), lower(path.file_stem()))
}

/// AppsFolder also lists uninstallers, readme files and web links that
/// installers drop into the Start menu. Only keep things that are apps.
fn is_launchable_app(name: &str, id: &str, target: Option<&str>) -> bool {
    let lower_name = name.to_lowercase();
    if lower_name.contains("uninstall") || name.contains("حذف نصب") {
        return false;
    }
    let (name_ext, _) = lower_ext_and_stem(&lower_name);
    if DOCUMENT_EXTENSIONS.contains(&name_ext.as_str()) {
        return false;
    }
    if id.contains("://") || target.is_some_and(|t| t.contains("://")) {
        return false;
    }
    // Prefer the resolved target; a path-like id ("{KnownFolder}\Vendor\app.exe")
    // names its target file too.
    let path = target.or_else(|| (id.contains('\\') || id.contains('/')).then_some(id));
    if let Some(path) = path {
        let (ext, stem) = lower_ext_and_stem(path);
        return ext == "exe"
            && !stem.starts_with("unins")
            && !stem.starts_with("uninst")
            && !DOCUMENT_VIEWERS.contains(&stem.as_str());
    }
    // Otherwise it is the AppUserModelID of a packaged app.
    true
}

/// Turns "{GUID}\rest" into a real path for display, if the GUID is a known folder.
fn resolve_known_folder_path(id: &str) -> Option<String> {
    let rest = id.strip_prefix('{')?;
    let (guid, tail) = rest.split_once("}\\")?;
    let guid: GUID = unsafe { CLSIDFromString(&HSTRING::from(format!("{{{guid}}}"))) }.ok()?;
    let base = unsafe { SHGetKnownFolderPath(&guid, KF_FLAG_DEFAULT, None) }.ok()?;
    let base = unsafe { take_pwstr(base) };
    Some(PathBuf::from(base).join(tail).display().to_string())
}

fn shell_parsing_name(target: &AppTarget) -> Option<String> {
    match target {
        AppTarget::WindowsApp { id } => Some(format!("shell:AppsFolder\\{id}")),
        AppTarget::Executable { path } => Some(path.display().to_string()),
        _ => None,
    }
}

pub fn check_app(target: &AppTarget) -> Result<(), LaunchError> {
    match target {
        AppTarget::Executable { path } if path.exists() => Ok(()),
        AppTarget::Executable { path } => Err(LaunchError::NotFound(path.display().to_string())),
        AppTarget::WindowsApp { id } => {
            let _com = Com::init();
            let parsing = HSTRING::from(format!("shell:AppsFolder\\{id}"));
            unsafe { SHCreateItemFromParsingName::<_, _, IShellItem>(&parsing, None) }
                .map(|_| ())
                .map_err(|_| LaunchError::NotFound(id.clone()))
        }
        _ => Err(LaunchError::Unsupported),
    }
}

pub fn target_for_path(path: &Path) -> AppTarget {
    AppTarget::Executable {
        path: path.to_owned(),
    }
}

// ------------------------------------------------------------------- icons

pub fn app_icon(target: &AppTarget) -> Option<Icon> {
    let parsing = shell_parsing_name(target)?;
    let _com = Com::init();
    unsafe {
        let factory: IShellItemImageFactory =
            SHCreateItemFromParsingName(&HSTRING::from(parsing), None).ok()?;
        let size = windows::Win32::Foundation::SIZE {
            cx: ICON_SIZE as i32,
            cy: ICON_SIZE as i32,
        };
        let bitmap = factory.GetImage(size, SIIGBF_ICONONLY).ok()?;
        let pixels = bitmap_pixels(bitmap);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let (width, height, rgba) = pixels?;
        encode_png(width, height, rgba)
    }
}

/// Reads a 32-bit shell bitmap as straight RGBA.
unsafe fn bitmap_pixels(bitmap: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    let mut bm = BITMAP::default();
    let read = unsafe {
        GetObjectW(
            HGDIOBJ(bitmap.0),
            size_of::<BITMAP>() as i32,
            Some((&raw mut bm).cast()),
        )
    };
    if read == 0 || bm.bmWidth <= 0 || bm.bmHeight <= 0 {
        return None;
    }
    let (width, height) = (bm.bmWidth, bm.bmHeight);
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let hdc = unsafe { GetDC(None) };
    let lines = unsafe {
        GetDIBits(
            hdc,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        )
    };
    unsafe { ReleaseDC(None, hdc) };
    if lines == 0 {
        return None;
    }
    bgra_to_rgba(&mut pixels);
    Some((width as u32, height as u32, pixels))
}

/// Converts the shell's BGRA (usually premultiplied) into straight RGBA.
fn bgra_to_rgba(pixels: &mut [u8]) {
    let (px, _) = pixels.as_chunks::<4>();
    let has_alpha = px.iter().any(|p| p[3] != 0);
    // Premultiplied data never has a color channel above its alpha.
    let premultiplied = has_alpha
        && px
            .iter()
            .all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]);
    for p in pixels.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
        if !has_alpha {
            p[3] = 255;
        } else if premultiplied && p[3] > 0 && p[3] < 255 {
            let a = u32::from(p[3]);
            for c in &mut p[..3] {
                *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

// ---------------------------------------------------------------- launching

pub fn launch_app(target: &AppTarget) -> Result<Launched, LaunchError> {
    match target {
        AppTarget::WindowsApp { id } => shell_execute(&format!("shell:AppsFolder\\{id}"), None),
        AppTarget::Executable { path } => shell_execute(&path.display().to_string(), path.parent()),
        _ => Err(LaunchError::Unsupported),
    }
}

pub fn open(target: &OpenTarget) -> Result<Launched, LaunchError> {
    match target {
        OpenTarget::Url(url) => shell_execute(url, None),
        OpenTarget::Path(path) => shell_execute(&path.display().to_string(), None),
    }
}

fn shell_execute(file: &str, dir: Option<&Path>) -> Result<Launched, LaunchError> {
    let _com = Com::init();
    let file_w = HSTRING::from(file);
    let dir_w = dir.map(|d| HSTRING::from(d.as_os_str()));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC,
        hwnd: HWND::default(),
        lpFile: PCWSTR(file_w.as_ptr()),
        lpDirectory: dir_w
            .as_ref()
            .map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }.map_err(|e| map_error(&e, file))?;
    let pid = if info.hProcess.is_invalid() {
        None
    } else {
        let pid = unsafe { GetProcessId(info.hProcess) };
        let _ = unsafe { CloseHandle(info.hProcess) };
        (pid != 0).then_some(pid)
    };
    Ok(Launched { pid })
}

fn map_error(e: &windows::core::Error, what: &str) -> LaunchError {
    let hr = e.code().0 as u32;
    // HRESULT_FROM_WIN32 puts Win32 codes into facility 7.
    let win32 = if hr & 0xFFFF_0000 == 0x8007_0000 {
        hr & 0xFFFF
    } else {
        hr
    };
    match win32 {
        2 | 3 => LaunchError::NotFound(what.to_owned()),
        5 => LaunchError::AccessDenied,
        1223 => LaunchError::ElevationCancelled,
        1155 => LaunchError::NoHandler,
        _ => LaunchError::Os {
            code: i64::from(win32),
            message: e.message().trim().to_owned(),
        },
    }
}

pub fn run_command(
    line: &str,
    cwd: Option<&Path>,
    terminal: bool,
    _terminal_override: Option<&str>,
) -> Result<Launched, LaunchError> {
    let shell = std::env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into());
    let mut cmd = Command::new(shell);
    // `/S` makes cmd strip exactly the outer quotes and keep the line verbatim.
    if terminal {
        cmd.raw_arg(format!("/S /K \"{line}\""))
            .creation_flags(CREATE_NEW_CONSOLE);
    } else {
        cmd.raw_arg(format!("/S /C \"{line}\""))
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    }
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let child = cmd.spawn().map_err(|e| io_error(&e, "cmd.exe"))?;
    Ok(Launched {
        pid: Some(child.id()),
    })
}

fn io_error(e: &std::io::Error, what: &str) -> LaunchError {
    match e.kind() {
        std::io::ErrorKind::NotFound => LaunchError::NotFound(what.to_owned()),
        std::io::ErrorKind::PermissionDenied => LaunchError::AccessDenied,
        _ => LaunchError::Os {
            code: i64::from(e.raw_os_error().unwrap_or(0)),
            message: e.to_string(),
        },
    }
}

// --------------------------------------------------------------------- misc

const RUN_KEY: PCWSTR = windows::core::w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const APPROVED_KEY: PCWSTR =
    windows::core::w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run");
const RUN_VALUE: PCWSTR = windows::core::w!("Roshan");

/// Reads a registry value under HKCU as raw bytes.
fn read_hkcu(key: PCWSTR) -> Option<Vec<u8>> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_ANY, RegGetValueW};
    let mut len = 0u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            RUN_VALUE,
            RRF_RT_ANY,
            None,
            None,
            Some(&mut len),
        )
    };
    if status.is_err() || len == 0 {
        return None;
    }
    let mut data = vec![0u8; len as usize];
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            RUN_VALUE,
            RRF_RT_ANY,
            None,
            Some(data.as_mut_ptr().cast()),
            Some(&mut len),
        )
    };
    status.is_ok().then(|| {
        data.truncate(len as usize);
        data
    })
}

pub fn start_at_login() -> bool {
    if read_hkcu(RUN_KEY).is_none() {
        return false;
    }
    // Task Manager's "Disable" leaves the Run entry and marks it here with
    // an odd first byte.
    read_hkcu(APPROVED_KEY).is_none_or(|flags| flags.first().is_none_or(|b| b & 1 == 0))
}

pub fn set_start_at_login(enabled: bool, exe: &Path) -> Result<(), String> {
    use windows::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW,
    };
    unsafe {
        // Clear any "disabled in Task Manager" mark either way.
        let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, APPROVED_KEY, RUN_VALUE);
        if !enabled {
            let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE);
            return Ok(());
        }
        let command = format!("\"{}\" {}", exe.display(), crate::STARTUP_FLAG);
        let wide: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            RUN_VALUE,
            REG_SZ.0,
            Some(wide.as_ptr().cast()),
            (wide.len() * 2) as u32,
        )
        .ok()
        .map_err(|e| e.message())
    }
}

pub fn claim_single_instance() -> bool {
    use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow,
    };
    use windows::core::w;

    // The mutex lives as long as the process; Windows releases it on exit.
    let mutex = unsafe { CreateMutexW(None, true, w!("Local\\Roshan.SingleInstance")) };
    let already_running = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if mutex.is_err() || !already_running {
        return true;
    }
    // GPUI windows use this class; the title is set by Roshan.
    if let Ok(hwnd) = unsafe { FindWindowW(w!("Zed::Window"), w!("Roshan")) } {
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
    }
    false
}

pub fn system_language() -> Option<String> {
    let lang = unsafe { GetUserDefaultUILanguage() };
    let mut buf = [0u16; 85];
    let len = unsafe { LCIDToLocaleName(u32::from(lang), Some(&mut buf), 0) };
    (len > 1).then(|| String::from_utf16_lossy(&buf[..len as usize - 1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_non_apps() {
        let pf = "{6D809377-6AF0-444B-8957-A3773F02200E}";
        assert!(is_launchable_app("Telegram", "TelegramDesktop", None));
        assert!(is_launchable_app(
            "Calculator",
            "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App",
            None
        ));
        assert!(is_launchable_app(
            "Tool",
            &format!(r"{pf}\Vendor\tool.exe"),
            None
        ));
        assert!(is_launchable_app(
            "Tool",
            "Microsoft.AutoGenerated.{X}",
            Some(r"C:\Program Files\Vendor\tool.exe")
        ));
        assert!(!is_launchable_app(
            "Uninstall Tool",
            &format!(r"{pf}\Vendor\tool.exe"),
            None
        ));
        assert!(!is_launchable_app(
            "Remove",
            &format!(r"{pf}\Vendor\unins000.exe"),
            None
        ));
        assert!(!is_launchable_app(
            "Readme",
            &format!(r"{pf}\Vendor\readme.txt"),
            None
        ));
        assert!(!is_launchable_app("Website", "https://example.com/", None));
        assert!(!is_launchable_app(
            "Help",
            "Vendor.Help",
            Some("http://example.com/help")
        ));
        assert!(!is_launchable_app("Help", "x", Some(r"C:\Vendor\APP.HLP")));
        assert!(!is_launchable_app(
            "readme",
            "x",
            Some(r"C:\Windows\notepad.exe")
        ));
        assert!(!is_launchable_app(
            "license.txt",
            "x",
            Some(r"C:\Vendor\viewer.exe")
        ));
    }

    #[test]
    fn unpremultiplies_shell_bitmaps() {
        // One half-transparent pure blue pixel, premultiplied BGRA.
        let mut px = vec![128, 0, 0, 128];
        bgra_to_rgba(&mut px);
        assert_eq!(px, [0, 0, 255, 128]);
        // No alpha at all means fully opaque.
        let mut px = vec![10, 20, 30, 0];
        bgra_to_rgba(&mut px);
        assert_eq!(px, [30, 20, 10, 255]);
    }

    /// Exercises the real shell; run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn discovers_real_apps_with_icons() {
        let apps = crate::discover_apps();
        assert!(!apps.is_empty());
        for app in apps.iter().take(5) {
            println!("{} -> {:?} ({:?})", app.name, app.target, app.detail);
            let icon = app_icon(&app.target);
            assert!(icon.is_some(), "no icon for {}", app.name);
        }
    }
}
