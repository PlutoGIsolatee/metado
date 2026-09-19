//! Task 3.1: metado-cap-http tests

use metado_cap_http::{HttpClient, MetaHttp};
use metado_engine::capability::CapabilitySet;

#[test]
fn test_meta() {
    let meta = MetaHttp.meta();
    assert_eq!(meta.name, "http");
    for perm in ["http.get", "http.post", "http.get.api.*"] {
        assert!(meta.permissions.contains(&perm.to_string()), "missing {}", perm);
    }
    for exp in ["get", "post"] {
        assert!(meta.exports.contains(&exp.to_string()), "missing {}", exp);
    }
}

struct MockClient;

impl HttpClient for MockClient {
    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        if url.contains("missing") {
            return Err("404".into());
        }
        Ok(format!("GET {}", url).into_bytes())
    }

    fn post(&self, url: &str, body: &[u8]) -> Result<Vec<u8>, String> {
        let body = std::str::from_utf8(body).unwrap_or("<non-utf8>");
        Ok(format!("POST {} {}", url, body).into_bytes())
    }
}

#[test]
fn test_get_flow() {
    let client = MockClient;
    let out = client.get("https://example.com/x").unwrap();
    assert_eq!(out, b"GET https://example.com/x");
}

#[test]
fn test_post_flow() {
    let client = MockClient;
    let out = client
        .post("https://example.com/y", b"{\"a\":1}")
        .unwrap();
    assert_eq!(out, b"POST https://example.com/y {\"a\":1}");
}

#[test]
fn test_error_propagates() {
    let client = MockClient;
    assert!(client.get("https://example.com/missing").is_err());
}

#[test]
fn test_reqwest_smoke_localhost() {
    // 真实 reqwest 网络栈冒烟：连 127.0.0.1:1（必然端口不通）→ 报错即证明阻塞客户端已接线
    use metado_cap_http::ReqwestClient;
    let client = ReqwestClient::new();
    assert!(client.get("http://127.0.0.1:1/").is_err());
}