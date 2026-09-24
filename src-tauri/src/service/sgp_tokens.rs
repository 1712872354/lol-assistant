//! SGP 凭据 token 缓存与拉取注入签名（history 服务内部）。
//! 双 token（league-session / entitlements）按用途规定优先级；缓存带 TTL，失败另有短 TTL。

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::service::http::BoxFut;

pub const SGP_TOKEN_TTL: Duration = Duration::from_secs(5 * 60);
pub const SGP_TOKEN_ERR_TTL: Duration = Duration::from_secs(15);

/// SGP 段位拉取注入签名（默认走 sgp::fetch_ranked_stats；测试注入假实现）。
pub type SgpRankedFetch = Arc<
    dyn Fn(
            String,
            String,
            String,
        ) -> BoxFut<'static, Result<crate::sgp::RankedStats, crate::error::AppError>>
        + Send
        + Sync,
>;
/// SGP 战绩拉取注入签名（默认走 sgp::fetch_match_history；测试注入假实现）。
pub type SgpMatchFetch = Arc<
    dyn Fn(
            String,
            String,
            String,
            i32,
            i32,
        ) -> BoxFut<'static, Result<Vec<u8>, crate::error::AppError>>
        + Send
        + Sync,
>;

pub fn default_sgp_ranked() -> SgpRankedFetch {
    Arc::new(|host, puuid, token| {
        Box::pin(async move { crate::sgp::fetch_ranked_stats(&host, &puuid, &token).await })
    })
}

pub fn default_sgp_match() -> SgpMatchFetch {
    Arc::new(|host, puuid, token, start, count| {
        Box::pin(async move {
            crate::sgp::fetch_match_history(&host, &puuid, &token, start, count).await
        })
    })
}

/// SGP 双凭据：session 与 entitlements 各一。
#[derive(Clone, Default)]
pub struct SgpTokens {
    pub session: String,
    pub entitlements: String,
}

impl SgpTokens {
    /// 段位查询优先 entitlements 之外的顺序：session 先。
    pub fn ranked(&self) -> Vec<String> {
        non_empty(&self.session, &self.entitlements)
    }
    /// 战绩查询优先 entitlements。
    pub fn matched(&self) -> Vec<String> {
        non_empty(&self.entitlements, &self.session)
    }
}

/// 非空 token 列表（保持给定优先级，去重）。
fn non_empty(a: &str, b: &str) -> Vec<String> {
    let mut out = Vec::new();
    if !a.is_empty() {
        out.push(a.to_string());
    }
    if !b.is_empty() && b != a {
        out.push(b.to_string());
    }
    out
}

/// token 缓存：TTL 内有效；拉取失败时以短 TTL 缓存空结果避免打爆。
pub struct TokenCache {
    pub toks: SgpTokens,
    pub at: Option<Instant>,
    pub ttl: Duration,
}

impl Default for TokenCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenCache {
    pub fn new() -> Self {
        Self {
            toks: SgpTokens::default(),
            at: None,
            ttl: Duration::ZERO,
        }
    }

    pub fn valid(&self) -> bool {
        self.ttl > Duration::ZERO && self.at.map(|at| at.elapsed() < self.ttl).unwrap_or(false)
    }
}
