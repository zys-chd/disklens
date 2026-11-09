//! 路径字符串归一化：安全分级与删除护栏的比较都建立在它之上。
//!
//! globset 只在 Windows 上把候选路径里的 `\` 换成 `/`，Linux 上原样比较。
//! 只要两边分隔符形状不一致（Windows 形状的路径、手写的规则），
//! `*/node_modules` 这类模式就会静默失配 —— 而分级是删除动作的安全依据。
//! 所以模式与路径都先过 `norm_path_str`，比较不再依赖 globset 的平台细节。

use std::borrow::Cow;
use std::path::Path;

/// Windows/macOS 默认文件系统不区分大小写；Linux 区分。
#[cfg(any(windows, target_os = "macos"))]
pub(crate) const CASE_INSENSITIVE_FS: bool = true;
#[cfg(not(any(windows, target_os = "macos")))]
pub(crate) const CASE_INSENSITIVE_FS: bool = false;

/// 统一成 `C:/a/b` 形状：`\`→`/`、去 `\\?\` 前缀、盘符大写、去尾部 `/`（根除外）。
///
/// shortcut: 替换不分平台 —— Linux 文件名里的 `\` 会被当成分隔符；
/// 只影响分级/护栏判定（偏保守），不影响真实系统调用。
pub fn norm_path_str(p: &Path) -> String {
    let mut s = p.to_string_lossy().replace('\\', "/");
    if let Some(rest) = s.strip_prefix("//?/") {
        // `\\?\C:\a`（canonicalize 的产物）与 `\\?\UNC\srv\share`
        s = match rest.strip_prefix("UNC/") {
            Some(unc) => format!("//{unc}"),
            None => rest.to_string(),
        };
    }
    let b = s.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        let upper = s[..1].to_ascii_uppercase();
        s.replace_range(..1, &upper);
    }
    while s.ends_with('/') && !is_root(&s) {
        s.pop();
    }
    s
}

/// 根：`/`、`//`、`C:/`
pub(crate) fn is_root(s: &str) -> bool {
    let b = s.as_bytes();
    if b == b"/" || b == b"//" {
        return true;
    }
    b.len() == 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'/'
}

/// `C:/...` —— 盘符绝对路径（Linux 上不会被当作绝对路径，需单独识别）
pub(crate) fn is_drive_abs(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'/'
}

/// 匹配键：macOS/Windows 上折叠大小写，Linux 原样。
pub(crate) fn case_key(s: &str) -> Cow<'_, str> {
    if CASE_INSENSITIVE_FS {
        Cow::Owned(s.to_lowercase())
    } else {
        Cow::Borrowed(s)
    }
}
