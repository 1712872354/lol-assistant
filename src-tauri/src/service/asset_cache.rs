//! 资源字节与图标索引的内存缓存（history 服务内部）。
//! 字节缓存按条数封顶整体清空；索引缓存按 TTL 过期。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const INDEX_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_BYTE_ENTRIES: usize = 1024;

struct AssetCacheInner {
    bytes: HashMap<String, (String, String)>,
    indexes: HashMap<String, (Instant, HashMap<i32, String>)>,
}

#[derive(Clone)]
pub struct AssetCache {
    inner: Arc<Mutex<AssetCacheInner>>,
}

impl Default for AssetCache {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(AssetCacheInner {
                bytes: HashMap::new(),
                indexes: HashMap::new(),
            })),
        }
    }

    pub fn get_bytes(&self, key: &str) -> Option<(String, String)> {
        self.inner.lock().unwrap().bytes.get(key).cloned()
    }

    pub fn put_bytes(&self, key: &str, mime: String, data: String) {
        let mut g = self.inner.lock().unwrap();
        if g.bytes.len() >= MAX_BYTE_ENTRIES {
            g.bytes.clear();
        }
        g.bytes.insert(key.to_string(), (mime, data));
    }

    pub fn get_index(&self, path: &str) -> Option<HashMap<i32, String>> {
        let g = self.inner.lock().unwrap();
        g.indexes
            .get(path)
            .filter(|(at, _)| at.elapsed() < INDEX_TTL)
            .map(|(_, m)| m.clone())
    }

    pub fn put_index(&self, path: &str, paths: HashMap<i32, String>) {
        self.inner
            .lock()
            .unwrap()
            .indexes
            .insert(path.to_string(), (Instant::now(), paths));
    }
}
