//! 跨平台形状路径：归一化、安全分级、删除护栏。
//!
//! 这些用例全部在 Linux 上跑，但喂的是 Windows/macOS 形状的**字符串**，
//! 因为分级与护栏的比较建立在归一化字符串上，与当前平台无关。

use std::path::{Path, PathBuf};

use disklens_core::{is_protected, norm_path_str, Rule, RuleSet, SafetyLevel, ScanOptions};

fn home() -> PathBuf {
    PathBuf::from("/home/u")
}

fn rules() -> RuleSet {
    RuleSet::default_rules(&home())
}

fn p(s: &str) -> &Path {
    Path::new(s)
}

fn idx(rs: &RuleSet, pattern: &str) -> usize {
    rs.rules()
        .iter()
        .position(|r| r.pattern == pattern)
        .unwrap_or_else(|| panic!("规则表中找不到 {pattern}"))
}

// ───────────────────────── 归一化 ─────────────────────────

#[test]
fn norm_path_str_unifies_separators_prefix_and_trailing_slash() {
    assert_eq!(norm_path_str(p(r"C:\a\b\")), "C:/a/b");
    assert_eq!(norm_path_str(p("C:/a/b")), "C:/a/b");
    assert_eq!(norm_path_str(p(r"\\?\C:\a\b")), "C:/a/b");
    assert_eq!(norm_path_str(p("/a/b/")), "/a/b");
    assert_eq!(norm_path_str(p("/a/b")), "/a/b");
    // 盘符统一大写
    assert_eq!(norm_path_str(p(r"c:\A\B")), "C:/A/B");
    // 根不吞末尾分隔符
    assert_eq!(norm_path_str(p(r"C:\")), "C:/");
    assert_eq!(norm_path_str(p("/")), "/");
    assert_eq!(norm_path_str(p("")), "");
    // verbatim UNC 还原成 `//server/share`
    assert_eq!(
        norm_path_str(p(r"\\?\UNC\srv\share\x")),
        "//srv/share/x"
    );
}

// ───────────────────── Windows 形状路径的分级 ─────────────────────

#[test]
fn windows_shaped_paths_hit_the_same_rules_as_linux() {
    let rs = rules();

    let nm = rs.classify(p(r"C:\Users\zys\Projects\app\node_modules\pkg\index.js"));
    assert_eq!(nm.level, SafetyLevel::Careful);
    assert_eq!(rs.category(nm), "前端依赖");
    assert_eq!(nm.rule_idx(), Some(idx(&rs, "*/node_modules")));

    let pyc = rs.classify(p(r"C:\proj\pkg\__pycache__\m.pyc"));
    assert_eq!(pyc.level, SafetyLevel::Safe);
    assert_eq!(rs.category(pyc), "Python 缓存");
    assert_eq!(pyc.rule_idx(), Some(idx(&rs, "*/__pycache__")));

    let dbg = rs.classify(p(r"C:\proj\target\debug\deps\app.exe"));
    assert_eq!(dbg.level, SafetyLevel::Careful);
    assert_eq!(rs.category(dbg), "构建产物");
    assert_eq!(dbg.rule_idx(), Some(idx(&rs, "*/target/debug")));

    // 末尾分隔符不同不能改变结论
    let trailing = rs.classify(p(r"C:\proj\app\node_modules\"));
    assert_eq!(trailing.level, SafetyLevel::Careful);
    assert_eq!(trailing.rule_idx(), nm.rule_idx());

    // 混合分隔符（用户/工具混拼出来的路径）同样命中
    let mixed = rs.classify(p(r"C:/proj/app\node_modules/x"));
    assert_eq!(mixed.level, SafetyLevel::Careful);
    assert_eq!(mixed.rule_idx(), nm.rule_idx());
}

#[test]
fn windows_shaped_patterns_match_windows_shaped_paths() {
    // 规则文件里写 `C:\Windows\*` 与 `C:/Windows/*` 必须等价
    let rs = RuleSet::new(
        vec![
            Rule::new(r"C:\Windows\*", "系统", SafetyLevel::Keep, "win"),
            Rule::new(r"C:\Program Files*", "系统", SafetyLevel::Keep, "pf"),
            Rule::new(r"D:\Data\cache*", "缓存", SafetyLevel::Safe, "d"),
        ],
        &home(),
    );
    assert_eq!(
        rs.classify(p(r"C:\Windows\System32\drivers\etc\hosts")).level,
        SafetyLevel::Keep
    );
    assert_eq!(
        rs.classify(p("C:/Windows/System32")).level,
        SafetyLevel::Keep
    );
    assert_eq!(
        rs.classify(p(r"C:\Program Files (x86)\App\a.exe")).level,
        SafetyLevel::Keep
    );
    // 另一个盘符根，不能被当成 C: 或相对路径
    let d = rs.classify(p(r"D:\Data\cache\blob.tmp"));
    assert_eq!(d.level, SafetyLevel::Safe);
    assert_eq!(rs.category(d), "缓存");
}

// ───────────────────────── 护栏 ─────────────────────────

#[test]
fn guard_rejects_roots_system_dirs_and_lowercase_variants() {
    let h = p("/home/zys");
    assert_eq!(is_protected(p("/"), h), Some("系统根目录受保护"));
    assert_eq!(is_protected(p(r"C:\"), h), Some("系统根目录受保护"));
    assert_eq!(is_protected(p("C:/"), h), Some("系统根目录受保护"));
    assert_eq!(
        is_protected(p(r"\\?\C:\"), h),
        Some("系统根目录受保护"),
        "canonicalize 的 verbatim 前缀不能绕过盘符根判定"
    );

    assert_eq!(
        is_protected(p(r"C:\Windows"), h),
        Some("Windows 系统目录受保护")
    );
    assert_eq!(
        is_protected(p(r"C:\Windows\System32"), h),
        Some("位于受保护路径内")
    );
    assert_eq!(
        is_protected(p(r"c:\windows\system32"), h),
        Some("位于受保护路径内"),
        "Windows 路径大小写不敏感"
    );
    assert_eq!(
        is_protected(p(r"C:\Program Files\App\a.exe"), h),
        Some("位于受保护路径内")
    );
}

#[test]
fn guard_rejects_traversal_and_expands_tilde() {
    let tx = tempfile::tempdir().unwrap();
    let h = tx.path().join("home");
    std::fs::create_dir_all(h.join("Downloads")).unwrap();

    assert_eq!(is_protected(p(".."), &h), Some("路径包含 ..（禁止越界）"));
    assert_eq!(
        is_protected(p(r"C:\Users\z\..\Windows"), &h),
        Some("路径包含 ..（禁止越界）"),
        "Windows 形状的 `..` 也要拦住"
    );
    assert_eq!(
        is_protected(p("~"), &h),
        Some("主目录不能删除"),
        "`~` 就是 home，删了等于删主目录"
    );
    assert_eq!(
        is_protected(p("~/Downloads/a.mkv"), &h),
        None,
        "home 下的普通文件必须可删"
    );
}

#[test]
fn trailing_separator_does_not_bypass_guard() {
    let h = p("/home/zys");
    let with = is_protected(p("/usr/etc/"), h);
    let without = is_protected(p("/usr/etc"), h);
    assert_eq!(with, Some("位于受保护路径内"));
    assert_eq!(with, without);
    assert_eq!(
        is_protected(p(r"C:\Windows\System32\"), h),
        is_protected(p(r"C:\Windows\System32"), h)
    );
}

// ───────────── 现有 Linux 规则结果不变（回归锚点） ─────────────

#[test]
fn linux_rule_levels_are_unchanged() {
    let rs = rules();
    for (path, level, category) in [
        ("/home/u", SafetyLevel::Keep, "石墙"),
        ("/", SafetyLevel::Keep, "石墙"),
        ("/home/u/Projects/a/src/main.rs", SafetyLevel::Keep, "用户代码"),
        ("/home/u/Projects/a/target/debug/x", SafetyLevel::Careful, "构建产物"),
        ("/usr/lib/python3.14/site-packages/x.py", SafetyLevel::Keep, "系统"),
        ("/var/log/nginx/access.log", SafetyLevel::Careful, "系统日志"),
        ("/home/u/app/node_modules/x.js", SafetyLevel::Careful, "前端依赖"),
        ("/home/u/.cache/some-app/blob", SafetyLevel::Safe, "应用缓存"),
        ("/home/u/Downloads/a.iso", SafetyLevel::Unknown, "下载"),
        ("/tmp/scratch/out.o", SafetyLevel::Safe, "临时文件"),
        // `*/build` 排在 `/tmp/*` 之前：/tmp 下的 build 目录按构建产物算（既有行为）
        ("/tmp/build/x", SafetyLevel::Careful, "构建产物"),
        ("/home/u/.ssh/id_rsa", SafetyLevel::Keep, "密钥"),
    ] {
        let s = rs.classify(p(path));
        assert_eq!(s.level, level, "{path}");
        assert_eq!(rs.category(s), category, "{path}");
    }
    // 规则条数不变（平台追加项都在 cfg 里）
    #[cfg(all(unix, not(target_os = "macos")))]
    assert_eq!(rs.len(), 77);
}

/// 大小写折叠只在 macOS/Windows 生效；这里顺带锚定 Linux 保持区分。
#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn linux_matching_stays_case_sensitive() {
    let rs = rules();
    assert_eq!(
        rs.classify(p("/HOME/U/.SSH/id_rsa")).level,
        SafetyLevel::Unknown
    );
}

// ───────────── 硬链接去重能力上报（Windows 不做） ─────────────

#[test]
fn scan_stats_report_hardlink_dedup_capability() {
    let td = tempfile::tempdir().unwrap();
    let tree = disklens_core::scan(td.path(), &ScanOptions::default()).unwrap();
    assert_eq!(
        tree.stats.hardlink_dedup,
        cfg!(unix),
        "Unix 按 dev+ino 去重；Windows 稳定版 std 读不到 nlink/file_index，只能不去重"
    );
}
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn case_insensitive_platforms_fold_case_when_matching() {
    let rs = rules();
    assert_eq!(
        rs.classify(p("/HOME/U/.SSH/id_rsa")).level,
        SafetyLevel::Keep
    );
    assert_eq!(
        rs.classify(p(r"C:\PROJ\APP\NODE_MODULES\x")).level,
        SafetyLevel::Careful
    );
}
