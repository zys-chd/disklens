//! 在系统文件管理器中打开/定位路径。三平台各一套，失败返回可展示的错误。
//!
//! 命令构造抽成纯函数 `open_argv`：这样在 Linux 上也能断言 Windows/macOS 的 argv 对不对，
//! 而不用真的把 explorer/open 跑起来。

use std::path::Path;

/// 命令构造按平台参数化（只用于选择规则，不用于探测当前系统）。
/// 每个平台只构造自己那个变体，单测会构造全部三个。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Platform {
    Linux,
    Windows,
    MacOs,
}

/// 纯函数：返回 argv（argv[0] 为程序名）。不执行任何东西。
pub fn open_argv(platform: Platform, path: &str, reveal: bool) -> Vec<String> {
    match platform {
        Platform::Windows => {
            // explorer 只认原生分隔符；`/select,` 必须与路径同一个参数
            let native = path.replace('/', "\\");
            if reveal {
                vec!["explorer".into(), format!("/select,{native}")]
            } else {
                vec!["explorer".into(), native]
            }
        }
        Platform::MacOs => {
            let mut argv = vec!["open".to_string()];
            if reveal {
                argv.push("-R".into());
            }
            argv.push(path.to_string());
            argv
        }
        Platform::Linux => {
            // FileManager1 定位失败时回退到父目录
            let target = if reveal { parent_dir(path) } else { path };
            vec!["xdg-open".into(), target.to_string()]
        }
    }
}

fn parent_dir(path: &str) -> &str {
    match path.rsplit_once('/') {
        Some((parent, _)) if !parent.is_empty() => parent,
        Some(_) => "/",
        None => path,
    }
}

#[cfg(target_os = "linux")]
pub fn open_path(path: &Path, reveal: bool) -> Result<(), String> {
    if reveal && show_items(path).is_ok() {
        return Ok(());
    }
    run(&open_argv(Platform::Linux, &path.to_string_lossy(), reveal))
}

/// FileManager1.ShowItems 只接受 file URI；用 gdbus 调用，避免为此引入 dbus 依赖。
#[cfg(target_os = "linux")]
fn show_items(path: &Path) -> Result<(), String> {
    let items = format!("['{}']", file_uri(path));
    run(&[
        "gdbus".into(),
        "call".into(),
        "--session".into(),
        "--dest".into(),
        "org.freedesktop.FileManager1".into(),
        "--object-path".into(),
        "/org/freedesktop/FileManager1".into(),
        "--method".into(),
        "org.freedesktop.FileManager1.ShowItems".into(),
        items,
        String::new(),
    ])
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

// shortcut: 交给 explorer/open 的路径按 lossy UTF-8 传递；非法 UTF-16 的路径打不开（极罕见）
#[cfg(target_os = "windows")]
pub fn open_path(path: &Path, reveal: bool) -> Result<(), String> {
    run(&open_argv(
        Platform::Windows,
        &path.to_string_lossy(),
        reveal,
    ))
}
#[cfg(target_os = "macos")]
pub fn open_path(path: &Path, reveal: bool) -> Result<(), String> {
    run(&open_argv(Platform::MacOs, &path.to_string_lossy(), reveal))
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn open_path(_path: &Path, _reveal: bool) -> Result<(), String> {
    Err("当前平台不支持打开路径".into())
}

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
fn run(argv: &[String]) -> Result<(), String> {
    let Some((cmd, args)) = argv.split_first() else {
        return Err("空命令".into());
    };
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

#[cfg(test)]
mod tests {
    use super::{open_argv, parent_dir, Platform};

    #[test]
    fn windows_argv_uses_native_separators_and_select_prefix() {
        assert_eq!(
            open_argv(Platform::Windows, r"C:\a\b", true),
            vec!["explorer", r"/select,C:\a\b"]
        );
        assert_eq!(
            open_argv(Platform::Windows, r"C:\a\b", false),
            vec!["explorer", r"C:\a\b"]
        );
        // 归一化后的 `/` 形状也要能还原成 explorer 认的原生路径
        assert_eq!(
            open_argv(Platform::Windows, "C:/a/b c", true),
            vec!["explorer", r"/select,C:\a\b c"],
            "带空格的路径必须是同一个参数"
        );
    }

    #[test]
    fn macos_argv_reveal_is_dash_capital_r() {
        assert_eq!(
            open_argv(Platform::MacOs, "/a/b", true),
            vec!["open", "-R", "/a/b"]
        );
        assert_eq!(
            open_argv(Platform::MacOs, "/a/b", false),
            vec!["open", "/a/b"]
        );
    }

    #[test]
    fn linux_argv_reveal_falls_back_to_parent() {
        assert_eq!(
            open_argv(Platform::Linux, "/a/b", true),
            vec!["xdg-open", "/a"]
        );
        assert_eq!(
            open_argv(Platform::Linux, "/a/b", false),
            vec!["xdg-open", "/a/b"]
        );
        assert_eq!(parent_dir("/b"), "/");
        assert_eq!(parent_dir("b"), "b");
    }
}
