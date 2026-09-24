//! 统一应用错误（AppError）。
//!
//! 内部签名一律 `Result<T, AppError>`，可按变体分支处理；
//! Tauri 命令边界（commands.rs）经 `From<AppError> for String` 转用户可读文案。
//! `Display` 即用户可读文案（中文），开发向细节由调用方写日志。

/// LCU 未连接的用户可读文案（服务层多处复用）。
pub const ERR_NOT_CONNECTED: &str = "LCU 未连接，请先启动英雄联盟客户端并登录";

/// 应用统一错误。`Display` 即用户可读文案（中文）。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// LCU 未连接
    #[error("{}", ERR_NOT_CONNECTED)]
    NotConnected,
    /// HTTP 传输失败 / 非 2xx（开发向细节，如状态码与路径）
    #[error("{0}")]
    Http(String),
    /// 资源不存在（404 等），`{0}` 为用户文案
    #[error("{0}")]
    NotFound(String),
    /// 响应解析失败
    #[error("{0}")]
    Parse(String),
    /// 更新流程错误
    #[error("{0}")]
    Updater(String),
    /// 参数 / 前置条件不满足
    #[error("{0}")]
    Invalid(String),
    /// 未归类的用户可读消息（过渡用，逐步归入上面各变体）
    #[error("{0}")]
    Msg(String),
}

/// 兼容层：既有 `Err("...".into())` 站点零改动迁移。
impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Msg(s)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::Msg(s.to_string())
    }
}

/// Tauri 命令边界：错误 → 用户可读文案。
impl From<AppError> for String {
    fn from(e: AppError) -> Self {
        e.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T2.3 回归：错误必须可按变体匹配（旧实现只有 String，只能字符串嗅探）。
    #[test]
    fn app_error_variants_are_matchable() {
        let not_found = AppError::NotFound("未找到战绩数据".into());
        assert!(matches!(not_found, AppError::NotFound(_)));
        assert!(matches!(
            AppError::Http("timeout".into()),
            AppError::Http(_)
        ));
        assert!(matches!(AppError::NotConnected, AppError::NotConnected));
        assert!(!matches!(
            AppError::Http("404 x".into()),
            AppError::NotFound(_)
        ));

        // Display 即用户文案
        assert_eq!(AppError::NotConnected.to_string(), ERR_NOT_CONNECTED);
        assert_eq!(
            AppError::NotFound("未找到战绩数据".into()).to_string(),
            "未找到战绩数据"
        );

        // 命令边界：AppError → String 一键转换
        let s: String = AppError::Updater("下载失败".into()).into();
        assert_eq!(s, "下载失败");

        // 兼容层：既有 Err("...".into()) 站点可直接迁移为 Msg
        let m: AppError = "一般消息".to_string().into();
        assert!(matches!(m, AppError::Msg(_)));
    }

    /// T2.3 回归：404（NotFound）与传输错误（Http）必须可区分，
    /// 供探测链路判定未登录态（替代 `contains("404")` 字符串嗅探）。
    #[test]
    fn not_found_distinguishable_from_http() {
        fn is_unauthenticated_probe(e: &AppError) -> bool {
            matches!(e, AppError::NotFound(_))
        }
        assert!(is_unauthenticated_probe(&AppError::NotFound(
            "404 /lol-summoner/v1/current-summoner".into()
        )));
        assert!(!is_unauthenticated_probe(&AppError::Http(
            "timeout: connection refused".into()
        )));
    }
}
