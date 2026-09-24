//! 服务内缓存状态（近况 / 英雄索引）。

use std::collections::HashMap;
use std::time::Instant;

use super::protocol::Career;

/* ── 服务 ── */

pub(super) struct ChampIndexState {
    pub(super) at: Option<Instant>,
    pub(super) map: HashMap<String, i32>,
}

pub(super) struct CareerState {
    pub(super) limit: i32,
    pub(super) cache: HashMap<String, (Instant, Career)>,
}
