use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::guard::is_protected;

#[derive(Clone, Debug, Serialize)]
pub struct OpFailure {
    pub path: String,
    pub error: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct OpResult {
    pub ok: Vec<PathBuf>,
    pub failed: Vec<OpFailure>,
}

impl OpResult {
    fn ok(&mut self, p: &Path) {
        self.ok.push(p.to_path_buf());
    }

    fn fail(&mut self, p: &Path, error: impl ToString) {
        self.failed.push(OpFailure {
            path: p.to_string_lossy().into_owned(),
            error: error.to_string(),
        });
    }
}

/// 逐项移入回收站。回收站失败只报 failed，绝不降级为永久删除。
pub fn move_to_trash(paths: &[PathBuf], home: &Path) -> OpResult {
    let mut out = OpResult::default();
    for path in paths {
        if let Some(reason) = is_protected(path, home) {
            out.fail(path, reason);
            continue;
        }
        if !path.exists() && fs::symlink_metadata(path).is_err() {
            out.fail(path, "路径不存在");
            continue;
        }
        match trash::delete(path) {
            Ok(()) => out.ok(path),
            Err(e) => out.fail(path, format!("回收站操作失败: {e}")),
        }
    }
    out
}

/// 永久删除。confirm_text 必须逐项等于 basename；不匹配的项报 failed 且不动。
pub fn delete_permanent(paths: &[PathBuf], confirm_text: &str, home: &Path) -> OpResult {
    let mut out = OpResult::default();
    for path in paths {
        if let Some(reason) = is_protected(path, home) {
            out.fail(path, reason);
            continue;
        }
        let base = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if base.is_empty() || base != confirm_text {
            out.fail(path, "确认文本与目标名称不符");
            continue;
        }
        match remove_any(path) {
            Ok(()) => out.ok(path),
            Err(e) => out.fail(path, format!("删除失败: {e}")),
        }
    }
    out
}

fn remove_any(path: &Path) -> std::io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}
