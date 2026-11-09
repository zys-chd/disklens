use std::fs;
use std::path::PathBuf;

use disklens_core::{delete_permanent, move_to_trash};
use tempfile::TempDir;

struct Fx {
    _td: TempDir,
    home: PathBuf,
    downloads: PathBuf,
}

fn fixture() -> Fx {
    let td = TempDir::new().unwrap();
    let home = td.path().join("home");
    let downloads = home.join("Downloads");
    fs::create_dir_all(&downloads).unwrap();
    fs::create_dir_all(home.join("Projects")).unwrap();
    Fx {
        _td: td,
        home,
        downloads,
    }
}

#[test]
fn delete_permanent_requires_confirm_text_equal_to_basename() {
    let fx = fixture();
    let target = fx.downloads.join("big.iso");
    fs::write(&target, b"x").unwrap();
    let res = delete_permanent(std::slice::from_ref(&target), "wrong", &fx.home);
    assert!(res.ok.is_empty());
    assert_eq!(res.failed.len(), 1);
    assert_eq!(res.failed[0].error, "确认文本与目标名称不符");
    assert!(target.exists(), "确认失败时文件必须原样保留");
}

#[test]
fn delete_permanent_removes_file_when_confirm_matches() {
    let fx = fixture();
    let target = fx.downloads.join("big.iso");
    fs::write(&target, b"x").unwrap();
    let res = delete_permanent(std::slice::from_ref(&target), "big.iso", &fx.home);
    assert_eq!(res.ok, vec![target.clone()]);
    assert!(res.failed.is_empty());
    assert!(!target.exists());
}

#[test]
fn delete_permanent_removes_directory_recursively() {
    let fx = fixture();
    let target = fx.downloads.join("cache");
    fs::create_dir_all(target.join("deep")).unwrap();
    fs::write(target.join("deep/a.bin"), b"a").unwrap();
    let res = delete_permanent(std::slice::from_ref(&target), "cache", &fx.home);
    assert_eq!(res.ok, vec![target.clone()]);
    assert!(!target.exists());
}

#[test]
fn delete_permanent_reports_partial_success_per_item() {
    let fx = fixture();
    let one = fx.downloads.join("one.bin");
    let two = fx.downloads.join("two.bin");
    fs::write(&one, b"1").unwrap();
    fs::write(&two, b"2").unwrap();
    let res = delete_permanent(&[one.clone(), two.clone()], "one.bin", &fx.home);
    assert_eq!(res.ok, vec![one.clone()]);
    assert_eq!(res.failed.len(), 1);
    assert_eq!(res.failed[0].path, two.to_string_lossy());
    assert!(!one.exists());
    assert!(two.exists(), "没通过确认的项不能删");
}

#[test]
fn delete_permanent_refuses_protected_path() {
    let fx = fixture();
    let projects = fx.home.join("Projects");
    let res = delete_permanent(std::slice::from_ref(&projects), "Projects", &fx.home);
    assert!(res.ok.is_empty());
    assert_eq!(res.failed[0].error, "代码目录受保护");
    assert!(projects.exists());
}

#[cfg(unix)]
#[test]
fn delete_permanent_removes_symlink_but_not_its_target() {
    use std::os::unix::fs::symlink;
    let fx = fixture();
    let target = fx.downloads.join("real.bin");
    fs::write(&target, b"data").unwrap();
    let link = fx.downloads.join("link.bin");
    symlink(&target, &link).unwrap();
    let res = delete_permanent(std::slice::from_ref(&link), "link.bin", &fx.home);
    assert_eq!(res.ok, vec![link.clone()]);
    assert!(!link.exists());
    assert!(target.exists(), "只能删链接本身");
}

#[test]
fn move_to_trash_refuses_protected_path_without_touching_it() {
    let fx = fixture();
    let projects = fx.home.join("Projects");
    let res = move_to_trash(std::slice::from_ref(&projects), &fx.home);
    assert!(res.ok.is_empty());
    assert_eq!(res.failed.len(), 1);
    assert_eq!(res.failed[0].error, "代码目录受保护");
    assert!(projects.exists());
}

#[test]
fn move_to_trash_failure_never_falls_back_to_permanent_delete() {
    let fx = fixture();
    let missing = fx.downloads.join("not-there.bin");
    let res = move_to_trash(std::slice::from_ref(&missing), &fx.home);
    assert!(res.ok.is_empty());
    assert_eq!(res.failed.len(), 1);
    assert!(!missing.exists());
}

#[test]
fn move_to_trash_either_moves_or_reports_failure_but_never_deletes() {
    let fx = fixture();
    let f = fx.downloads.join("trash-me.bin");
    fs::write(&f, b"x").unwrap();
    let res = move_to_trash(std::slice::from_ref(&f), &fx.home);
    assert_eq!(res.ok.len() + res.failed.len(), 1);
    if res.ok.len() == 1 {
        assert!(!f.exists(), "成功入回收站后原路径应消失");
    } else {
        assert!(f.exists(), "回收站失败时绝不能降级为删除");
    }
}
