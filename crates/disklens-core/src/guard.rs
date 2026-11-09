//! 不可删除路径护栏。返回拒绝原因；`None` 表示允许。
//! 覆盖 03-架构 §6：系统目录、home 及其敏感子目录、Windows 系统目录、
//! Obsidian 仓库、`..`、空路径，以及 canonicalize 后的父目录保护。
//!
//! 比较一律走 `norm_path_str`（`\`→`/`、盘符大写、去 `\\?\` 与尾部 `/`），
//! 否则 `/etc/` 与 `/etc`、`C:\Windows` 与 `C:/Windows` 会成为两条不同的路径。
//! shortcut: 比较始终不区分大小写（Linux 上也如此）—— 护栏宁可误拒，不可误放。

use std::fs;
use std::path::{Path, PathBuf};

use crate::path::{is_drive_abs, is_root, norm_path_str};

pub fn is_protected(path: &Path, home: &Path) -> Option<&'static str> {
    if path.as_os_str().is_empty() {
        return Some("路径为空");
    }
    let raw = norm_path_str(&expand_tilde(path, home));
    if raw.split('/').any(|c| c == "..") {
        return Some("路径包含 ..（禁止越界）");
    }
    let cand = key(&raw);
    let home_key = key(&norm_path_str(home));
    if is_root(&cand) {
        return Some("系统根目录受保护");
    }

    // (路径键, 拒绝原因, 是否连子项一起保护)
    let mut guarded: Vec<(String, &'static str, bool)> = Vec::new();
    for (p, reason) in [
        ("/System", "系统目录受保护"),
        ("/usr", "系统目录受保护"),
        ("/etc", "系统目录受保护"),
        ("/bin", "系统目录受保护"),
        ("/lib", "系统目录受保护"),
        ("/var/lib", "系统目录受保护"),
        ("/boot", "系统目录受保护"),
        // Windows 目录按字符串判定：跨平台都生效，Linux 上不会出现这种形状的真实路径
        ("C:/Windows", "Windows 系统目录受保护"),
        ("C:/Program Files", "Windows 系统目录受保护"),
        ("C:/Program Files (x86)", "Windows 系统目录受保护"),
    ] {
        guarded.push((key(p), reason, true));
    }
    // %SystemRoot% 可能不在 C: 上
    if let Some(root) = std::env::var_os("SystemRoot").or_else(|| std::env::var_os("windir")) {
        guarded.push((key(&norm_path_str(Path::new(&root))), "Windows 系统目录受保护", true));
    }
    // home 本身不可删，但 home 下的普通文件要能删，所以只做自身/上级保护。
    // 注意 %LOCALAPPDATA%\Temp 在 home 之下：这里不保护子项，它才清得掉。
    guarded.push((home_key.clone(), "主目录不能删除", false));
    for (name, reason) in [
        ("Projects", "代码目录受保护"),
        ("Documents", "文档目录受保护"),
        (".ssh", "SSH 密钥目录受保护"),
        (".hermes", "Hermes 配置目录受保护"),
    ] {
        guarded.push((key(&norm_path_str(&home.join(name))), reason, true));
    }

    if let Some(reason) = guarded
        .iter()
        .find(|(p, _, _)| *p == cand)
        .map(|(_, r, _)| *r)
    {
        return Some(reason);
    }
    if cand.split('/').any(|c| c == "obsidian") {
        return Some("Obsidian 仓库受保护");
    }
    if guarded
        .iter()
        .any(|(p, _, protect_children)| *protect_children && is_ancestor(p, &cand))
    {
        return Some("位于受保护路径内");
    }
    if guarded.iter().any(|(p, _, _)| is_ancestor(&cand, p)) {
        return Some("目标是受保护路径的上级目录");
    }
    None
}

fn expand_tilde(path: &Path, home: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s == "~" {
        home.to_path_buf()
    } else if let Some(rest) = s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\")) {
        home.join(rest)
    } else {
        path.to_path_buf()
    }
}

/// 绝对化 + canonicalize（解析符号链接）+ 归一化 + 折叠大小写。
fn key(path: &str) -> String {
    let abs = if path.starts_with('/') || is_drive_abs(path) {
        path.to_string()
    } else {
        match std::env::current_dir() {
            Ok(cwd) => format!("{}/{}", norm_path_str(&cwd), path),
            Err(_) => path.to_string(),
        }
    };
    fs::canonicalize(&abs)
        .ok()
        .map(|p| norm_path_str(&p))
        .or_else(|| {
            // 目标可能已不存在：规范化父目录，再拼回文件名
            let p = Path::new(&abs);
            let dir = p.parent()?;
            let name = p.file_name()?;
            let dir = fs::canonicalize(dir).ok()?;
            Some(format!("{}/{}", norm_path_str(&dir), name.to_string_lossy()))
        })
        .unwrap_or(abs)
        .to_lowercase()
}

/// a 是否为 b 的严格上级（`/usr` ⊂ `/usr/bin`，但 `/usr` ⊄ `/usr-local`）
fn is_ancestor(a: &str, b: &str) -> bool {
    b.len() > a.len() && b.starts_with(a) && b.as_bytes()[a.len()] == b'/'
}
