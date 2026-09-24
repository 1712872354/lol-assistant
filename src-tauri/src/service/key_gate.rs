//! 同 key 并发合并（single-flight）：先到者回源，后到者等锁后直接命中缓存。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 按 key 串行化的门闩集合。用法：`let g = gate.key(k); let _hold = g.lock().await;` 后做「查缓存 → 回源」双重检查。
#[derive(Default)]
pub struct KeyGate {
    map: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl KeyGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// 取（或建）该 key 的门闩。key 有界（puuid×filter 组合 + 少量固定 key），不做淘汰。
    pub fn key(&self, k: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.map
            .lock()
            .unwrap()
            .entry(k.to_string())
            .or_default()
            .clone()
    }
}
