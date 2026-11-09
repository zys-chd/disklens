use std::fs;
use std::path::{Component, Path, PathBuf};

/// 不可删除路径护栏。返回拒绝原因；`None` 表示允许。
/// 覆盖 03-架构 §6：系统目录、home 及其敏感子目录、Windows 系统目录、
/// Obsidian 仓库、`..`、空路径，以及 canonicalize 后的父目录保护。
pub fn is_protected(path: &Path, home: &Path) -> Option<&'static str> {
    if path.as_os_str().is_empty() {
        return Some("路径为空");
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Some("路径包含 ..（禁止越界）");
    }
    let cand = normalize(path);
    let home = normalize(home);
    if is_root(&cand) {
        return Some("系统根目录受保护");
    }

    // (路径, 拒绝原因, 是否连子项一起保护)
    let mut guarded: Vec<(PathBuf, &'static str, bool)> = Vec::new();
    let mut add = |p: PathBuf, reason: &'static str, children: bool| {
        guarded.push((normalize(&p), reason, children));
    };
    for p in [
        "/System", "/usr", "/etc", "/bin", "/lib", "/var/lib", "/boot",
    ] {
        add(PathBuf::from(p), "系统目录受保护", true);
    }
    // home 本身不可删，但 home 下的普通文件要能删，所以只做自身/上级保护
    add(home.clone(), "主目录不能删除", false);
    add(home.join("Projects"), "代码目录受保护", true);
    add(home.join("Documents"), "文档目录受保护", true);
    add(home.join(".ssh"), "SSH 密钥目录受保护", true);
    add(home.join(".hermes"), "Hermes 配置目录受保护", true);
    #[cfg(windows)]
    {
        for p in [
            "C:\\Windows",
            "C:\\Program Files",
            "C:\\Program Files (x86)",
        ] {
            add(PathBuf::from(p), "Windows 系统目录受保护", true);
        }
        if let Some(root) = std::env::var_os("SystemRoot") {
            add(PathBuf::from(root), "Windows 系统目录受保护", true);
        }
    }

    if let Some(reason) = guarded
        .iter()
        .find(|(p, _, _)| same_path(p, &cand))
        .map(|(_, r, _)| *r)
    {
        return Some(reason);
    }
    if cand.components().any(|c| {
        c.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("obsidian")
    }) {
        return Some("Obsidian 仓库受保护");
    }
    for (p, _, protect_children) in &guarded {
        if *protect_children && is_ancestor(p, &cand) {
            return Some("位于受保护路径内");
        }
    }
    for (p, _, _) in &guarded {
        if is_ancestor(&cand, p) {
            return Some("目标是受保护路径的上级目录");
        }
    }
    None
}

fn normalize(path: &Path) -> PathBuf {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    if let Ok(p) = fs::canonicalize(&abs) {
        return p;
    }
    // 目标可能已不存在：规范化父目录，再拼回文件名
    match (
        abs.parent().and_then(|p| fs::canonicalize(p).ok()),
        abs.file_name(),
    ) {
        (Some(parent), Some(name)) => parent.join(name),
        _ => abs,
    }
}

fn is_root(p: &Path) -> bool {
    p.components()
        .all(|c| matches!(c, Component::RootDir | Component::Prefix(_)))
}

fn comps(p: &Path) -> Vec<String> {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect()
}

fn same_path(a: &Path, b: &Path) -> bool {
    comps(a) == comps(b)
}

/// a 是否为 b 的严格上级
fn is_ancestor(a: &Path, b: &Path) -> bool {
    let (ca, cb) = (comps(a), comps(b));
    ca.len() < cb.len() && cb[..ca.len()] == ca[..]
}
