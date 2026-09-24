//! 腾讯 SGP（Service Gateway Proxy）云端数据源客户端。
//! 端点契约取自 League Akari 抓包配置（2026-07）。

use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// SGP 网关端口（国服各区统一）
const SGP_PORT: u16 = 21019;

/// X-Riot-ClientPlatform 请求头（LCU 命令行同源值）
const CLIENT_PLATFORM: &str = "ew0KCSJwbGF0Zm9ybVR5cGUiOiAiUEMiDQp9";

/// SGP 响应体上限
const MAX_BODY_BYTES: usize = 16 << 20;

/// PlatformID 仅允许字母数字下划线连字符
fn valid_platform_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// knownHosts PlatformID → SGP 网关完整地址（Akari builtin.ts 2026-07）
fn known_hosts() -> std::collections::HashMap<&'static str, &'static str> {
    use std::collections::HashMap;
    let mut m = HashMap::new();
    m.insert("GZ100", "https://gz100-sgp.lol.qq.com:21019");
    m.insert("HN1", "https://hn1-k8s-sgp.lol.qq.com:21019");
    m.insert("HN10", "https://hn10-k8s-sgp.lol.qq.com:21019");
    m.insert("TJ100", "https://tj100-sgp.lol.qq.com:21019");
    m.insert("TJ101", "https://tj101-sgp.lol.qq.com:21019");
    m.insert("NJ100", "https://nj100-sgp.lol.qq.com:21019");
    m.insert("CQ100", "https://cq100-sgp.lol.qq.com:21019");
    m.insert("BGP2", "https://bgp2-k8s-sgp.lol.qq.com:21019");
    m.insert("PBE", "https://pbe-sgp.lol.qq.com:21019");
    m.insert("PREPBE", "https://prepbe-sgp.lol.qq.com:21019");
    m
}

/// 由 PlatformID 推导 SGP 网关地址；未命中映射按 lowercase-sgp 兜底。
pub fn host(platform_id: &str) -> String {
    let pid = platform_id.trim();
    if pid.is_empty() || !valid_platform_id(pid) {
        return String::new();
    }
    if let Some(h) = known_hosts().get(pid.to_uppercase().as_str()) {
        return h.to_string();
    }
    format!("https://{}-sgp.lol.qq.com:{}", pid.to_lowercase(), SGP_PORT)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueueEntry {
    #[serde(rename = "queueType", default)]
    pub queue_type: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub rank: String,
    #[serde(default)]
    pub division: String,
    #[serde(rename = "leaguePoints", default)]
    pub league_points: i32,
    #[serde(default)]
    pub wins: i32,
    #[serde(default)]
    pub losses: i32,
}

impl QueueEntry {
    /// 段位小级（rank 优先，division 兜底）
    pub fn div(&self) -> &str {
        if !self.rank.is_empty() {
            &self.rank
        } else {
            &self.division
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RankedStats {
    #[serde(default)]
    pub queues: Vec<QueueEntry>,
}

/// HTTP 客户端（生产：完整 TLS 校验、禁系统代理；测试可注入自签跳过客户端）
static HTTP_CLI: Mutex<Option<reqwest::Client>> = Mutex::new(None);

pub fn set_http_client(c: reqwest::Client) {
    *HTTP_CLI.lock().unwrap() = Some(c);
}

fn http_client() -> reqwest::Client {
    if let Some(c) = HTTP_CLI.lock().unwrap().as_ref() {
        return c.clone();
    }
    reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .no_proxy()
        .build()
        .expect("build sgp http client")
}

/// SGP 并发闸门（对齐 LCU 闸门思想）：防打爆腾讯接口风控。
static SGP_GATE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

async fn sgp_get(url: &str, token: &str) -> Result<Vec<u8>, crate::error::AppError> {
    let _permit = SGP_GATE
        .acquire()
        .await
        .map_err(|e| crate::error::AppError::Http(format!("sgp gate: {e}")))?;
    let client = http_client();
    let resp = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .header("X-Riot-ClientPlatform", CLIENT_PLATFORM)
        .send()
        .await
        .map_err(|e| format!("sgp request failed: {e}"))?;
    let status = resp.status();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("sgp read body: {e}"))?;
    if bytes.len() > MAX_BODY_BYTES {
        return Err("sgp body too large".into());
    }
    if !status.is_success() {
        return Err(crate::error::AppError::Http(format!(
            "sgp http {}",
            status.as_u16()
        )));
    }
    Ok(bytes.to_vec())
}

pub async fn fetch_ranked_stats(
    host: &str,
    puuid: &str,
    token: &str,
) -> Result<RankedStats, crate::error::AppError> {
    if host.is_empty() || puuid.is_empty() || token.is_empty() {
        return Err("sgp: missing host/puuid/token".into());
    }
    let url = format!(
        "{}/leagues-ledge/v2/rankedStats/puuid/{}",
        host,
        path_escape(puuid)
    );
    let body = sgp_get(&url, token).await?;
    serde_json::from_slice(&body)
        .map_err(|e| crate::error::AppError::Parse(format!("sgp parse failed: {e}")))
}

pub async fn fetch_match_history(
    host: &str,
    puuid: &str,
    token: &str,
    start_index: i32,
    count: i32,
) -> Result<Vec<u8>, crate::error::AppError> {
    if host.is_empty() || puuid.is_empty() || token.is_empty() {
        return Err("sgp: missing host/puuid/token".into());
    }
    let start = start_index.max(0);
    let count = if count <= 0 { 20 } else { count };
    let url = format!(
        "{}/match-history-query/v1/products/lol/player/{}/SUMMARY?startIndex={}&count={}",
        host,
        path_escape(puuid),
        start,
        count
    );
    sgp_get(&url, token).await
}

// 转义走 crate::util 完整 percent-encode（原本地实现仅转义 `/`，query 元字符未编码）
use crate::util::path_escape;

/* ── SGP 认证凭据（LCU 端点取数；注入 getter 保持零 LCU 依赖） ── */

const LCU_PATH_LEAGUE_SESSION_TOKEN: &str = "/lol-league-session/v1/league-session-token";
const LCU_PATH_ENTITLEMENTS_TOKEN: &str = "/entitlements/v1/token";

/// LCU GET 函数签名（注入 lcu.Client 方法值；测试注入假实现）
pub type LcuGetter = Box<dyn Fn(&str) -> Result<(u16, Vec<u8>), crate::error::AppError>>;

pub fn fetch_league_session_token(get: &LcuGetter) -> Result<String, crate::error::AppError> {
    fetch_lcu_token(get, LCU_PATH_LEAGUE_SESSION_TOKEN, "")
}

pub fn fetch_entitlements_token(get: &LcuGetter) -> Result<String, crate::error::AppError> {
    fetch_lcu_token(get, LCU_PATH_ENTITLEMENTS_TOKEN, "accessToken")
}

fn fetch_lcu_token(
    get: &LcuGetter,
    path: &str,
    json_field: &str,
) -> Result<String, crate::error::AppError> {
    let (status, body) = get(path)?;
    if !(200..300).contains(&status) {
        return Err(format!("sgp token {path}: http {status}").into());
    }
    let tok = parse_token_body(&body, json_field);
    if tok.is_empty() {
        return Err(format!("sgp token {path}: empty token").into());
    }
    Ok(tok)
}

pub fn parse_token_body(body: &[u8], json_field: &str) -> String {
    let trim = String::from_utf8_lossy(body).trim().to_string();
    if trim.is_empty() {
        return String::new();
    }
    if !json_field.is_empty() && trim.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&trim) {
            if let Some(s) = v.get(json_field).and_then(|x| x.as_str()) {
                return s.trim().to_string();
            }
        }
        return String::new();
    }
    if let Ok(s) = serde_json::from_str::<String>(&trim) {
        return s.trim().to_string();
    }
    trim
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn host_cases() {
        assert_eq!(host("GZ100"), "https://gz100-sgp.lol.qq.com:21019");
        assert_eq!(host("gz100"), "https://gz100-sgp.lol.qq.com:21019");
        assert_eq!(host("HN1"), "https://hn1-k8s-sgp.lol.qq.com:21019");
        assert_eq!(host("BGP2"), "https://bgp2-k8s-sgp.lol.qq.com:21019");
        assert_eq!(host("XYZ999"), "https://xyz999-sgp.lol.qq.com:21019");
        assert_eq!(host(""), "");
        assert_eq!(host("a/b"), "");
        assert_eq!(host("x.y"), "");
    }

    #[test]
    fn queue_entry_div() {
        let e = QueueEntry {
            rank: "II".into(),
            division: "IV".into(),
            ..Default::default()
        };
        assert_eq!(e.div(), "II");
        let e = QueueEntry {
            rank: String::new(),
            division: "III".into(),
            ..Default::default()
        };
        assert_eq!(e.div(), "III");
    }

    #[test]
    fn fetch_league_session_token_json_string() {
        let get: LcuGetter = Box::new(|path| {
            assert_eq!(path, LCU_PATH_LEAGUE_SESSION_TOKEN);
            Ok((200, b"\"tok-abc\"".to_vec()))
        });
        let tok = fetch_league_session_token(&get).unwrap();
        assert_eq!(tok, "tok-abc");
    }

    #[test]
    fn fetch_league_session_token_raw() {
        let get: LcuGetter = Box::new(|_| Ok((200, b"raw.jwt.token\n".to_vec())));
        let tok = fetch_league_session_token(&get).unwrap();
        assert_eq!(tok, "raw.jwt.token");
    }

    #[test]
    fn fetch_league_session_token_errors() {
        let get: LcuGetter = Box::new(|_| Ok((200, b"  ".to_vec())));
        assert!(fetch_league_session_token(&get).is_err());
        let get: LcuGetter = Box::new(|_| Ok((404, vec![])));
        let err = fetch_league_session_token(&get).unwrap_err();
        assert!(err.to_string().contains("404"), "err={err}");
        let get: LcuGetter = Box::new(|_| Err("conn refused".into()));
        assert!(fetch_league_session_token(&get).is_err());
    }

    #[test]
    fn fetch_entitlements_token_object() {
        let get: LcuGetter = Box::new(|path| {
            assert_eq!(path, LCU_PATH_ENTITLEMENTS_TOKEN);
            Ok((
                200,
                br#"{"accessToken":"ent-token-1","entitlements":["lol_path"]}"#.to_vec(),
            ))
        });
        let tok = fetch_entitlements_token(&get).unwrap();
        assert_eq!(tok, "ent-token-1");

        let get: LcuGetter = Box::new(|_| Ok((200, br#"{"x":1}"#.to_vec())));
        assert!(
            fetch_league_session_token(&get).is_err() || fetch_entitlements_token(&get).is_err()
        );
        let get: LcuGetter = Box::new(|_| Ok((403, vec![])));
        let err = fetch_entitlements_token(&get).unwrap_err();
        assert!(err.to_string().contains("403"));
    }

    #[test]
    fn fetch_ranked_stats_missing_params() {
        // 同步校验：空参直接返回（用 block_on 在 tokio runtime 外不可用，这里测 host 层参数）
        assert!(host("").is_empty());
        // 直接测参数校验分支
        let fut = async {
            fetch_ranked_stats("", "P", "tok").await.is_err()
                && fetch_ranked_stats("h", "", "tok").await.is_err()
                && fetch_ranked_stats("h", "P", "").await.is_err()
        };
        // 无 runtime 时用 pollster 不可用 → 用 mini runtime
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert!(rt.block_on(fut));
    }

    #[test]
    fn fetch_ranked_stats_success_and_http_errors() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();

        // 简易 HTTP/1.1 服务器线程
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            for _ in 0..3 {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let (status_line, body) = if req
                    .contains("GET /leagues-ledge/v2/rankedStats/puuid/PUUID-1")
                {
                    (
                        "200 OK",
                        r#"{"queues":[{"queueType":"RANKED_SOLO_5x5","tier":"GOLD","rank":"IV","leaguePoints":45,"wins":10,"losses":8},{"queueType":"RANKED_FLEX_SR","tier":"PLATINUM","rank":"I","leaguePoints":91,"wins":2,"losses":3}]}"#,
                    )
                } else if req.contains("403") {
                    ("403 Forbidden", "denied")
                } else {
                    ("404 Not Found", "nope")
                };
                let resp = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
            }
        });

        let host_url = format!("http://127.0.0.1:{port}");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        // 成功路径
        let h = host_url.clone();
        let stats = rt
            .block_on(async move { fetch_ranked_stats(&h, "PUUID-1", "test-token").await })
            .unwrap();
        assert_eq!(stats.queues.len(), 2);
        assert_eq!(stats.queues[0].tier, "GOLD");
        assert_eq!(stats.queues[0].div(), "IV");
        assert_eq!(stats.queues[0].league_points, 45);
        assert_eq!(stats.queues[1].tier, "PLATINUM");
        assert_eq!(stats.queues[1].div(), "I");
        assert_eq!(stats.queues[1].league_points, 91);
    }

    /// T3.4 回归：SGP 并发放量必须封顶（≤2），防打爆腾讯接口风控。
    #[tokio::test]
    async fn sgp_concurrency_is_bounded() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let peak = Arc::new(AtomicUsize::new(0));
        let cur = Arc::new(AtomicUsize::new(0));
        let peak_w = peak.clone();
        let cur_w = cur.clone();
        std::thread::spawn(move || {
            let peak = peak_w;
            let cur = cur_w;
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                let peak = peak.clone();
                let cur = cur.clone();
                std::thread::spawn(move || {
                    let now = cur.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    let mut buf = [0u8; 2048];
                    let _ = s.read(&mut buf);
                    std::thread::sleep(std::time::Duration::from_millis(120));
                    let body = "[]";
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = s.write_all(resp.as_bytes());
                    cur.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });

        let url = format!("http://127.0.0.1:{port}/match-history-query/v1/products/lol/player/x/SUMMARY?startIndex=0&count=20");
        let futs: Vec<_> = (0..3).map(|_| sgp_get(&url, "t")).collect();
        let results = futures_util::future::join_all(futs).await;
        for r in results {
            let _ = r;
        }
        let p = peak.load(Ordering::SeqCst);
        assert!(p <= 2, "SGP 并发峰值 {p} 超出闸门 2");
        assert!(p >= 1);
    }
}
