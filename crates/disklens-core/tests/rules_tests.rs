//! 安全分级规则引擎测试：真实路径场景、继承/顺序/石墙语义、规则文件、统计与性能。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use disklens_core::{
    classify_tree, scan, Node, NodeId, NodeKind, Rule, RuleSet, SafetyLevel, ScanOptions,
    ScanStats, Tree, NO_PARENT,
};

/// Windows 上 `/home/u` 是「盘符相对路径」，会被归一化加上 cwd 前缀，导致与规则里的
/// 绝对模式对不上。所以测试用的 home 必须按平台给成真实绝对路径。
#[cfg(windows)]
fn home() -> PathBuf {
    PathBuf::from(r"C:\Users\u")
}
#[cfg(not(windows))]
fn home() -> PathBuf {
    PathBuf::from("/home/u")
}

fn rules() -> RuleSet {
    RuleSet::default_rules(&home())
}

fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

fn idx(rs: &RuleSet, pattern: &str) -> usize {
    rs.rules()
        .iter()
        .position(|r| r.pattern == pattern)
        .unwrap_or_else(|| panic!("规则表中找不到 {pattern}"))
}

#[test]
fn default_table_is_ported_completely() {
    let rs = rules();
    // 顺序契约：系统段在最前，石墙在最后；缓存兜底在用户数据之前
    #[cfg(windows)]
    assert_eq!(rs.rules()[0].pattern, r"C:\Windows");
    #[cfg(target_os = "macos")]
    assert_eq!(rs.rules()[0].pattern, "/System");
    #[cfg(not(any(windows, target_os = "macos")))]
    assert_eq!(rs.rules()[0].pattern, "/usr");
    assert_eq!(rs.rules().last().unwrap().pattern, "/");
    assert!(idx(&rs, "/usr/*") < idx(&rs, "*/node_modules"));
    assert!(idx(&rs, "*/node_modules") < idx(&rs, "/tmp/*"));
    assert!(idx(&rs, "~/.cache/video-upscaler*") < idx(&rs, "~/.cache/*"));
    assert!(idx(&rs, "~/.cache/*") < idx(&rs, "~/Projects"));
    assert!(idx(&rs, "~/Projects") < idx(&rs, "~/Downloads/*"));
    assert!(idx(&rs, "~/.*") < idx(&rs, "~"));
    // 平台追加的 exact 规则：Windows 多 `C:\Windows`，macOS 多 `/System`
    let exact = rs.rules().iter().filter(|r| r.exact_only).count();
    #[cfg(not(any(windows, target_os = "macos")))]
    assert_eq!(exact, 8, "exact_only = /usr /etc /boot /opt /var/tmp 与石墙 ~ /home /");
    #[cfg(any(windows, target_os = "macos"))]
    assert_eq!(exact, 9, "平台系统目录多一条 exact 规则");
    #[cfg(not(any(windows, target_os = "macos")))]
    assert_eq!(rs.len(), 77);
}

#[test]
fn node_modules_careful_is_inherited_by_deep_file() {
    let rs = rules();
    let s = rs.classify(&p("/home/u/app/node_modules/foo/x.js"));
    assert_eq!(s.level, SafetyLevel::Careful);
    assert_eq!(rs.category(s), "前端依赖");
    // 胜出的是祖先规则，不是 x.js 自身
    assert_eq!(s.rule_idx(), Some(idx(&rs, "*/node_modules")));
    // 目录本身也是同一规则
    let dir = rs.classify(&p("/home/u/app/node_modules"));
    assert_eq!(dir.level, SafetyLevel::Careful);
    assert_eq!(dir.rule_idx(), s.rule_idx());
}

#[test]
fn specific_cache_rule_beats_generic_cache_fallback() {
    let rs = rules();
    // 用 home() 拼路径而不是写死 "/home/u/..."：Windows 上 "/..." 是盘符相对路径，
    // 会被归一化成 cwd 前缀，与规则里的绝对模式对不上（Linux 上两者等价）
    let s = rs.classify(&home().join(".cache/video-upscaler/tmp/a.bin"));
    assert_eq!(s.level, SafetyLevel::Safe);
    assert_eq!(rs.category(s), "应用缓存");
    assert_eq!(s.rule_idx(), Some(idx(&rs, "~/.cache/video-upscaler*")));
    assert!(s.rule_idx().unwrap() < idx(&rs, "~/.cache/*"));

    // 未列出的应用缓存落到 ~/.cache/* 兜底
    let generic = rs.classify(&home().join(".cache/some-new-app/blob"));
    assert_eq!(generic.level, SafetyLevel::Safe);
    assert_eq!(generic.rule_idx(), Some(idx(&rs, "~/.cache/*")));
}

#[test]
fn user_config_kept_and_projects_build_artifacts_win_on_order() {
    let rs = rules();
    let cfg = rs.classify(&home().join(".config/x"));
    assert_eq!(cfg.level, SafetyLevel::Keep);
    assert_eq!(rs.category(cfg), "配置");

    // ~/Projects 是 keep，但更靠前的 */target/debug 应赢（顺序优先）
    let proj = rs.classify(&home().join("Projects/a"));
    assert_eq!(proj.level, SafetyLevel::Keep);
    let target = rs.classify(&home().join("Projects/a/target"));
    assert_eq!(target.level, SafetyLevel::Keep);
    let debug = rs.classify(&home().join("Projects/a/target/debug"));
    assert_eq!(debug.level, SafetyLevel::Careful);
    assert_eq!(rs.category(debug), "构建产物");
    assert_eq!(debug.rule_idx(), Some(idx(&rs, "*/target/debug")));
    let deep = rs.classify(&home().join("Projects/a/target/debug/x"));
    assert_eq!(deep.level, SafetyLevel::Careful);
    assert_eq!(deep.rule_idx(), debug.rule_idx());

    // 对照组：不在 Projects 下的 target/debug 同样按构建产物处理
    let elsewhere = rs.classify(&home().join("code/b/target/debug/y"));
    assert_eq!(elsewhere.level, SafetyLevel::Careful);
}

/// `/usr` 是 unix 系统目录规则；Windows 的等价项（C:\Windows）在 platform_paths.rs 里测
#[cfg(unix)]
#[test]
fn system_rule_keeps_site_packages_under_usr() {
    let rs = rules();
    let usr = rs.classify(&p("/usr/lib/python3.14/site-packages/x"));
    assert_eq!(usr.level, SafetyLevel::Keep);
    assert_eq!(rs.category(usr), "系统");
    assert_eq!(usr.rule_idx(), Some(idx(&rs, "/usr/*")));

    // 非系统的 site-packages 才落到 careful 虚拟环境规则
    let venv = rs.classify(&p("/mnt/data/venv/lib/site-packages/x"));
    assert_eq!(venv.level, SafetyLevel::Careful);
    assert_eq!(rs.category(venv), "Python 虚拟环境");
    assert_eq!(venv.rule_idx(), Some(idx(&rs, "*/site-packages")));
}

/// `/var/log`、`/var/cache` 是 unix 专属规则；Windows 上没有这些路径，
/// 等价的 Windows 规则（%LOCALAPPDATA%\Temp 等）在 platform_paths.rs 里单独测。
#[cfg(unix)]
#[test]
fn slash_star_crosses_separators_like_fnmatch() {
    let rs = rules();
    let log = rs.classify(&p("/var/log/nginx/access.log"));
    assert_eq!(log.level, SafetyLevel::Careful);
    assert_eq!(rs.category(log), "系统日志");
    assert_eq!(log.rule_idx(), Some(idx(&rs, "/var/log/*")));

    let pkg = rs.classify(&p("/var/cache/pacman/pkg/foo.pkg.tar.zst"));
    assert_eq!(pkg.level, SafetyLevel::Safe);
    assert_eq!(rs.category(pkg), "包管理器缓存");
    assert_eq!(pkg.rule_idx(), Some(idx(&rs, "/var/cache/pacman/pkg*")));
    assert!(rs.rule_at(pkg.rule_idx().unwrap()).unwrap().needs_root);
}

#[test]
fn pycache_matches_by_basename_anywhere() {
    let rs = rules();
    let s = rs.classify(&p("/mnt/data/proj/__pycache__/m.pyc"));
    assert_eq!(s.level, SafetyLevel::Safe);
    assert_eq!(rs.category(s), "Python 缓存");

    // 自定义无斜杠模式，只能靠 basename 命中（旧版 _hit 两种都试）
    let custom = RuleSet::new(
        vec![Rule::new("__pycache__", "缓存", SafetyLevel::Safe, "")],
        &home(),
    );
    assert_eq!(
        custom.classify(&p("/mnt/a/b/__pycache__/x.pyc")).level,
        SafetyLevel::Safe
    );
    assert_eq!(
        custom.classify(&p("/mnt/a/b/__pycache__")).level,
        SafetyLevel::Safe
    );
    assert_eq!(
        custom.classify(&p("/mnt/a/b/c.pyc")).level,
        SafetyLevel::Unknown
    );
}

#[test]
fn home_wall_is_exact_only_and_not_inherited_by_downloads() {
    let rs = rules();
    assert_eq!(rs.classify(&home()).level, SafetyLevel::Keep);
    assert_eq!(rs.category(rs.classify(&home())), "石墙");

    let downloads = rs.classify(&home().join("Downloads"));
    assert_eq!(downloads.level, SafetyLevel::Unknown);
    assert_eq!(downloads.rule_idx(), None);

    let foo = rs.classify(&home().join("Downloads/foo"));
    assert_eq!(foo.level, SafetyLevel::Unknown);
    assert_eq!(rs.category(foo), "下载");
    assert_eq!(foo.rule_idx(), Some(idx(&rs, "~/Downloads/*")));
}

#[test]
fn rules_file_created_on_first_run_and_reloadable() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("disklens").join("rules.json");
    let rs = RuleSet::load_or_create(&path, &home());
    assert!(path.exists(), "首次运行应自动创建规则文件");
    assert_eq!(rs.len(), RuleSet::default_rules(&home()).len());

    let text = fs::read_to_string(&path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["version"], 1);
    assert_eq!(json["rules"].as_array().unwrap().len(), rs.len());
    assert!(text.contains("needs_root"));
    assert!(text.contains("\"level\": \"careful\"") || text.contains("careful"));

    let rs2 = RuleSet::load_or_create(&path, &home());
    assert_eq!(rs2.len(), rs.len());
    let s = rs2.classify(&p("/home/u/app/node_modules/x"));
    assert_eq!(s.level, SafetyLevel::Careful);
    assert_eq!(rs2.category(s), "前端依赖");
}

#[test]
fn corrupt_rules_file_falls_back_to_default_without_panic() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("rules.json");
    fs::write(&path, "{ this is not json").unwrap();
    let rs = RuleSet::load_or_create(&path, &home());
    assert_eq!(rs.len(), RuleSet::default_rules(&home()).len());
    // 损坏文件不被覆盖
    assert_eq!(fs::read_to_string(&path).unwrap(), "{ this is not json");
    assert_eq!(
        rs.classify(&p("/home/u/app/node_modules/x")).level,
        SafetyLevel::Careful
    );
}

#[test]
fn unknown_level_string_maps_to_unknown() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("rules.json");
    fs::write(
        &path,
        r#"{"version":1,"rules":[{"pattern":"*.pyc","category":"自定义","level":"SuperSafe"}]}"#,
    )
    .unwrap();
    let rs = RuleSet::load_or_create(&path, &home());
    assert_eq!(rs.len(), 1);
    // 用 basename 规则匹配，避免依赖平台绝对路径形状
    let s = rs.classify(&home().join("a/b/x.pyc"));
    assert_eq!(s.level, SafetyLevel::Unknown);
    assert_eq!(rs.category(s), "自定义");
    assert_eq!(s.rule_idx(), Some(0));
}

// ───────────────────────── 统计 ─────────────────────────

fn dir(name: &str, parent: NodeId, size: u64, files: u32, children: Vec<NodeId>) -> Node {
    Node {
        name: name.into(),
        parent,
        children,
        size,
        own_size: 0,
        file_count: files,
        dir_count: 0,
        mtime: 0,
        kind: NodeKind::Dir,
        ext_id: 0,
        depth: 0,
        error_count: 0,
    }
}

fn file(name: &str, parent: NodeId, size: u64) -> Node {
    Node {
        name: name.into(),
        parent,
        children: Vec::new(),
        size,
        own_size: size,
        file_count: 0,
        dir_count: 0,
        mtime: 0,
        kind: NodeKind::File,
        ext_id: 0,
        depth: 0,
        error_count: 0,
    }
}

fn tree_of(nodes: Vec<Node>, root_path: &str) -> Tree {
    Tree {
        nodes,
        root: 0,
        root_path: PathBuf::from(root_path),
        exts: vec!["".into()],
        ext_stats: vec![Default::default()],
        stats: ScanStats::default(),
        safety: Vec::new(),
    }
}

#[test]
fn safety_stats_counts_nested_same_group_once() {
    let nodes = vec![
        dir("u", NO_PARENT, 950, 10, vec![1, 3, 5]),
        dir("node_modules", 0, 600, 6, vec![2]),
        dir("foo", 1, 600, 6, vec![]),
        dir("Downloads", 0, 300, 3, vec![4]),
        dir("__pycache__", 3, 50, 1, vec![]),
        file("a.bin", 0, 50),
    ];
    let mut tree = tree_of(nodes, &home().to_string_lossy());
    let rs = rules();
    classify_tree(&mut tree, &rs);
    assert_eq!(tree.safety.len(), tree.nodes.len());

    let rep = disklens_core::safety_stats(&tree, &rs);
    assert_eq!(rep.safe_bytes, 50);
    assert_eq!(rep.careful_bytes, 600);

    let row = |cat: &str, lvl: SafetyLevel| {
        rep.stats
            .iter()
            .find(|s| s.category == cat && s.level == lvl)
            .unwrap_or_else(|| panic!("缺 {cat}/{lvl:?}，实际 {:?}", rep.stats))
    };
    let keep = row("石墙", SafetyLevel::Keep);
    assert_eq!((keep.bytes, keep.files, keep.count), (950, 10, 1));
    // node_modules 与 foo 同组嵌套，只计最上层一次
    let nm = row("前端依赖", SafetyLevel::Careful);
    assert_eq!((nm.bytes, nm.files, nm.count), (600, 6, 1));
    let dl = row("未分类", SafetyLevel::Unknown);
    assert_eq!((dl.bytes, dl.files, dl.count), (300, 3, 1));
    let pyc = row("Python 缓存", SafetyLevel::Safe);
    assert_eq!((pyc.bytes, pyc.files, pyc.count), (50, 1, 1));
    assert_eq!(rep.stats.len(), 4);
    // bytes 降序
    assert_eq!(rep.stats[0].bytes, 950);
}

#[test]
fn scan_then_classify_fills_parallel_array() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("app").join("node_modules").join("pkg");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("index.js"), b"x").unwrap();
    let mut tree = scan(tmp.path(), &ScanOptions::default()).unwrap();
    let rs = RuleSet::default_rules(tmp.path());
    classify_tree(&mut tree, &rs);
    assert_eq!(tree.safety.len(), tree.nodes.len());
    for s in &tree.safety {
        assert!(s.rule_idx().is_none_or(|i| i < rs.len()));
    }
}

// ───────────────────────── 性能守卫 ─────────────────────────

#[test]
fn classify_and_stats_50k_nodes_not_pathologically_slow() {
    let projects = 500;
    let files_per_project = 97;
    let mut nodes = vec![dir(
        "Projects",
        NO_PARENT,
        0,
        0,
        Vec::with_capacity(projects),
    )];
    for i in 0..projects {
        let pid = nodes.len() as NodeId;
        nodes.push(dir(&format!("p{i}"), 0, 0, 0, Vec::new()));
        nodes[0].children.push(pid);
        let tid = nodes.len() as NodeId;
        nodes.push(dir("target", pid, 0, 0, vec![]));
        nodes[pid as usize].children.push(tid);
        let did = nodes.len() as NodeId;
        nodes.push(dir(
            "debug",
            tid,
            0,
            0,
            Vec::with_capacity(files_per_project),
        ));
        nodes[tid as usize].children.push(did);
        for j in 0..files_per_project {
            let fid = nodes.len() as NodeId;
            nodes.push(file(&format!("f{j}.rs"), did, 1024));
            nodes[did as usize].children.push(fid);
        }
        let bytes = files_per_project as u64 * 1024;
        for id in [did, tid, pid] {
            nodes[id as usize].file_count = files_per_project as u32;
            nodes[id as usize].size = bytes;
            nodes[id as usize].own_size = u64::from(id == did) * bytes;
        }
        nodes[0].file_count += files_per_project as u32;
        nodes[0].size += bytes;
    }
    let mut tree = tree_of(nodes, &home().join("Projects").to_string_lossy());
    assert!(tree.nodes.len() >= 50_000);

    let rs = rules();
    let started = Instant::now();
    classify_tree(&mut tree, &rs);
    let rep = disklens_core::safety_stats(&tree, &rs);
    let elapsed = started.elapsed();
    eprintln!(
        "classify+stats {} 节点耗时 {:?}（safe {} B, careful {} B）",
        tree.nodes.len(),
        elapsed,
        rep.safe_bytes,
        rep.careful_bytes
    );
    assert_eq!(
        tree.safety[tree.nodes.len() - 1].level,
        SafetyLevel::Careful,
        "debug 目录下的文件应继承 */target/debug"
    );
    assert!(rep.careful_bytes > 0, "debug 目录应被统计为可谨慎清理");
    // 墙钟时间只用来拦"病态退化"（比如 O(n²)），不能当正确性门槛：
    // 本机 121ms，GitHub ubuntu runner 就超了 200ms。精确数字见 docs/reports/T5-rules.md。
    let limit = 5000u128;
    assert!(
        elapsed.as_millis() < limit,
        "耗时 {elapsed:?} ≥ {limit}ms，疑似复杂度退化"
    );
}

#[test]
fn classify_absolute_and_tilde_paths_agree() {
    let rs = rules();
    let a = rs.classify(Path::new("~/.config/x"));
    let b = rs.classify(&home().join(".config/x"));
    assert_eq!(a, b);
    assert_eq!(a.level, SafetyLevel::Keep);
}
