//! 共享纯工具：URL 转义 / flex JSON 字符串 / ID 与显示名归一。

use serde::Deserialize as _;
use serde_json::Value;

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

/* ── flex JSON 值：string / number / null 动态取串 ── */

/// flexString：兼容 JSON 里 string / number / null 形态。
pub fn flex_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Null => String::new(),
        _ => {
            let s = v.to_string();
            s.trim_matches('"').to_string()
        }
    }
}

/// serde 容错：任意 JSON 值 → String（flex 形态）。
pub fn de_flex_str<'de, D>(d: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Value::deserialize(d)?;
    Ok(flex_value(&v))
}

/* ── ID 与显示名归一 ── */

/// ID 归一：空串与哨兵 "0" 一律视为缺失。
pub fn opt_id(s: String) -> Option<String> {
    let s = s.trim().to_string();
    if s.is_empty() || s == "0" {
        None
    } else {
        Some(s)
    }
}

/// 显示名归一：空串视为缺失。
pub fn opt_name(s: String) -> Option<String> {
    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
