//! 连接监控状态机（对齐 Go internal/lcu/monitor.go）。
//! 节奏：检测中 2s / 稳定 5s / 探测间隔 2s；连续 3 轮 miss 才断开。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::Mutex;
use tokio::task::AbortHandle;

use super::client::Client;
use super::endpoints::{PATH_BUILD_INFO, PATH_CURRENT_SUMMONER};
use super::types::{ConnStatus, Credentials, LcuEvent, State};

pub const POLL_INTERVAL: Duration = Duration::from_secs(2);
pub const POLL_INTERVAL_STABLE: Duration = Duration::from_secs(5);
pub const MISS_LIMIT: usize = 3;

/// 检测 / 探测钩子（注入以便测试）。detect 接收用户配置的 client_path。
pub type DetectFn = Arc<dyn Fn(&str) -> Option<Credentials> + Send + Sync>;
pub type ProbeFn = Arc<
    dyn Fn(
            Credentials,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<ConnStatus, crate::error::AppError>> + Send,
            >,
        > + Send
        + Sync,
>;

#[derive(Clone)]
pub struct Monitor {
    inner: Arc<Mutex<MonitorInner>>,
    client_path: Arc<Mutex<String>>,
    detect: DetectFn,
    probe: ProbeFn,
    /// 会话代次：凭据换代时递增，旧 WS 任务据此自杀
    session: Arc<AtomicU64>,
    /// 轮询循环句柄（stop 时终止）
    poll_abort: Arc<std::sync::Mutex<Option<AbortHandle>>>,
}

struct MonitorInner {
    status: ConnStatus,
    creds: Option<Credentials>,
    client: Option<Client>,
    miss: usize,
    ws_abort: Option<AbortHandle>,
}

impl Monitor {
    pub fn new() -> Self {
        Self::with_hooks(
            Arc::new(super::cmdline::detect_credentials),
            Arc::new(|c| Box::pin(probe_and_build(c))),
        )
    }

    pub fn with_hooks(detect: DetectFn, probe: ProbeFn) -> Self {
        Self {
            inner: Arc::new(Mutex::new(MonitorInner {
                status: ConnStatus::default(),
                creds: None,
                client: None,
                miss: 0,
                ws_abort: None,
            })),
            client_path: Arc::new(Mutex::new(String::new())),
            detect,
            probe,
            session: Arc::new(AtomicU64::new(0)),
            poll_abort: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// 设置用户配置的客户端目录（lockfile 通道候选，设置变更时同步）。
    pub async fn set_client_path(&self, p: &str) {
        *self.client_path.lock().await = p.to_string();
    }

    pub async fn status(&self) -> ConnStatus {
        self.inner.lock().await.status.clone()
    }

    pub async fn client(&self) -> Option<Client> {
        self.inner.lock().await.client.clone()
    }

    pub async fn credentials(&self) -> Option<Credentials> {
        self.inner.lock().await.creds.clone()
    }

    fn mark_disconnected(inner: &mut MonitorInner) -> ConnStatus {
        if let Some(h) = inner.ws_abort.take() {
            h.abort();
        }
        inner.status = ConnStatus::default();
        inner.creds = None;
        inner.client = None;
        inner.miss = 0;
        inner.status.clone()
    }

    /// 提交已连接状态并拉起 WS 订阅（凭据换代时取消旧循环）。
    async fn commit_connected(
        &self,
        inner: &mut MonitorInner,
        c: Credentials,
        st: ConnStatus,
        on_event: &Option<Arc<dyn Fn(LcuEvent) + Send + Sync>>,
    ) -> bool {
        let changed = inner.status != st;
        inner.status = st;
        inner.creds = Some(c.clone());
        inner.client = Some(Client::new(&c));
        inner.miss = 0;
        // 旧 WS 先取消，再按新凭据拉起
        if let Some(h) = inner.ws_abort.take() {
            h.abort();
        }
        if let Some(cb) = on_event {
            let sid = self.session.fetch_add(1, Ordering::SeqCst) + 1;
            let expect = self.session.load(Ordering::SeqCst);
            let alive = self.session.clone();
            let cb = cb.clone();
            let handle = tokio::spawn(async move {
                let _ = super::ws::connect_loop(c, move |evt| {
                    if alive.load(Ordering::SeqCst) == expect && sid == expect {
                        cb(evt);
                    }
                })
                .await;
            });
            // connect_loop 返回 Err/结束时不清 abort（由下轮 tick 或 disconnect 清理）
            inner.ws_abort = Some(handle.abort_handle());
            let _ = sid;
        }
        changed
    }

    /// 单轮 tick：检测 → 探测 → 提交/断开。返回下一轮间隔。
    ///
    /// 并发契约（C1）：`inner` 临界区只做计数/比较/克隆等 CPU 操作，
    /// **绝不跨任何 I/O**——探测（probe）在锁外执行，避免拖死 status()/client()/stop()。
    pub async fn tick<F>(
        &self,
        on_change: Option<F>,
        on_event: Option<Arc<dyn Fn(LcuEvent) + Send + Sync>>,
    ) -> Duration
    where
        F: FnOnce(ConnStatus),
    {
        let client_path = self.client_path.lock().await.clone();
        let detect_started = std::time::Instant::now();
        let creds = (self.detect)(&client_path);
        let detect_ms = detect_started.elapsed().as_millis();
        if creds.is_none() {
            log::debug!(
                "[lcu] detect miss client_path={:?} cost={}ms",
                client_path,
                detect_ms
            );
        } else if detect_ms > 500 {
            log::warn!("[lcu] detect slow cost={}ms", detect_ms);
        }

        // ── 阶段一：短锁决策，随即释放 ──
        enum Next {
            /// 连续 miss 达阈值：需触发断开回调
            Disconnect(ConnStatus),
            /// 无需探测的间隔（未检测到进程 / 稳定态降频）
            Interval(Duration),
            /// 需要探测：recheck=true 为登录等待期复查（仅升级 Connected 才提交，失败不计 miss）
            Probe { c: Credentials, recheck: bool },
        }

        let next = {
            let mut inner = self.inner.lock().await;
            match creds {
                // 未检测到进程
                None => {
                    inner.miss += 1;
                    if inner.miss >= MISS_LIMIT && inner.status.state != State::Disconnected {
                        Next::Disconnect(Self::mark_disconnected(&mut inner))
                    } else if inner.status.state == State::Connected {
                        Next::Interval(POLL_INTERVAL_STABLE)
                    } else {
                        Next::Interval(POLL_INTERVAL)
                    }
                }
                Some(c) => {
                    // 已连接且凭据未变 → 稳定态降频，不重复探测
                    let same = inner.creds.as_ref().is_some_and(|prev| prev.equal(&c));
                    if same && inner.status.state == State::Connected {
                        inner.miss = 0;
                        Next::Interval(POLL_INTERVAL_STABLE)
                    } else if same && inner.status.state == State::Unauthenticated {
                        // 登录等待期：轻量复查（对齐 Go fetchSummonerStatus 路径）
                        Next::Probe { c, recheck: true }
                    } else {
                        // 凭据变化或首次发现 → 探测（失败不提交半开状态）
                        Next::Probe { c, recheck: false }
                    }
                }
            }
        };
        // 锁已释放

        // ── 阶段二：无锁探测 + 短锁提交 ──
        match next {
            Next::Disconnect(st) => {
                if let Some(cb) = on_change {
                    cb(st);
                }
                POLL_INTERVAL
            }
            Next::Interval(iv) => iv,
            Next::Probe { c, recheck } => {
                self.probe_and_commit(c, recheck, &on_event, on_change)
                    .await
            }
        }
    }

    /// 探测（**不持 inner 锁**）→ 短锁提交/计 miss。返回下一轮间隔。
    async fn probe_and_commit<F>(
        &self,
        c: Credentials,
        recheck: bool,
        on_event: &Option<Arc<dyn Fn(LcuEvent) + Send + Sync>>,
        on_change: Option<F>,
    ) -> Duration
    where
        F: FnOnce(ConnStatus),
    {
        match (self.probe)(c.clone()).await {
            // 登录等待期复查未升级：不提交
            Ok(st) if recheck && st.state != State::Connected => POLL_INTERVAL,
            Ok(st) => {
                let mut inner = self.inner.lock().await;
                let changed = self.commit_connected(&mut inner, c, st, on_event).await;
                let committed = inner.status.clone();
                drop(inner);
                if changed {
                    if let Some(cb) = on_change {
                        cb(committed);
                    }
                }
                // 对齐 Go：提交后仍返回 2s，下轮才进入稳定 5s
                POLL_INTERVAL
            }
            Err(e) => {
                log::debug!("[lcu] probe failed: {e}");
                // 登录等待期复查失败：不计 miss
                if recheck {
                    return POLL_INTERVAL;
                }
                // 探测失败不提交半开状态；连续 miss 才断开
                let mut inner = self.inner.lock().await;
                inner.miss += 1;
                if inner.miss >= MISS_LIMIT && inner.status.state != State::Disconnected {
                    let st = Self::mark_disconnected(&mut inner);
                    drop(inner);
                    if let Some(cb) = on_change {
                        cb(st);
                    }
                }
                POLL_INTERVAL
            }
        }
    }

    /// 启动后台轮询循环（连接状态 + LCU WS 事件双回调）。
    pub fn spawn<C, E>(self, on_change: C, on_event: E)
    where
        C: Fn(ConnStatus) + Send + Sync + 'static,
        E: Fn(LcuEvent) + Send + Sync + 'static,
    {
        let on_change = Arc::new(on_change);
        let on_event: Arc<dyn Fn(LcuEvent) + Send + Sync> = Arc::new(on_event);
        let poll_abort = self.poll_abort.clone();
        let handle = tokio::spawn(async move {
            loop {
                let oc = on_change.clone();
                let oe = on_event.clone();
                let interval = self
                    .tick(Some(move |st: ConnStatus| oc(st)), Some(oe))
                    .await;
                tokio::time::sleep(interval).await;
            }
        });
        *poll_abort.lock().unwrap() = Some(handle.abort_handle());
    }

    /// 终止轮询循环与 WS 订阅，并使旧会话回调失效（幂等；进程退出路径调用）。
    pub async fn stop(&self) {
        self.session.fetch_add(1, Ordering::SeqCst);
        if let Some(h) = self.poll_abort.lock().unwrap().take() {
            h.abort();
        }
        let mut inner = self.inner.lock().await;
        if let Some(h) = inner.ws_abort.take() {
            h.abort();
        }
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

/// 真实探测链路（对齐 Go probeAndBuild + fetchSummonerStatus）：
/// 1. GET /system/v1/builds ≤5 次（600ms 间隔）判定 LCU 就绪；
/// 2. 就绪后 GET current-summoner：200+puuid→connected / 404→unauthenticated。
pub async fn probe_and_build(creds: Credentials) -> Result<ConnStatus, crate::error::AppError> {
    use crate::error::AppError;
    let client = Client::new(&creds);
    let mut last_err = String::new();
    for attempt in 1..=5 {
        match client.get(PATH_BUILD_INFO).await {
            Ok(_) => return fetch_summoner_status(&client, &creds).await,
            Err(e) => {
                last_err = e.to_string();
                log::debug!("[lcu] readiness probe {attempt}/5: {last_err}");
            }
        }
        if attempt < 5 {
            tokio::time::sleep(Duration::from_millis(600)).await;
        }
    }
    Err(AppError::Http(format!(
        "lcu http not ready after 5 probes: {last_err}"
    )))
}

/// 拉取 current-summoner 并映射登录态。
async fn fetch_summoner_status(
    client: &Client,
    creds: &Credentials,
) -> Result<ConnStatus, crate::error::AppError> {
    use crate::error::AppError;
    match client.get(PATH_CURRENT_SUMMONER).await {
        Ok(v) => {
            if let Some(st) = parse_current_summoner_value(&v) {
                let mut st = st;
                st.platform_id = Some(creds.platform_id.clone());
                Ok(st)
            } else {
                Ok(ConnStatus {
                    state: State::Unauthenticated,
                    platform_id: Some(creds.platform_id.clone()),
                    ..Default::default()
                })
            }
        }
        // 404（NotFound 变体）→ 未登录；按变体匹配，不再字符串嗅探
        Err(AppError::NotFound(_)) => Ok(ConnStatus {
            state: State::Unauthenticated,
            platform_id: Some(creds.platform_id.clone()),
            ..Default::default()
        }),
        Err(e) => Err(AppError::Http(format!("current-summoner: {e}"))),
    }
}

fn apply_summoner(st: &mut ConnStatus, v: &Value) {
    if let Some(s) = v["gameName"].as_str() {
        st.game_name = Some(s.to_string());
    }
    if let Some(s) = v["tagLine"].as_str() {
        st.tag_line = Some(s.to_string());
    }
    if let Some(s) = v["displayName"].as_str() {
        if st.game_name.is_none() {
            st.game_name = Some(s.to_string());
        }
    }
    if let Some(n) = v["summonerLevel"].as_i64() {
        st.summoner_level = Some(n);
    }
    if let Some(n) = v["profileIconId"].as_i64() {
        st.profile_icon_id = Some(n);
    }
    if let Some(s) = v["puuid"].as_str() {
        st.puuid = Some(s.to_string());
    }
}

fn parse_current_summoner_value(v: &Value) -> Option<ConnStatus> {
    let puuid = v["puuid"].as_str()?;
    let mut st = ConnStatus {
        state: State::Connected,
        puuid: Some(puuid.to_string()),
        ..Default::default()
    };
    apply_summoner(&mut st, v);
    Some(st)
}

/// 解析 current-summoner JSON → ConnStatus（对齐 Go parseCurrentSummoner）。
pub fn parse_current_summoner(raw: &[u8]) -> Option<ConnStatus> {
    let v: Value = serde_json::from_slice(raw).ok()?;
    parse_current_summoner_value(&v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[test]
    fn parse_current_summoner_valid() {
        let json = concat!(
            r#"{"puuid":"pu-1","gameName":"召唤师A","hashTag":"x","tagLine":"TAG1","#,
            r#""summonerLevel":305,"profileIconId":4562}"#
        );
        let st = parse_current_summoner(json.as_bytes()).unwrap();
        assert_eq!(st.game_name.as_deref(), Some("召唤师A"));
        assert_eq!(st.tag_line.as_deref(), Some("TAG1"));
        assert_eq!(st.summoner_level, Some(305));
        assert_eq!(st.profile_icon_id, Some(4562));
        assert_eq!(st.puuid.as_deref(), Some("pu-1"));
    }

    #[test]
    fn parse_current_summoner_display_name_fallback() {
        let st =
            parse_current_summoner(r#"{"puuid":"p2","displayName":"旧名"}"#.as_bytes()).unwrap();
        assert_eq!(st.game_name.as_deref(), Some("旧名"));
    }

    #[test]
    fn parse_current_summoner_requires_puuid() {
        assert!(parse_current_summoner(r#"{"displayName":"无puuid"}"#.as_bytes()).is_none());
        assert!(parse_current_summoner(b"bad json").is_none());
    }

    #[test]
    fn credentials_equal_platform_change_ok() {
        let a = Credentials {
            pid: 1,
            port: 2,
            token: "t".into(),
            platform_id: "HN1".into(),
        };
        let b = Credentials {
            platform_id: "HN2".into(),
            ..a.clone()
        };
        assert!(a.equal(&b), "platform change should not force reconnect");
        let c = Credentials {
            port: 3,
            ..a.clone()
        };
        assert!(!a.equal(&c), "port change must be detected");
    }

    fn test_monitor(detect: DetectFn, probe: ProbeFn) -> Monitor {
        Monitor::with_hooks(detect, probe)
    }

    #[tokio::test]
    async fn monitor_connect_flow() {
        let creds = Credentials {
            pid: 111,
            port: 222,
            token: "tok".into(),
            platform_id: "HN1".into(),
        };
        let probe_calls = Arc::new(AtomicUsize::new(0));
        let pc = probe_calls.clone();
        let m = test_monitor(
            {
                let c = creds.clone();
                Arc::new(move |_| Some(c.clone()))
            },
            Arc::new(move |_| {
                let pc = pc.clone();
                Box::pin(async move {
                    pc.fetch_add(1, Ordering::SeqCst);
                    Ok(ConnStatus {
                        state: State::Connected,
                        game_name: Some("测试召唤师".into()),
                        tag_line: Some("TAG".into()),
                        puuid: Some("p".into()),
                        ..Default::default()
                    })
                })
            }),
        );

        let mut states = Vec::new();
        let interval = m
            .tick(Some(|st: ConnStatus| states.push(st.state)), None)
            .await;
        assert_eq!(interval, POLL_INTERVAL);
        let st = m.status().await;
        assert_eq!(st.state, State::Connected);
        assert_eq!(st.game_name.as_deref(), Some("测试召唤师"));
        assert!(m.client().await.is_some());
        assert_eq!(states, vec![State::Connected]);

        // 第二轮：凭据未变、已连接 → 稳定态降频，不重复探测
        let interval = m.tick::<fn(ConnStatus)>(None, None).await;
        assert_eq!(interval, POLL_INTERVAL_STABLE);
        assert_eq!(probe_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn monitor_probe_failure_not_committed() {
        let m = test_monitor(
            Arc::new(|_| {
                Some(Credentials {
                    pid: 1,
                    port: 2,
                    token: "t".into(),
                    platform_id: String::new(),
                })
            }),
            Arc::new(|_| Box::pin(async { Err("http not ready".into()) })),
        );
        let interval = m.tick::<fn(ConnStatus)>(None, None).await;
        assert_eq!(interval, POLL_INTERVAL);
        assert_eq!(m.status().await.state, State::Disconnected);
        assert!(m.client().await.is_none());
    }

    #[tokio::test]
    async fn monitor_disconnect_after_misses() {
        let found = Arc::new(AtomicBool::new(true));
        let creds = Credentials {
            pid: 1,
            port: 2,
            token: "t".into(),
            platform_id: String::new(),
        };
        let m = test_monitor(
            {
                let found = found.clone();
                let c = creds.clone();
                Arc::new(move |_| {
                    if found.load(Ordering::SeqCst) {
                        Some(c.clone())
                    } else {
                        None
                    }
                })
            },
            Arc::new(|_| {
                Box::pin(async {
                    Ok(ConnStatus {
                        state: State::Connected,
                        ..Default::default()
                    })
                })
            }),
        );

        let mut states = Vec::new();
        let _ = m
            .tick(Some(|st: ConnStatus| states.push(st.state)), None)
            .await;
        assert_eq!(m.status().await.state, State::Connected);

        found.store(false, Ordering::SeqCst);
        for i in 0..MISS_LIMIT - 1 {
            let _ = m.tick::<fn(ConnStatus)>(None, None).await;
            assert_eq!(
                m.status().await.state,
                State::Connected,
                "must stay connected before threshold, round {i}"
            );
        }
        let _ = m
            .tick(Some(|st: ConnStatus| states.push(st.state)), None)
            .await;
        assert_eq!(m.status().await.state, State::Disconnected);
        assert!(m.client().await.is_none());

        found.store(true, Ordering::SeqCst);
        let _ = m
            .tick(Some(|st: ConnStatus| states.push(st.state)), None)
            .await;
        assert_eq!(m.status().await.state, State::Connected);
        assert_eq!(
            states,
            vec![State::Connected, State::Disconnected, State::Connected]
        );
    }

    /// C1 回归：探测（probe）进行中不得持有 inner 锁，否则 status/client 全部挂起。
    #[tokio::test]
    async fn probe_does_not_hold_inner_lock() {
        let creds = Credentials {
            pid: 7,
            port: 8,
            token: "t".into(),
            platform_id: "HN1".into(),
        };
        let probing = Arc::new(AtomicBool::new(false));
        let m = test_monitor(
            {
                let c = creds.clone();
                Arc::new(move |_| Some(c.clone()))
            },
            {
                let probing = probing.clone();
                Arc::new(move |_| {
                    let probing = probing.clone();
                    Box::pin(async move {
                        probing.store(true, Ordering::SeqCst);
                        // 模拟慢探测（生产最坏约 52s：5×10s + 4×600ms）
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        Ok(ConnStatus {
                            state: State::Connected,
                            ..Default::default()
                        })
                    })
                })
            },
        );

        let m2 = m.clone();
        let tick = tokio::spawn(async move { m2.tick::<fn(ConnStatus)>(None, None).await });

        // 等 probe 真正进入慢路径
        while !probing.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        // probe 进行中：status()/client() 必须立刻返回，不得被 inner 锁拖住
        tokio::time::timeout(Duration::from_millis(100), m.status())
            .await
            .expect("status() must not block while probe is in flight");
        tokio::time::timeout(Duration::from_millis(100), m.client())
            .await
            .expect("client() must not block while probe is in flight");
        tokio::time::timeout(Duration::from_millis(100), m.credentials())
            .await
            .expect("credentials() must not block while probe is in flight");

        let interval = tick.await.unwrap();
        assert_eq!(interval, POLL_INTERVAL);
        assert_eq!(m.status().await.state, State::Connected);
    }

    #[tokio::test]
    async fn monitor_unauth_upgrade_via_hooks() {
        let logged_in = Arc::new(AtomicBool::new(false));
        let creds = Credentials {
            pid: 5,
            port: 4111,
            token: "tk".into(),
            platform_id: "HN1".into(),
        };
        let m = test_monitor(
            {
                let c = creds.clone();
                Arc::new(move |_| Some(c.clone()))
            },
            {
                let logged_in = logged_in.clone();
                Arc::new(move |_| {
                    let logged_in = logged_in.clone();
                    Box::pin(async move {
                        if logged_in.load(Ordering::SeqCst) {
                            Ok(ConnStatus {
                                state: State::Connected,
                                game_name: Some("后登录".into()),
                                ..Default::default()
                            })
                        } else {
                            Ok(ConnStatus {
                                state: State::Unauthenticated,
                                ..Default::default()
                            })
                        }
                    })
                })
            },
        );

        let mut states = Vec::new();
        let _ = m
            .tick(Some(|st: ConnStatus| states.push(st.state)), None)
            .await;
        assert_eq!(m.status().await.state, State::Unauthenticated);

        logged_in.store(true, Ordering::SeqCst);
        let _ = m
            .tick(Some(|st: ConnStatus| states.push(st.state)), None)
            .await;
        let st = m.status().await;
        assert_eq!(st.state, State::Connected);
        assert_eq!(st.game_name.as_deref(), Some("后登录"));
        assert_eq!(states, vec![State::Unauthenticated, State::Connected]);
    }
}
