//! 战绩数据解析层：LCU / SGP 战绩 JSON → 前端视图模型（纯函数，可单测）。
//! JSON 字段契约对齐 Yuumi match_parser.rs 与 Go internal/parser。

pub mod lcu_match;
pub mod queue;
pub mod sgp_match;

pub use lcu_match::{
    format_duration, format_short_time, format_time, kda_string, parse_match_detail,
    parse_match_summaries, rating, tier_cn, MatchDetail, MatchSummary, PlayerRow, TeamSummary,
};
pub use queue::{lookup_queue, queue_info_for, QueueInfo};
pub use sgp_match::parse_sgp_summaries;
