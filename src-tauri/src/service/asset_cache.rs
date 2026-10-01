//! 资源字节与图标索引的缓存（内存 + 磁盘）。
//! 内存缓存按条数封顶驱逐旧条；磁盘缓存持久化，重启后免拉取。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
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
    disk_dir: Arc<Mutex<Option<PathBuf>>>,
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
            disk_dir: Arc::new(Mutex::new(None)),
        }
    }

    /// 启用磁盘缓存（在 app data 目录下创建 assets/ 子目录）。
    pub fn with_disk_cache(self, dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&dir);
        *self.disk_dir.lock().unwrap() = Some(dir);
        self
    }

    /// 运行时启用磁盘缓存（setup 阶段调用）。
    pub fn set_disk_cache(&self, dir: PathBuf) {
        let _ = fs::create_dir_all(&dir);
        *self.disk_dir.lock().unwrap() = Some(dir);
    }

    pub fn get_bytes(&self, key: &str) -> Option<(String, String)> {
        // 1. 内存命中
        {
            let g = self.inner.lock().unwrap();
            if let Some(v) = g.bytes.get(key) {
                return Some(v.clone());
            }
        }
        // 2. 磁盘命中 → 加载回内存
        let disk = self.disk_dir.lock().unwrap().clone();
        if let Some(dir) = disk {
            if let Some((mime, data)) = disk_load(&dir, key) {
                let mut g = self.inner.lock().unwrap();
                if g.bytes.len() >= MAX_BYTE_ENTRIES {
                    g.bytes.clear();
                }
                g.bytes.insert(key.to_string(), (mime.clone(), data.clone()));
                return Some((mime, data));
            }
        }
        None
    }

    pub fn put_bytes(&self, key: &str, mime: String, data: String) {
        // 写入内存
        {
            let mut g = self.inner.lock().unwrap();
            if g.bytes.len() >= MAX_BYTE_ENTRIES {
                g.bytes.clear();
            }
            g.bytes.insert(key.to_string(), (mime.clone(), data.clone()));
        }
        // 异步写入磁盘（fire-and-forget，不阻塞调用方）
        let disk = self.disk_dir.lock().unwrap().clone();
        if let Some(dir) = disk {
            let key = key.to_string();
            let mime = mime.clone();
            let data = data.clone();
            tokio::spawn(async move {
                disk_save(&dir, &key, &mime, &data);
            });
        }
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

/* ── 磁盘 I/O ── */

fn disk_path(dir: &std::path::Path, key: &str) -> PathBuf {
    // key 格式 "kind:id" → 文件名 "kind_id.dat"
    let safe = key.replace(':', "_");
    dir.join(format!("{safe}.dat"))
}

/// 磁盘文件格式：首行 MIME，其余为 base64 数据。
fn disk_load(dir: &std::path::Path, key: &str) -> Option<(String, String)> {
    let path = disk_path(dir, key);
    let content = fs::read_to_string(&path).ok()?;
    let mut lines = content.lines();
    let mime = lines.next()?.to_string();
    let data = lines.next()?.to_string();
    if mime.is_empty() || data.is_empty() {
        return None;
    }
    Some((mime, data))
}

fn disk_save(dir: &std::path::Path, key: &str, mime: &str, data: &str) {
    let path = disk_path(dir, key);
    let content = format!("{mime}\n{data}");
    let _ = fs::write(&path, content);
}