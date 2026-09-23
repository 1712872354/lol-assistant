//! LCU HTTP 访问抽象（服务层依赖面）。
//! 生产：`MonitorHttp`（经 Monitor 取 Client）；测试：`FakeHttp` 路由表。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::lcu::{Client, ConnStatus, Monitor, State};

pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// LCU GET + 状态快照（服务层唯一 LCU 入口）。
pub trait LcuHttp: Send + Sync + 'static {
    /// GET，返回 (status, body)；网络错误 → Err(String)。非 2xx 仍为 Ok。
    fn get<'a>(&'a self, path: &'a str) -> BoxFut<'a, Result<(u16, Vec<u8>), String>>;
    /// 连接状态快照。
    fn status(&self) -> BoxFut<'_, ConnStatus>;
}

/// 生产实现：复用 Monitor 连接的 LCU Client。
#[derive(Clone)]
pub struct MonitorHttp {
    mon: Monitor,
}

impl MonitorHttp {
    pub fn new(mon: Monitor) -> Self {
        Self { mon }
    }

    async fn client(&self) -> Result<Client, String> {
        self.mon
            .client()
            .await
            .ok_or_else(|| crate::service::ERR_NOT_CONNECTED.to_string())
    }
}

impl LcuHttp for MonitorHttp {
    fn get<'a>(&'a self, path: &'a str) -> BoxFut<'a, Result<(u16, Vec<u8>), String>> {
        Box::pin(async move {
            let cli = self.client().await?;
            cli.get_raw(path).await.map_err(|e| e.to_string())
        })
    }

    fn status(&self) -> BoxFut<'_, ConnStatus> {
        Box::pin(async move { self.mon.status().await })
    }
}

/// 可选动态路由 handler 类型（测试注入计数等）
pub type FakeHandler = Arc<dyn Fn(&str) -> Result<(u16, Vec<u8>), String> + Send + Sync>;

/// 测试实现：路径 → (status, body)；未命中返回 404。
#[derive(Default, Clone)]
pub struct FakeHttp {
    pub routes: Arc<std::collections::HashMap<String, (u16, Vec<u8>)>>,
    pub status: ConnStatus,
    pub handler: Option<FakeHandler>,
}

impl FakeHttp {
    pub fn new(status: ConnStatus) -> Self {
        Self {
            status,
            ..Default::default()
        }
    }

    pub fn with_routes(
        mut self,
        routes: std::collections::HashMap<String, (u16, Vec<u8>)>,
    ) -> Self {
        self.routes = Arc::new(routes);
        self
    }

    pub fn with_handler(
        mut self,
        h: impl Fn(&str) -> Result<(u16, Vec<u8>), String> + Send + Sync + 'static,
    ) -> Self {
        self.handler = Some(Arc::new(h));
        self
    }

    pub fn connected() -> Self {
        Self::new(ConnStatus {
            state: State::Connected,
            puuid: Some("PSELF".into()),
            game_name: Some("歪比巴卜小宝贝".into()),
            tag_line: Some("60021".into()),
            profile_icon_id: Some(42),
            summoner_level: Some(300),
            platform_id: Some("GZ100".into()),
        })
    }
}

impl LcuHttp for FakeHttp {
    fn get<'a>(&'a self, path: &'a str) -> BoxFut<'a, Result<(u16, Vec<u8>), String>> {
        Box::pin(async move {
            if let Some(h) = &self.handler {
                return h(path);
            }
            if let Some((s, b)) = self.routes.get(path) {
                return Ok((*s, b.clone()));
            }
            // 允许 path 中带 query：按 path 部分再试一次
            let base = path.split('?').next().unwrap_or(path);
            if let Some((s, b)) = self.routes.get(base) {
                return Ok((*s, b.clone()));
            }
            Ok((404, Vec::new()))
        })
    }

    fn status(&self) -> BoxFut<'_, ConnStatus> {
        let st = self.status.clone();
        Box::pin(async move { st })
    }
}

/* ── URL 编码（对齐 Go net/url PathEscape / QueryEscape 常用子集） ── */

pub fn path_escape(s: &str) -> String {
    percent_encode(s, false)
}

pub fn query_escape(s: &str) -> String {
    percent_encode(s, true)
}

fn percent_encode(s: &str, space_as_plus: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' if space_as_plus => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_escape_uuid_safe() {
        assert_eq!(
            path_escape("5e65c58d-5b4a-5936-9104-806bb8443eef"),
            "5e65c58d-5b4a-5936-9104-806bb8443eef"
        );
        assert_eq!(path_escape("a/b"), "a%2Fb");
    }

    #[test]
    fn query_escape_space_and_hash() {
        assert_eq!(
            query_escape("安静的亚索#CN1"),
            "%E5%AE%89%E9%9D%99%E7%9A%84%E4%BA%9A%E7%B4%A2%23CN1"
        );
        assert_eq!(query_escape("a b"), "a+b");
    }

    #[tokio::test]
    async fn fake_http_routes_and_404() {
        let mut routes = std::collections::HashMap::new();
        routes.insert("/ok".to_string(), (200u16, b"hi".to_vec()));
        let http = FakeHttp::new(ConnStatus::default()).with_routes(routes);
        assert_eq!(http.get("/ok").await.unwrap(), (200, b"hi".to_vec()));
        assert_eq!(http.get("/missing").await.unwrap().0, 404);
        assert_eq!(http.get("/ok?x=1").await.unwrap().0, 200, "query strip");
    }
}
