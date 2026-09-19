//! Metado http 能力（Task 3.1）：get/post（fetch 风格形状）。
//! 默认宿主面 = `HttpClient` trait（可注入 mock / 经 capability message 的实现）；
//! `ReqwestClient` 为内置阻塞实现（纯 http，TLS 后端按宿主构建期叠加）。

use metado_engine::capability::{CapabilityMeta, CapabilitySet};

pub struct MetaHttp;

impl CapabilitySet for MetaHttp {
    fn meta(&self) -> CapabilityMeta {
        CapabilityMeta {
            name: "http".into(),
            permissions: vec![
                "http.get".into(),
                "http.post".into(),
                "http.get.api.*".into(),
            ],
            exports: vec!["get".into(), "post".into()],
        }
    }
}

pub trait HttpClient {
    /// 完整响应体返回；非 2xx / 网络错误一律 Err
    fn get(&self, url: &str) -> Result<Vec<u8>, String>;
    fn post(&self, url: &str, body: &[u8]) -> Result<Vec<u8>, String>;
}

pub struct ReqwestClient;

impl ReqwestClient {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReqwestClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient for ReqwestClient {
    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        let resp = reqwest::blocking::get(url).map_err(|e| format!("http get: {}", e))?;
        let status = resp.status();
        let body = resp.bytes().map_err(|e| format!("http get body: {}", e))?;
        if !status.is_success() {
            return Err(format!("http {} {}: {}", status.as_u16(), status.as_str(), url));
        }
        Ok(body.to_vec())
    }

    fn post(&self, url: &str, body: &[u8]) -> Result<Vec<u8>, String> {
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(url)
            .body(body.to_vec())
            .send()
            .map_err(|e| format!("http post: {}", e))?;
        let status = resp.status();
        let resp_body = resp.bytes().map_err(|e| format!("http post body: {}", e))?;
        if !status.is_success() {
            return Err(format!(
                "http {} {}: {}",
                status.as_u16(),
                status.as_str(),
                url
            ));
        }
        Ok(resp_body.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_basic() {
        let meta = MetaHttp.meta();
        assert_eq!(meta.name, "http");
    }
}