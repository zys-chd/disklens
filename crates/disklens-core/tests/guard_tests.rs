use std::fs;
use std::path::{Path, PathBuf};

use disklens_core::is_protected;
use tempfile::TempDir;

struct Fx {
    _td: TempDir,
    home: PathBuf,
    outside: PathBuf,
}

fn fixture() -> Fx {
    let td = TempDir::new().unwrap();
    let home = td.path().join("home");
    let outside = td.path().join("scratch");
    for d in [
        home.join("Projects"),
        home.join("Documents"),
        home.join("Downloads"),
        home.join(".ssh"),
        home.join(".hermes"),
        outside.clone(),
    ] {
        fs::create_dir_all(&d).unwrap();
    }
    Fx {
        _td: td,
        home,
        outside,
    }
}

#[test]
fn rejects_system_root_and_drive_root() {
    assert_eq!(
        is_protected(Path::new("/"), Path::new("/home/zys")),
        Some("系统根目录受保护")
    );
}

#[test]
fn rejects_system_directories_and_their_children() {
    for p in [
        "/usr", "/etc", "/bin", "/lib", "/var/lib", "/boot", "/System",
    ] {
        assert_eq!(
            is_protected(Path::new(p), Path::new("/home/zys")),
            Some("系统目录受保护"),
            "{p}"
        );
    }
    assert_eq!(
        is_protected(Path::new("/etc/passwd"), Path::new("/home/zys")),
        Some("位于受保护路径内")
    );
    assert_eq!(
        is_protected(Path::new("/usr/local/bin/tool"), Path::new("/home/zys")),
        Some("位于受保护路径内")
    );
}

#[test]
fn rejects_home_itself_but_allows_ordinary_files_under_home() {
    let fx = fixture();
    assert_eq!(is_protected(&fx.home, &fx.home), Some("主目录不能删除"));
    let ordinary = fx.home.join("Downloads/movie.mkv");
    assert_eq!(is_protected(&ordinary, &fx.home), None);
    assert_eq!(is_protected(&fx.home.join("Music"), &fx.home), None);
}

#[test]
fn rejects_sensitive_subdirectories_and_anything_inside_them() {
    let fx = fixture();
    for (name, reason) in [
        ("Projects", "代码目录受保护"),
        ("Documents", "文档目录受保护"),
        (".ssh", "SSH 密钥目录受保护"),
        (".hermes", "Hermes 配置目录受保护"),
    ] {
        let p = fx.home.join(name);
        assert_eq!(is_protected(&p, &fx.home), Some(reason), "{name}");
        assert_eq!(
            is_protected(&p.join("some/deep/file.bin"), &fx.home),
            Some("位于受保护路径内"),
            "{name} 子项"
        );
    }
}

#[test]
fn rejects_ancestor_of_protected_paths() {
    let fx = fixture();
    assert_eq!(
        is_protected(fx.home.parent().unwrap(), &fx.home),
        Some("目标是受保护路径的上级目录")
    );
}

#[test]
fn rejects_parent_dir_component_and_empty_path() {
    let fx = fixture();
    assert_eq!(
        is_protected(&fx.home.join("Downloads/../Projects"), &fx.home),
        Some("路径包含 ..（禁止越界）")
    );
    assert_eq!(is_protected(Path::new(""), &fx.home), Some("路径为空"));
}

#[cfg(unix)]
#[test]
fn canonicalized_symlink_into_protected_dir_is_rejected() {
    use std::os::unix::fs::symlink;
    let fx = fixture();
    let link = fx.home.join("Downloads/projects-link");
    symlink(fx.home.join("Projects"), &link).unwrap();
    assert_eq!(
        is_protected(&link, &fx.home),
        Some("代码目录受保护"),
        "canonicalize 后应命中 Projects"
    );
}

#[test]
fn rejects_obsidian_vault_by_component_name() {
    let fx = fixture();
    let vault = fx.home.join("vaults/Obsidian/notes");
    fs::create_dir_all(&vault).unwrap();
    assert_eq!(is_protected(&vault, &fx.home), Some("Obsidian 仓库受保护"));
}

#[test]
fn allows_nonexistent_path_outside_protected_areas() {
    let fx = fixture();
    let ghost = fx.outside.join("already-deleted.iso");
    assert_eq!(is_protected(&ghost, &fx.home), None);
    let missing_dir = fx.home.join("Downloads/ghost-dir");
    assert_eq!(is_protected(&missing_dir, &fx.home), None);
}

#[test]
fn ignores_similarly_named_but_unrelated_paths() {
    let fx = fixture();
    let p = fx.outside.join("Projects-copy/Obsidian-notes/file.bin");
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    assert_eq!(is_protected(&p, &fx.home), None);
}
