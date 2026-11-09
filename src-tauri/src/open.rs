//! 在系统文件管理器中打开/定位路径。三平台各一套，失败返回可展示的错误。

use std::path::Path;

#[cfg(target_os = "linux")]
pub fn open_path(path: &Path, reveal: bool) -> Result<(), String> {
    if reveal && show_items(path).is_ok() {
        return Ok(());
    }
    // 回退：定位失败就打开父目录；打开本身则直接交给 xdg-open
    let target = if reveal {
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(path)
    } else {
        path
    };
    run("xdg-open", &[target])
}

/// FileManager1.ShowItems 只接受 file URI；用 gdbus 调用，避免为此引入 dbus 依赖。
#[cfg(target_os = "linux")]
fn show_items(path: &Path) -> Result<(), String> {
    let uri = file_uri(path);
    run(
        "gdbus",
        &[
            Path::new("call"),
            Path::new("--session"),
            Path::new("--dest"),
            Path::new("org.freedesktop.FileManager1"),
            Path::new("--object-path"),
            Path::new("/org/freedesktop/FileManager1"),
            Path::new("--method"),
            Path::new("org.freedesktop.FileManager1.ShowItems"),
            Path::new(&format!("['{uri}']")),
            Path::new(""),
        ],
    )
}

#[cfg(target_os = "linux")]
fn file_uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut s = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~') {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

#[cfg(target_os = "windows")]
pub fn open_path(path: &Path, reveal: bool) -> Result<(), String> {
    use std::ffi::OsString;
    if reveal {
        let mut arg = OsString::from("/select,");
        arg.push(path);
        run("explorer", &[&arg])
    } else {
        run("explorer", &[path.as_os_str()])
    }
}
#[cfg(target_os = "macos")]
pub fn open_path(path: &Path, reveal: bool) -> Result<(), String> {
    if reveal {
        run("open", &[Path::new("-R"), path])
    } else {
        run("open", &[path])
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn open_path(_path: &Path, _reveal: bool) -> Result<(), String> {
    Err("当前平台不支持打开路径".into())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run(cmd: &str, args: &[&Path]) -> Result<(), String> {
    let status = std::process::Command::new(cmd)
        .args(args)
        .status()
        .map_err(|e| format!("启动 {cmd} 失败: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{cmd} 退出码 {status}"))
    }
}

#[cfg(target_os = "windows")]
fn run(cmd: &str, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let status = std::process::Command::new(cmd)
        .args(args)
        .status()
        .map_err(|e| format!("启动 {cmd} 失败: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{cmd} 退出码 {status}"))
    }
}
