use std::collections::HashMap;

/// 权限名 → 命名空间（首个 `.` 分段；空输入 → 空串）。
pub fn namespace_of(permission: &str) -> &str {
    permission.split('.').next().unwrap_or("")
}

/// 段级权限匹配：`pattern` 授权 `request`。
/// - request 是 pattern 的段前缀（等于或更短）即授权：授权更细粒度的
///   `http.get.api.*` 意味着 `http.get` 方法可用，域名细分由后端在调用期裁决
/// - 尾段 `*` 作贪心后缀（覆盖 `example.com` 这类含 `.` 的多段域名/路径）
/// - `<...>` 段（signer 绑定模板）与单段 `*` 通配任意单段
/// - request 深于 pattern（段数更多）且 pattern 无尾段 `*` 不授权
pub fn permission_allows(pattern: &str, request: &str) -> bool {
    let p: Vec<&str> = pattern.split('.').filter(|s| !s.is_empty()).collect();
    let r: Vec<&str> = request.split('.').filter(|s| !s.is_empty()).collect();

    let eq_or_wildcard = |pk: &str, rk: &str| {
        pk == "*" || (pk.starts_with('<') && pk.ends_with('>')) || pk == rk
    };

    if r.len() > p.len() {
        // 仅当 pattern 以尾段 `*` 收尾时允许更长 request（其余段须匹配前缀）
        if p.last() == Some(&"*") {
            let prefix = &p[..p.len() - 1];
            if r.len() < prefix.len() {
                return false;
            }
            return prefix.iter().zip(r.iter()).all(|(pk, rk)| eq_or_wildcard(pk, rk));
        }
        return false;
    }
    p.iter().zip(r.iter()).all(|(pk, rk)| eq_or_wildcard(pk, rk))
}

/// granted 中任一模式授权 `request` 即放行。
pub fn grants_allow(granted: &[String], request: &str) -> bool {
    granted.iter().any(|g| permission_allows(g, request))
}

/// available 权限集的命名空间集合（去重、排序）。
pub fn available_namespaces(available: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for a in available {
        let ns = namespace_of(a);
        if !ns.is_empty() && !out.iter().any(|s| s == ns) {
            out.push(ns.to_string());
        }
    }
    out.sort();
    out
}

/// 导出命名空间 = 命名空间 ∈ available 命名空间的请求命名空间 ∪ {metado}。
/// 粒度是命名空间而非权限字面量：请求 `http.get.api.example`（可用 `http.get.api.*`）
/// 只要求命名空间 `http` 受宿主支持即导出，不因权限面差异让命名空间整个消失。
pub fn exported_namespaces(requested: &[String], available: &[String]) -> Vec<String> {
    let avail = available_namespaces(available);
    let mut out: Vec<String> = requested
        .iter()
        .map(|r| namespace_of(r).to_string())
        .filter(|ns| !ns.is_empty() && avail.iter().any(|a| a == ns))
        .collect();
    out.push("metado".into());
    out.sort();
    out.dedup();
    out
}

#[derive(Debug, Clone, Default)]
pub struct PermissionSet {
    sets: HashMap<String, Vec<String>>,
}

impl PermissionSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn define(&mut self, name: &str, perms: Vec<String>) {
        self.sets.insert(name.to_string(), perms);
    }

    pub fn expand(&self, names: &[String]) -> Result<Vec<String>, String> {
        let mut result = Vec::new();
        for name in names {
            match self.sets.get(name) {
                Some(perms) => result.extend(perms.clone()),
                None => return Err(format!("Undefined permission set: {}", name)),
            }
        }
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_of_first_segment() {
        assert_eq!(namespace_of("http.get"), "http");
        assert_eq!(namespace_of("storage.<signer>.read"), "storage");
        assert_eq!(namespace_of("log"), "log");
        assert_eq!(namespace_of(""), "");
    }

    #[test]
    fn permission_allows_exact() {
        assert!(permission_allows("http.get", "http.get"));
        assert!(!permission_allows("http.post", "http.get"));
    }

    #[test]
    fn permission_allows_granular_grant_covers_parent_method() {
        // 授权 http.get.api.* ⇒ http.get 方法可用（域名在调用期由后端裁决）
        assert!(permission_allows("http.get.api.*", "http.get"));
        assert!(permission_allows("http.get.api.*", "http.get.api.example.com"));
        assert!(permission_allows("http.get.api.*", "http.get.api"));
        // 精确授权不放大到子路径
        assert!(!permission_allows("http.get", "http.get.api.example.com"));
    }

    #[test]
    fn permission_allows_signer_wildcard() {
        assert!(permission_allows("storage.<signer>.write", "storage.write"));
        assert!(permission_allows("storage.<signer>.read", "storage.gc2139.read"));
        assert!(!permission_allows("storage.<signer>.write", "storage.write.deep"));
        assert!(!permission_allows("storage.<signer>.read", "log.info"));
    }

    #[test]
    fn grants_allow_any_pattern() {
        let granted = vec!["log.info".to_string(), "http.get.api.*".to_string()];
        assert!(grants_allow(&granted, "http.get"));
        assert!(grants_allow(&granted, "log.info"));
        assert!(!grants_allow(&granted, "storage.write"));
    }

    #[test]
    fn available_namespaces_dedup_sorted() {
        let avail = vec![
            "http.get".to_string(),
            "http.post".to_string(),
            "storage.read".to_string(),
        ];
        assert_eq!(
            available_namespaces(&avail),
            vec!["http".to_string(), "storage".to_string()]
        );
    }

    #[test]
    fn exported_namespaces_namespace_level() {
        let avail = vec![
            "http.get".to_string(),
            "http.post".to_string(),
            "http.get.api.*".to_string(),
            "storage.read".to_string(),
            "log.info".to_string(),
        ];
        let requested = vec![
            "http.get.api.example".to_string(),
            "storage.read".to_string(),
            "no.such.perm".to_string(),
        ];
        let exported = exported_namespaces(&requested, &avail);
        assert!(exported.contains(&"http".to_string()));
        assert!(exported.contains(&"storage".to_string()));
        assert!(exported.contains(&"metado".to_string()));
        assert!(!exported.contains(&"no".to_string()));
        assert!(!exported.contains(&"log".to_string()));
    }
}
