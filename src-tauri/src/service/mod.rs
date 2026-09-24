//! 服务层：history（历史战绩）+ gameinfo（对局信息聚合）。
//! 依赖经 trait 注入，单元测试用 Fake 实现，不依赖真实 LCU。

pub mod asset_cache;
pub mod gameinfo;
pub mod history;
pub mod http;
pub mod key_gate;
pub mod sgp_tokens;

pub use gameinfo::{
    GameinfoService, HistApi, LiveApi, PlayerSlot, RecentMatch, TeamView, ViewState,
};
pub use history::{
    AssetResult, HistoryService, MatchPage, RankedInfo, SummonerResult, ERR_NOT_CONNECTED,
};
