//! 安全分级规则引擎：safe / careful / keep / unknown。
//!
//! 语义移植自旧 Python 版 `disk_analyzer/rules.py`：
//! 一个路径的等级 = 规则列表中**最早命中「该路径自身或其任一祖先」**的规则。
//! `exact_only` 规则只命中所指路径自身，不被后代继承（石墙）。
//! 自根向下分类时，子节点只在 `[0, 父目录的继承命中 index)` 内查找，bound 单调递减。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::model::{NodeId, NodeKind, Tree, NO_PARENT};

/// `Safety::rule_idx` 的哨兵值：没有任何规则命中。
pub const NO_RULE: u16 = u16::MAX;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SafetyLevel {
    Safe,
    Careful,
    Keep,
    #[default]
    Unknown,
}

impl SafetyLevel {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "safe" => Self::Safe,
            "careful" => Self::Careful,
            "keep" => Self::Keep,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Careful => "careful",
            Self::Keep => "keep",
            Self::Unknown => "unknown",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Safe => "🟢 安全清理",
            Self::Careful => "🟡 谨慎",
            Self::Keep => "🔴 不要动",
            Self::Unknown => "❓ 待人工判断",
        }
    }

    /// UI 排序用：safe 0, careful 1, unknown 2, keep 3（与旧版 LEVEL_ORDER 一致）。
    pub fn order(self) -> u8 {
        match self {
            Self::Safe => 0,
            Self::Careful => 1,
            Self::Unknown => 2,
            Self::Keep => 3,
        }
    }
}

impl Serialize for SafetyLevel {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SafetyLevel {
    /// 未知字符串 → unknown（用户规则写错不致命）。
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::parse(&String::deserialize(d)?))
    }
}

/// 单个节点的分类结果。4 字节，`Copy`；category/reason 等按 `rule_idx` 回查 RuleSet。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Safety {
    pub level: SafetyLevel,
    pub rule_idx: u16,
}

impl Default for Safety {
    fn default() -> Self {
        Self {
            level: SafetyLevel::Unknown,
            rule_idx: NO_RULE,
        }
    }
}

impl Safety {
    pub fn rule_idx(&self) -> Option<usize> {
        (self.rule_idx != NO_RULE).then_some(self.rule_idx as usize)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rule {
    pub pattern: String,
    #[serde(default = "unclassified")]
    pub category: String,
    #[serde(default)]
    pub level: SafetyLevel,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub needs_root: bool,
    #[serde(default)]
    pub suggest_cmd: String,
    /// 只匹配路径自身，不向后代继承
    #[serde(default)]
    pub exact_only: bool,
}

fn unclassified() -> String {
    "未分类".into()
}

fn r(pattern: &str, category: &str, level: SafetyLevel, reason: &str, method: &str) -> Rule {
    Rule {
        method: method.into(),
        ..Rule::new(pattern, category, level, reason)
    }
}

impl Rule {
    pub fn new(pattern: &str, category: &str, level: SafetyLevel, reason: &str) -> Self {
        Rule {
            pattern: pattern.into(),
            category: category.into(),
            level,
            reason: reason.into(),
            method: String::new(),
            needs_root: false,
            suggest_cmd: String::new(),
            exact_only: false,
        }
    }

    fn exact(mut self) -> Self {
        self.exact_only = true;
        self
    }

    fn root(mut self, cmd: &str) -> Self {
        self.needs_root = true;
        self.suggest_cmd = cmd.into();
        self
    }
}

/// 默认规则表：顺序即优先级。内容与旧版 `DEFAULT_RULES` 逐条对应。
pub fn default_rules() -> Vec<Rule> {
    use SafetyLevel::{Careful as C, Keep as K, Safe as S, Unknown as U};
    let mut v = vec![
        // ── 1. 系统（必须最靠前，否则会被通用缓存模式抢走）
        r("/usr", "系统", K, "系统文件", "不要动").exact(),
        r(
            "/usr/*",
            "系统",
            K,
            "系统文件（含系统 python 的 site-packages）",
            "不要动",
        ),
        r("/etc", "系统", K, "系统配置", "不要动").exact(),
        r("/etc/*", "系统", K, "系统配置", "不要动"),
        r("/boot", "系统", K, "内核与引导", "不要动").exact(),
        r("/boot/*", "系统", K, "内核与引导", "不要动"),
        r(
            "/var/lib/*",
            "系统",
            K,
            "系统数据（含 pacman 数据库）",
            "不要动",
        ),
        r("/opt", "系统", K, "/opt 下的第三方软件", "不要动").exact(),
        r("/opt/*", "系统", K, "/opt 下的第三方软件", "不要动"),
        r("/srv/*", "系统", K, "服务数据", "不要动"),
        r("/proc/*", "系统", K, "内核伪文件系统", "不要动"),
        r("/sys/*", "系统", K, "内核伪文件系统", "不要动"),
        r("/dev/*", "系统", K, "设备", "不要动"),
        r("/run/*", "系统", K, "运行时数据", "不要动"),
        // ── 2. /var 的缓存与日志（需要 sudo：给命令，不代执行）
        r(
            "/var/log/journal*",
            "系统日志",
            C,
            "systemd 日志，删除后历史日志不可查",
            "只保留最近 100MB",
        )
        .root("sudo journalctl --vacuum-size=100M"),
        r(
            "/var/log/*",
            "系统日志",
            C,
            "系统日志",
            "按需清理 30 天前的日志",
        )
        .root("sudo find /var/log -name '*.log' -mtime +30 -delete"),
        r(
            "/var/cache/pacman/pkg*",
            "包管理器缓存",
            S,
            "pacman 安装包缓存，已安装的版本不需要保留",
            "保留最近 1 个版本",
        )
        .root("sudo paccache -rk1    # 或 sudo pacman -Sc"),
        r(
            "/var/cache/*",
            "系统缓存",
            S,
            "/var 下其他缓存",
            "先看清内容再删",
        )
        .root("sudo du -sh /var/cache/*"),
        r(
            "/var/tmp",
            "临时文件",
            U,
            "跨重启保留的临时文件，可能有构建/编辑器残留",
            "人工核对后删除",
        )
        .exact(),
        r(
            "/var/tmp/*",
            "临时文件",
            U,
            "跨重启保留的临时文件，需人工判断",
            "人工核对后删除",
        ),
        // ── 3. Hermes 自身
        r(
            "~/.hermes/*/cache/scratch/*",
            "Hermes 临时/测试残渣",
            S,
            "Hermes 运行时临时目录（chromium 临时 profile、pytest 中间产物）",
            "可整目录删除",
        ),
        r(
            "~/.hermes/cache/*",
            "Hermes 缓存",
            S,
            "Hermes 缓存目录",
            "可整目录删除",
        ),
        r(
            "~/.hermes/*/sessions/*",
            "Hermes 会话",
            C,
            "历史会话记录，删除不可恢复",
            "确认后按时间清理",
        ),
        r(
            "~/.hermes/*",
            "Hermes 数据",
            K,
            "Hermes 本体/配置/状态库",
            "不要动",
        ),
        // ── 4. 应用与浏览器缓存（安全）
        r(
            "~/.cache/video-upscaler*",
            "应用缓存",
            S,
            "视频超分工具的中间/结果缓存",
            "可整目录删除",
        ),
        r(
            "~/.cache/microsoft-edge*",
            "浏览器缓存",
            S,
            "Edge 网络缓存（登录态/书签在 ~/.config，不受影响）",
            "可整目录删除",
        ),
        r(
            "~/.cache/google-chrome*",
            "浏览器缓存",
            S,
            "Chrome 网络缓存",
            "可整目录删除",
        ),
        r(
            "~/.cache/mozilla*",
            "浏览器缓存",
            S,
            "Firefox 缓存",
            "可整目录删除",
        ),
        r(
            "~/.cache/chromium*",
            "浏览器缓存",
            S,
            "Chromium 缓存",
            "可整目录删除",
        ),
        r(
            "~/.cache/epiphany*",
            "浏览器缓存",
            S,
            "GNOME Web 缓存",
            "可整目录删除",
        ),
        r(
            "~/.cache/thumbnails*",
            "缩略图缓存",
            S,
            "文件管理器缩略图，自动重建",
            "可整目录删除",
        ),
        r(
            "~/.cache/mesa_shader_cache*",
            "显卡着色器缓存",
            S,
            "OpenGL 着色器缓存，自动重建",
            "可整目录删除",
        ),
        r(
            "~/.cache/gnome-software*",
            "应用缓存",
            S,
            "GNOME 软件中心缓存",
            "可整目录删除",
        ),
        r(
            "~/.cache/fontconfig*",
            "字体缓存",
            S,
            "字体缓存，自动重建",
            "可整目录删除",
        ),
        // ── 5. 包管理器 / 开发工具缓存
        r(
            "~/.cache/ms-playwright*",
            "开发工具缓存",
            C,
            "Playwright 浏览器内核，删除后需重新下载数百 MB",
            "需要时再删",
        ),
        r(
            "~/.cache/electron*",
            "开发工具缓存",
            C,
            "Electron 二进制缓存",
            "需重新下载",
        ),
        r(
            "~/.cache/node-gyp*",
            "开发工具缓存",
            C,
            "node-gyp 编译缓存",
            "需重新编译",
        ),
        r("~/.npm", "包管理器缓存", S, "npm 下载缓存", "可整目录删除"),
        r(
            "~/.npm/*",
            "包管理器缓存",
            S,
            "npm 下载缓存",
            "npm cache clean --force",
        ),
        r(
            "~/.cache/uv/*",
            "包管理器缓存",
            S,
            "uv 下载缓存",
            "可整目录删除",
        ),
        r(
            "~/.cache/pip/*",
            "包管理器缓存",
            S,
            "pip 下载缓存",
            "可整目录删除",
        ),
        r(
            "~/.cargo/registry/*",
            "包管理器缓存",
            C,
            "Rust crate 缓存，删除后需重新下载",
            "按需清理",
        ),
        r(
            "~/.cache/huggingface/*",
            "模型缓存",
            C,
            "下载的模型权重，删了需重新下载",
            "确认不再用再删",
        ),
        r(
            "~/go/pkg/mod/*",
            "包管理器缓存",
            C,
            "Go 模块缓存",
            "按需清理",
        ),
        // ── 6. 构建产物 / 依赖
        r(
            "*/__pycache__",
            "Python 缓存",
            S,
            "字节码缓存，自动重新生成",
            "可批量删除",
        ),
        r(
            "*/.pytest_cache",
            "Python 缓存",
            S,
            "pytest 缓存",
            "可批量删除",
        ),
        r("*/.mypy_cache", "Python 缓存", S, "mypy 缓存", "可批量删除"),
        r("*/.ruff_cache", "Python 缓存", S, "ruff 缓存", "可批量删除"),
        r(
            "*/node_modules",
            "前端依赖",
            C,
            "npm 依赖，删除后需重新 npm install",
            "确认可重装后再删",
        ),
        r(
            "*/site-packages",
            "Python 虚拟环境",
            C,
            "虚拟环境依赖，应删整个 venv 而非此目录",
            "确认不用后再删",
        ),
        r(
            "*/.venv",
            "Python 虚拟环境",
            C,
            "项目虚拟环境",
            "确认不用后再删",
        ),
        r("*/dist", "构建产物", C, "构建输出，可重新构建", "按需清理"),
        r("*/build", "构建产物", C, "构建输出，可重新构建", "按需清理"),
        r(
            "*/target/debug",
            "构建产物",
            C,
            "Rust 调试产物（cargo clean 更安全）",
            "cargo clean",
        ),
        // ── 7. 回收站与临时
        r(
            "~/.local/share/Trash/*",
            "回收站",
            S,
            "已删除文件暂存区",
            "清空回收站",
        ),
        r(
            "/tmp/*",
            "临时文件",
            S,
            "系统临时文件（本机 /tmp 为内存盘，不占磁盘）",
            "重启即清",
        ),
        // ── 8. .cache 兜底（任何未列出的应用缓存）
        r(
            "~/.cache/*",
            "应用缓存",
            S,
            "通用应用缓存，删除后自动重建",
            "可整目录删除",
        ),
        // ── 9. 用户数据 / 游戏（绝对不要动）
        r(
            "~/.local/share/Steam/steamapps/common/*",
            "游戏数据",
            K,
            "已安装游戏，删除需重新下载数十 GB",
            "如需空间请整体迁移到其他磁盘",
        ),
        r(
            "~/.local/share/Steam/steamapps/workshop/*",
            "游戏数据",
            K,
            "创意工坊内容",
            "不要动",
        ),
        r(
            "~/.local/share/Steam/*",
            "游戏数据",
            K,
            "Steam 库",
            "不要动",
        ),
        r("~/Projects", "用户代码", K, "项目源码", "不要动"),
        r(
            "~/Projects/*",
            "用户代码",
            K,
            "项目源码（构建产物另按规则处理）",
            "不要动",
        ),
        r("~/Documents*", "用户文档", K, "文档", "不要动"),
        r("~/Pictures*", "用户图片", K, "图片", "不要动"),
        r("~/Videos*", "用户视频", K, "视频", "不要动"),
        r("~/Music*", "用户音频", K, "音乐", "不要动"),
        r("~/Desktop*", "桌面", K, "桌面文件", "不要动"),
        r(
            "~/Downloads/*",
            "下载",
            U,
            "下载目录内容需人工判断",
            "导出明细后人工核对",
        ),
        r("~/.config/*", "配置", K, "程序配置（含登录态）", "不要动"),
        r("~/.ssh/*", "密钥", K, "SSH 密钥", "不要动"),
        r("~/.gnupg/*", "密钥", K, "GPG 密钥", "不要动"),
        r("~/.local/share/keyrings/*", "密钥", K, "密钥环", "不要动"),
        r("~/.local/share/*", "用户数据", K, "本地用户数据", "不要动"),
        r("~/.*", "用户数据", K, "主目录下的隐藏目录", "不要动"),
        // ── 10. 石墙（exact_only：只保护自身，绝不被后代继承）
        r("~", "石墙", K, "石墙：用户主目录", "不要动").exact(),
        r("/home", "石墙", K, "石墙：主目录根", "不要动").exact(),
        r("/", "石墙", K, "石墙：根目录", "不要动").exact(),
    ];

    if cfg!(windows) {
        // 放在系统段：先于任何通用缓存规则
        v.splice(
            0..0,
            [
                r(r"C:\Windows", "系统", K, "Windows 系统目录", "不要动").exact(),
                r(r"C:\Windows\*", "系统", K, "Windows 系统目录", "不要动"),
                r(r"C:\Program Files*", "系统", K, "已安装程序", "不要动"),
                r(r"C:\ProgramData\*", "系统", K, "系统级程序数据", "不要动"),
                r(
                    r"%LOCALAPPDATA%\Temp\*",
                    "临时文件",
                    S,
                    "用户临时文件，自动清理",
                    "可整目录删除",
                ),
                r(
                    r"%LOCALAPPDATA%\Microsoft\Windows\INetCache\*",
                    "浏览器缓存",
                    S,
                    "IE/Edge 网络缓存",
                    "可整目录删除",
                ),
            ],
        );
    }
    if cfg!(target_os = "macos") {
        v.splice(
            0..0,
            [
                r("/System", "系统", K, "macOS 系统目录", "不要动").exact(),
                r("/System/*", "系统", K, "macOS 系统目录", "不要动"),
                r("/Library/*", "系统", K, "系统级资源", "不要动"),
            ],
        );
        // macOS 缓存/日志在 ~/Library，插在 ~/.cache 兜底之前
        let at = v
            .iter()
            .position(|x| x.pattern == "~/.cache/*")
            .unwrap_or(v.len());
        v.splice(
            at..at,
            [
                r(
                    "~/Library/Caches/*",
                    "应用缓存",
                    S,
                    "macOS 应用缓存，自动重建",
                    "可整目录删除",
                ),
                r(
                    "~/Library/Logs/*",
                    "系统日志",
                    S,
                    "macOS 应用日志",
                    "可整目录删除",
                ),
                r(
                    "~/Library/Developer/Xcode/DerivedData/*",
                    "构建产物",
                    C,
                    "Xcode 构建缓存",
                    "可重新构建",
                ),
                r(
                    "*.dSYM",
                    "构建产物",
                    C,
                    "调试符号包，可重新生成",
                    "按需清理",
                ),
            ],
        );
    }
    v
}

/// 用户规则文件：`dirs::config_dir()/disklens/rules.json`。
pub fn default_rules_path() -> PathBuf {
    dirs::config_dir()
        .map(|d| d.join("disklens").join("rules.json"))
        .unwrap_or_else(|| PathBuf::from("rules.json"))
}

#[derive(Serialize, Deserialize)]
struct RulesFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    rules: Vec<Rule>,
}

pub struct RuleSet {
    rules: Vec<Rule>,
    /// 全部规则，匹配完整路径
    full: GlobSet,
    /// 只含无 `/` 的模式，匹配 basename（旧版两种都试）
    base: GlobSet,
    base_map: Vec<u16>,
    home: PathBuf,
}

impl RuleSet {
    pub fn new(rules: Vec<Rule>, home: &Path) -> Self {
        let patterns: Vec<String> = rules
            .iter()
            .map(|x| expand_pattern(&x.pattern, home))
            .collect();
        let full = compile(patterns.iter().map(String::as_str));
        let mut base_map = Vec::new();
        for (i, p) in patterns.iter().enumerate() {
            if !p.contains('/') {
                base_map.push(i as u16);
            }
        }
        let base = compile(base_map.iter().map(|&i| patterns[i as usize].as_str()));
        Self {
            rules,
            full,
            base,
            base_map,
            home: home.to_path_buf(),
        }
    }

    pub fn default_rules(home: &Path) -> Self {
        Self::new(default_rules(), home)
    }

    /// 读用户规则；不存在则写出默认规则。损坏/空文件回退默认规则，绝不 panic。
    pub fn load_or_create(path: &Path, home: &Path) -> Self {
        if !path.exists() {
            let rs = Self::default_rules(home);
            let _ = rs.save(path);
            return rs;
        }
        let parsed = fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<RulesFile>(&s).ok())
            .filter(|f| !f.rules.is_empty());
        match parsed {
            Some(f) => Self::new(f.rules, home),
            None => Self::default_rules(home),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        // 手写格式化：规则文件给人看，一行一条
        let mut s = String::from("{\n \"version\": 1,\n \"rules\": [\n");
        for (i, rule) in self.rules.iter().enumerate() {
            s.push_str("  ");
            s.push_str(&serde_json::to_string(rule).map_err(io::Error::other)?);
            s.push_str(if i + 1 < self.rules.len() {
                ",\n"
            } else {
                "\n"
            });
        }
        s.push_str(" ]\n}\n");
        fs::write(path, s)
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn rule_at(&self, idx: usize) -> Option<&Rule> {
        self.rules.get(idx)
    }

    pub fn category(&self, s: Safety) -> &str {
        s.rule_idx()
            .and_then(|i| self.rules.get(i))
            .map_or("未分类", |r| r.category.as_str())
    }

    fn safety_for(&self, idx: Option<usize>) -> Safety {
        match idx.and_then(|i| self.rules.get(i).map(|r| (i, r.level))) {
            Some((i, level)) => Safety {
                level,
                rule_idx: i.min(NO_RULE as usize - 1) as u16,
            },
            None => Safety::default(),
        }
    }

    /// 在 `[0, bound)` 内匹配路径自身，返回 (首个命中, 首个可继承命中)。
    /// `bound = None` 表示没有祖先命中，搜全部规则。
    fn scan_one(&self, path: &Path, bound: Option<usize>) -> (Option<usize>, Option<usize>) {
        let lim = bound.unwrap_or(self.rules.len()).min(self.rules.len());
        let mut any: Option<usize> = None;
        let mut inh: Option<usize> = None;
        let mut add = |ri: usize| {
            if ri >= lim {
                return;
            }
            any = Some(any.map_or(ri, |f| f.min(ri)));
            if !self.rules[ri].exact_only {
                inh = Some(inh.map_or(ri, |f| f.min(ri)));
            }
        };
        for i in self.full.matches(path) {
            add(i);
        }
        if let Some(name) = path.file_name() {
            for i in self.base.matches(name) {
                add(self.base_map[i] as usize);
            }
        }
        (any, inh)
    }

    /// 完整路径（含祖先继承）的胜出规则下标。
    pub fn match_index(&self, path: &Path) -> Option<usize> {
        self.classify(path).rule_idx()
    }

    /// 单点分类：从根逐级向下收敛，与 `classify_tree` 语义一致。
    pub fn classify(&self, path: &Path) -> Safety {
        let p = expand_path(path, &self.home);
        let mut bound = None;
        let mut win = None;
        let mut cur = PathBuf::new();
        for comp in p.components() {
            cur.push(comp);
            let (any, inh) = self.scan_one(&cur, bound);
            win = any.or(bound);
            bound = inh.or(bound);
        }
        self.safety_for(win)
    }
}

fn compile<'a>(patterns: impl Iterator<Item = &'a str>) -> GlobSet {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        b.add(glob(p));
    }
    b.build().expect("globset 构建不会失败")
}

fn glob(pattern: &str) -> Glob {
    // literal_separator(false) = 旧版 fnmatch 语义：`*` 也跨越 `/`
    let build = |p: &str| {
        GlobBuilder::new(p)
            .literal_separator(false)
            .backslash_escape(!cfg!(windows))
            .build()
    };
    build(pattern)
        .or_else(|_| build(&globset::escape(pattern)))
        // 转义后仍失败（Windows 盘符等）时退化为永不匹配
        .unwrap_or_else(|_| Glob::new("__disklens_never_match__").expect("常量 glob"))
}

/// `~` 展开为 home；Windows 下展开 `%VAR%`。
fn expand_pattern(p: &str, home: &Path) -> String {
    let mut s = if p == "~" {
        home.to_string_lossy().into_owned()
    } else if let Some(rest) = p.strip_prefix("~/") {
        home.join(rest).to_string_lossy().into_owned()
    } else {
        p.to_string()
    };
    // shortcut: 相对模式（如 `*/node_modules`）保持原样按 fnmatch 匹配，
    // 不拼 home —— 否则 /tmp、外接盘上的同名目录会漏判（与旧版一致）。
    if cfg!(windows) {
        s = expand_env(&s).replace('\\', "/");
    }
    s
}

fn expand_path(path: &Path, home: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    let p = if s == "~" {
        home.to_path_buf()
    } else if let Some(rest) = s.strip_prefix("~/") {
        home.join(rest)
    } else if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|c| c.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    if cfg!(windows) {
        PathBuf::from(p.to_string_lossy().replace('\\', "/"))
    } else {
        p
    }
}

fn expand_env(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(v) => out.push_str(&v),
                    Err(_) => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// 给整棵树打上安全等级；`tree.safety` 与 `tree.nodes` 同索引。
pub fn classify_tree(tree: &mut Tree, rs: &RuleSet) {
    let mut out = vec![Safety::default(); tree.nodes.len()];
    let mut path = tree.root_path.clone();
    walk(&tree.nodes, tree.root, rs, &mut path, None, &mut out);
    tree.safety = out;
}

fn walk(
    nodes: &[crate::model::Node],
    id: NodeId,
    rs: &RuleSet,
    path: &mut PathBuf,
    bound: Option<usize>,
    out: &mut [Safety],
) {
    let (any, inh) = rs.scan_one(path, bound);
    let win = any.or(bound);
    out[id as usize] = rs.safety_for(win);
    let child_bound = inh.or(bound);
    for &c in &nodes[id as usize].children {
        path.push(&*nodes[c as usize].name);
        walk(nodes, c, rs, path, child_bound, out);
        path.pop();
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetyStat {
    pub category: String,
    pub level: SafetyLevel,
    pub bytes: u64,
    pub files: u64,
    /// 该分组内的目录数（不重复计嵌套目录）
    pub count: u32,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetyReport {
    pub stats: Vec<SafetyStat>,
    /// safe 等级可回收总字节（不重复计嵌套目录）
    pub safe_bytes: u64,
    /// careful 等级总字节
    pub careful_bytes: u64,
}

/// 按 category+level 聚合目录（只算目录；同组嵌套目录只计最上层，避免重复）。
/// 需先调用 `classify_tree`。
pub fn safety_stats(tree: &Tree, rs: &RuleSet) -> SafetyReport {
    use std::collections::HashMap;
    debug_assert_eq!(
        tree.safety.len(),
        tree.nodes.len(),
        "先调用 classify_tree 再 safety_stats"
    );
    let mut map: HashMap<(&str, SafetyLevel), (u64, u64, u32)> = HashMap::new();
    let mut by_level: HashMap<SafetyLevel, u64> = HashMap::new();
    let safety = |i: usize| tree.safety.get(i).copied().unwrap_or_default();
    for (i, node) in tree.nodes.iter().enumerate() {
        if node.kind != NodeKind::Dir {
            continue;
        }
        let s = safety(i);
        let (ps, parent_is_dir) = if node.parent == NO_PARENT {
            (Safety::default(), false)
        } else {
            (safety(node.parent as usize), true)
        };
        let cat = rs.category(s);
        if !(parent_is_dir && ps.level == s.level && rs.category(ps) == cat) {
            let e = map.entry((cat, s.level)).or_default();
            e.0 += node.size;
            e.1 += u64::from(node.file_count);
            e.2 += 1;
        }
        if !(parent_is_dir && ps.level == s.level) {
            *by_level.entry(s.level).or_default() += node.size;
        }
    }
    let mut stats: Vec<SafetyStat> = map
        .into_iter()
        .map(|((category, level), (bytes, files, count))| SafetyStat {
            category: category.to_string(),
            level,
            bytes,
            files,
            count,
        })
        .collect();
    stats.sort_by(|a, b| {
        b.bytes
            .cmp(&a.bytes)
            .then(a.level.order().cmp(&b.level.order()))
            .then(a.category.cmp(&b.category))
    });
    SafetyReport {
        stats,
        safe_bytes: by_level.get(&SafetyLevel::Safe).copied().unwrap_or(0),
        careful_bytes: by_level.get(&SafetyLevel::Careful).copied().unwrap_or(0),
    }
}
