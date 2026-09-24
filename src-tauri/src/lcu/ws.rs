//! LCU WebSocket：WAMP 事件解析 + 订阅 + 选人节流 + 指数退避重连。
//! 握手对齐 Go `wss://127.0.0.1:{port}/` + Basic 头 + InsecureSkipVerify
//! （对齐 Yuumi `connect_async_tls_with_config` / Python `ssl=False`）。

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error as TlsError, SignatureScheme};
use serde_json::Value;
use tokio_tungstenite::tungstenite::client::ClientRequestBuilder;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{connect_async_tls_with_config, Connector};

use super::endpoints::{PATH_CHAMP_SELECT_SESSION, WATCHED_URIS};
use super::lockfile::auth_header;
use super::types::{Credentials, LcuEvent};

const WS_BACKOFF_INITIAL: Duration = Duration::from_secs(1);
const WS_BACKOFF_MAX: Duration = Duration::from_secs(15);
const WS_HANDSHAKE: Duration = Duration::from_secs(5);
const WS_PING_INTERVAL: Duration = Duration::from_secs(30);
const CHAMP_SELECT_THROTTLE: Duration = Duration::from_millis(500);
const TRAILING_TICK: Duration = Duration::from_millis(250);

/// 解析 WAMP `[8, "OnJsonApiEvent_x", {...}]` 帧 → LcuEvent。
pub fn parse_wamp_event(raw: &[u8]) -> Option<LcuEvent> {
    let v: Value = serde_json::from_slice(raw).ok()?;
    let arr = v.as_array()?;
    if arr.len() < 3 {
        return None;
    }
    // 操作码必须为 8（订阅回执 3 等拒绝）
    if arr[0].as_i64()? != 8 {
        return None;
    }
    let _topic = arr[1].as_str()?;
    let obj = arr[2].as_object()?;
    let uri = obj.get("uri")?.as_str()?.to_string();
    let event_type = obj.get("eventType")?.as_str()?.to_string();
    let data = obj.get("data").cloned().unwrap_or(Value::Null);
    Some(LcuEvent {
        uri,
        event_type,
        data,
    })
}

/// 订阅白名单判定（前缀匹配，对齐 Go uriWatched）。
pub fn uri_watched(uri: &str) -> bool {
    WATCHED_URIS.iter().any(|p| uri.starts_with(p))
}

/// 简单时间窗节流：窗口内后继帧缓存为 pending，窗口到期补发最新帧（trailing）。
pub struct Throttle {
    window: Duration,
    last: Option<Instant>,
    pending: Option<LcuEvent>,
}

impl Throttle {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            last: None,
            pending: None,
        }
    }

    /// 当前帧是否立即放行；不放行时调用方应 `stash`。
    pub fn allow(&mut self, now: Instant) -> bool {
        match self.last {
            None => {
                self.last = Some(now);
                true
            }
            Some(t) if now.duration_since(t) >= self.window => {
                self.last = Some(now);
                true
            }
            Some(_) => false,
        }
    }

    /// 缓存窗口内最新帧（覆盖旧 pending）。
    pub fn stash(&mut self, evt: LcuEvent) {
        self.pending = Some(evt);
    }

    /// 窗口到期则取出并放行 pending；未到期返回 None。
    pub fn take_pending(&mut self, now: Instant) -> Option<LcuEvent> {
        let last = self.last?;
        if now.duration_since(last) < self.window {
            return None;
        }
        let evt = self.pending.take()?;
        self.last = Some(now);
        Some(evt)
    }
}

/// WS URL：`wss://127.0.0.1:{port}/`（认证走 Authorization 头，对齐 Go/Yuumi）。
pub fn ws_url(creds: &Credentials) -> String {
    format!("wss://127.0.0.1:{}/", creds.port)
}

/// 跳过证书校验（LCU 自签名；等价 Go InsecureSkipVerify / Python ssl=False）。
#[derive(Debug)]
struct NoVerifier;

impl ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn dangerous_tls_config() -> Arc<ClientConfig> {
    static CFG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    CFG.get_or_init(|| {
        Arc::new(
            ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(NoVerifier))
                .with_no_client_auth(),
        )
    })
    .clone()
}

fn is_champ_select(uri: &str) -> bool {
    uri == PATH_CHAMP_SELECT_SESSION || uri.starts_with(PATH_CHAMP_SELECT_SESSION)
}

/// 单次握手+读循环：正常结束返回 Ok，网络/握手失败返回 Err。
async fn session_once<F>(
    creds: &Credentials,
    on_event: &mut F,
) -> Result<(), crate::error::AppError>
where
    F: FnMut(LcuEvent),
{
    let connector = Connector::Rustls(dangerous_tls_config());
    let url: http::Uri = ws_url(creds).parse().map_err(|e| format!("ws url: {e}"))?;
    let request = ClientRequestBuilder::new(url)
        .with_header("Authorization", auth_header(&creds.token))
        .with_header("Content-Type", "application/json")
        .with_header("Accept", "application/json");

    let (ws, _) = tokio::time::timeout(
        WS_HANDSHAKE,
        connect_async_tls_with_config(request, None, false, Some(connector)),
    )
    .await
    .map_err(|_| "ws connect timeout".to_string())?
    .map_err(|e| format!("ws connect: {e}"))?;

    log::info!("[ws] connected port={}", creds.port);
    let (mut write, mut read) = ws.split();

    let sub = serde_json::json!([5, "OnJsonApiEvent"]).to_string();
    write
        .send(Message::Text(sub))
        .await
        .map_err(|e| format!("ws subscribe: {e}"))?;
    log::info!("[ws] subscribed OnJsonApiEvent");

    let mut throttle = Throttle::new(CHAMP_SELECT_THROTTLE);
    let mut ping = tokio::time::interval(WS_PING_INTERVAL);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut trailing = tokio::time::interval(TRAILING_TICK);
    trailing.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // 跳过首个立即 tick，避免一进循环就 ping/补发
    ping.tick().await;
    trailing.tick().await;

    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Some(evt) = parse_wamp_event(text.as_bytes()) {
                            if !uri_watched(&evt.uri) {
                                continue;
                            }
                            if is_champ_select(&evt.uri) {
                                if throttle.allow(Instant::now()) {
                                    on_event(evt);
                                } else {
                                    throttle.stash(evt);
                                }
                            } else {
                                on_event(evt);
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        log::info!("[ws] close frame");
                        return Ok(());
                    }
                    Some(Err(e)) => {
                        return Err(crate::error::AppError::Http(format!("ws read: {e}")))
                    }
                    None => {
                        log::info!("[ws] stream ended");
                        return Ok(());
                    }
                    _ => {}
                }
            }
            _ = ping.tick() => {
                if let Err(e) = write.send(Message::Ping(Vec::new())).await {
                    return Err(crate::error::AppError::Http(format!("ws ping: {e}")));
                }
            }
            _ = trailing.tick() => {
                if let Some(evt) = throttle.take_pending(Instant::now()) {
                    on_event(evt);
                }
            }
        }
    }
}

/// 连接循环：指数退避重连（1s→15s），直到进程 abort（monitor 换代/disconnect）。
pub async fn connect_loop<F>(
    creds: Credentials,
    mut on_event: F,
) -> Result<(), crate::error::AppError>
where
    F: FnMut(LcuEvent),
{
    let mut backoff = WS_BACKOFF_INITIAL;

    loop {
        match session_once(&creds, &mut on_event).await {
            Ok(_) => {
                log::warn!("[ws] session ended, retry in {backoff:?}");
            }
            Err(e) => {
                log::warn!("[ws] {e}, retry in {backoff:?}");
            }
        }
        tokio::time::sleep(backoff).await;
        if backoff < WS_BACKOFF_MAX {
            backoff = (backoff * 2).min(WS_BACKOFF_MAX);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn parse_wamp_valid() {
        let frame = br#"[8,"OnJsonApiEvent_lol-gameflow-v1-gameflow-phase",{"uri":"/lol-gameflow/v1/gameflow-phase","eventType":"Update","data":"InProgress"}]"#;
        let evt = parse_wamp_event(frame).unwrap();
        assert_eq!(evt.uri, "/lol-gameflow/v1/gameflow-phase");
        assert_eq!(evt.event_type, "Update");
        assert_eq!(evt.data, Value::String("InProgress".into()));
    }

    #[test]
    fn parse_wamp_object_data() {
        let frame = br#"[8,"OnJsonApiEvent_x",{"uri":"/lol-champ-select/v1/session","eventType":"Update","data":{"localPlayerCellId":3}}]"#;
        let evt = parse_wamp_event(frame).unwrap();
        assert_eq!(evt.data["localPlayerCellId"], 3);
    }

    #[test]
    fn parse_wamp_invalid() {
        for c in [
            &b""[..],
            &b"not-json"[..],
            &b"[3,{},1]"[..],
            &b"[8,\"OnJsonApiEvent\"]"[..],
            &b"[8,\"x\",\"not-object\"]"[..],
            &b"[8,\"x\",{\"eventType\":\"Update\"}]"[..],
        ] {
            assert!(parse_wamp_event(c).is_none(), "should reject {:?}", c);
        }
    }

    #[test]
    fn uri_watched_prefix() {
        assert!(uri_watched("/lol-gameflow/v1/gameflow-phase"));
        assert!(uri_watched("/lol-gameflow/v1/gameflow-phase?x=1"));
        assert!(uri_watched("/lol-champ-select/v1/session"));
        assert!(uri_watched("/lol-champ-select/v1/session/ban"));
        assert!(!uri_watched("/lol-chat/v1/me"));
        assert!(!uri_watched("/lol-honor-v2/v1/ballot"));
        assert!(!uri_watched("/lol-gameflow-malicious"));
    }

    #[test]
    fn ws_url_is_wss_with_slash() {
        let c = Credentials {
            pid: 1,
            port: 63443,
            token: "t".into(),
            platform_id: "GZ100".into(),
        };
        assert_eq!(ws_url(&c), "wss://127.0.0.1:63443/");
    }

    #[test]
    fn throttle_window_and_trailing() {
        let mut th = Throttle::new(Duration::from_millis(500));
        let t0 = Instant::now();
        assert!(th.allow(t0), "first frame must pass");
        assert!(
            !th.allow(t0 + Duration::from_millis(200)),
            "within window must drop"
        );
        let e1 = LcuEvent {
            uri: PATH_CHAMP_SELECT_SESSION.into(),
            event_type: "Update".into(),
            data: Value::from(1),
        };
        let e2 = LcuEvent {
            uri: PATH_CHAMP_SELECT_SESSION.into(),
            event_type: "Update".into(),
            data: Value::from(2),
        };
        th.stash(e1);
        th.stash(e2);
        assert!(th.take_pending(t0 + Duration::from_millis(200)).is_none());
        let pending = th
            .take_pending(t0 + Duration::from_millis(600))
            .expect("trailing frame after window");
        assert_eq!(pending.data, Value::from(2));
        // trailing 补发会重置窗口，紧随其后的帧仍应被节流
        assert!(!th.allow(t0 + Duration::from_millis(600)));
        assert!(th.allow(t0 + Duration::from_millis(1100)));
    }
}
