//! 扫描会话注册表：只保留最近 3 个结果，其余释放。

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use disklens_core::{RuleSet, Tree};

const KEEP_SCANS: usize = 3;

pub struct ScanRecord {
    pub tree: RwLock<Option<Arc<Tree>>>,
    /// 分类用的规则表，与树一起落库：NodeView 回查 category/reason 需要它
    pub rules: RwLock<Option<Arc<RuleSet>>>,
    pub cancel: Arc<AtomicBool>,
    pub alive: AtomicBool,
}

impl ScanRecord {
    fn new(cancel: Arc<AtomicBool>) -> Self {
        Self {
            tree: RwLock::new(None),
            rules: RwLock::new(None),
            cancel,
            alive: AtomicBool::new(true),
        }
    }
}

#[derive(Default)]
struct Inner {
    map: HashMap<String, Arc<ScanRecord>>,
    order: VecDeque<String>,
    seq: u64,
}

#[derive(Default)]
pub struct AppState {
    inner: Mutex<Inner>,
}

impl AppState {
    /// 登记一次新扫描；超出的旧结果直接释放并标记失效。
    pub fn begin(&self, cancel: Arc<AtomicBool>) -> String {
        let mut g = self.inner.lock().expect("state poisoned");
        g.seq += 1;
        let id = format!("scan-{}", g.seq);
        g.map.insert(id.clone(), Arc::new(ScanRecord::new(cancel)));
        g.order.push_back(id.clone());
        while g.order.len() > KEEP_SCANS {
            if let Some(old) = g.order.pop_front() {
                if let Some(rec) = g.map.remove(&old) {
                    rec.alive.store(false, Ordering::Release);
                }
            }
        }
        id
    }

    pub fn get(&self, id: &str) -> Result<Arc<ScanRecord>, String> {
        let g = self.inner.lock().expect("state poisoned");
        g.map
            .get(id)
            .filter(|r| r.alive.load(Ordering::Acquire))
            .cloned()
            .ok_or_else(|| format!("扫描不存在或已释放: {id}"))
    }

    /// 取已完成的树。扫描未结束时明确报错，不返回空树。
    pub fn tree(&self, id: &str) -> Result<Arc<Tree>, String> {
        let rec = self.get(id)?;
        let tree = rec.tree.read().expect("tree poisoned").clone();
        tree.ok_or_else(|| format!("扫描尚未完成: {id}"))
    }

    pub fn rules(&self, id: &str) -> Result<Arc<RuleSet>, String> {
        let rec = self.get(id)?;
        let rules = rec.rules.read().expect("rules poisoned").clone();
        rules.ok_or_else(|| format!("扫描尚未完成: {id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with(n: usize) -> (AppState, Vec<String>) {
        let s = AppState::default();
        let ids = (0..n)
            .map(|_| s.begin(Arc::new(AtomicBool::new(false))))
            .collect();
        (s, ids)
    }

    #[test]
    fn keeps_last_three_and_invalidates_old() {
        let (s, ids) = state_with(5);
        assert!(s.get(&ids[0]).is_err(), "最早的结果应被释放");
        assert!(s.get(&ids[1]).is_err());
        for id in &ids[2..] {
            assert!(s.get(id).is_ok());
        }
    }

    #[test]
    fn unknown_id_is_an_error() {
        let (s, _) = state_with(1);
        let Err(e) = s.get("scan-999") else {
            panic!("未知 scanId 应报错");
        };
        assert!(e.contains("扫描不存在"));
        assert!(s.tree("scan-999").is_err());
    }

    #[test]
    fn tree_query_before_done_fails() {
        let (s, ids) = state_with(1);
        let Err(e) = s.tree(&ids[0]) else {
            panic!("未完成的扫描不应返回树");
        };
        assert!(e.contains("尚未完成"));
    }
}
