//! LCU HTTP 客户端（对齐 Go internal/lcu/client.rs 源 client.go）。
//! 并发闸门：`Semaphore(2)`（SGP 不走闸门）。

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::Semaphore;

use super::lockfile::auth_header;
use super::types::Credentials;

pub use super::lockfile::allowed_path;

const MAX_BODY: usize = 16 * 1024 * 1024; // 16MB 响应上限

#[derive(Debug, thiserror::Error)]
pub enum LcuError {
    #[error("path not allowed: {0}")]
    PathNotAllowed(String),
    #[error("http error: {0}")]
    Http(String),
    #[error("body too large")]
    BodyTooLarge,
    #[error("invalid utf8/json")]
    Invalid,
}

/// 已验证的 LCU 客户端。
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    auth: String,
    semaphore: Arc<Semaphore>,
}

impl Client {
    pub fn new(creds: &Credentials) -> Self {
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(true) // LCU 自签证书
            .timeout(Duration::from_secs(10))
            .build()
            .expect("build reqwest client");
        let base_url = format!("https://127.0.0.1:{}", creds.port);
        let auth = auth_header(&creds.token);
        Self {
            http,
            base_url,
            auth,
            semaphore: Arc::new(Semaphore::new(2)),
        }
    }

    pub async fn get(&self, path: &str) -> Result<Value, LcuError> {
        self.request("GET", path, None).await
    }

    /// GET 原始响应：返回 (status, body)；仅网络/路径/体积错误返回 Err。
    /// 404/403 等非 2xx 仍为 Ok，供服务层按状态码区分语义。
    pub async fn get_raw(&self, path: &str) -> Result<(u16, Vec<u8>), LcuError> {
        if !super::lockfile::allowed_path(path) {
            return Err(LcuError::PathNotAllowed(path.to_string()));
        }
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| LcuError::Http(e.to_string()))?;
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .get(&url)
            .header(reqwest::header::AUTHORIZATION, &self.auth)
            .send()
            .await
            .map_err(|e| LcuError::Http(e.to_string()))?;
        let status = resp.status().as_u16();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| LcuError::Http(e.to_string()))?;
        if bytes.len() > MAX_BODY {
            return Err(LcuError::BodyTooLarge);
        }
        Ok((status, bytes.to_vec()))
    }

    pub async fn post(&self, path: &str, body: Option<Value>) -> Result<Value, LcuError> {
        self.request("POST", path, body).await
    }

    pub async fn put(&self, path: &str, body: Option<Value>) -> Result<Value, LcuError> {
        self.request("PUT", path, body).await
    }

    pub async fn delete(&self, path: &str) -> Result<Value, LcuError> {
        self.request("DELETE", path, None).await
    }

    /// SGP 专用：不走 2 并发闸门。
    pub async fn get_no_gate(&self, path: &str) -> Result<Value, LcuError> {
        self.raw_get(path).await
    }

    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, LcuError> {
        if !super::lockfile::allowed_path(path) {
            return Err(LcuError::PathNotAllowed(path.to_string()));
        }
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| LcuError::Http(e.to_string()))?;
        self.raw_request(method, path, body).await
    }

    async fn raw_get(&self, path: &str) -> Result<Value, LcuError> {
        self.raw_request("GET", path, None).await
    }

    async fn raw_request(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, LcuError> {
        if !super::lockfile::allowed_path(path) {
            return Err(LcuError::PathNotAllowed(path.to_string()));
        }
        let url = format!("{}{}", self.base_url, path);
        let method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|e| LcuError::Http(e.to_string()))?;
        let mut req = self
            .http
            .request(method, &url)
            .header(reqwest::header::AUTHORIZATION, &self.auth)
            .header(reqwest::header::CONTENT_TYPE, "application/json");
        if let Some(b) = body {
            req = req.json(&b);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| LcuError::Http(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 204 {
            return Ok(Value::Null);
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| LcuError::Http(e.to_string()))?;
        if bytes.len() > MAX_BODY {
            return Err(LcuError::BodyTooLarge);
        }
        if status.as_u16() == 404 {
            // 探测链路用 404 判定 unauthenticated 等
            return Err(LcuError::Http(format!("404 {}", path)));
        }
        if !status.is_success() {
            return Err(LcuError::Http(format!("{} {}", status.as_u16(), path)));
        }
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_slice(&bytes).map_err(|_| LcuError::Invalid)
    }
}
