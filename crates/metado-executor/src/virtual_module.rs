use std::collections::HashSet;

/// `@metado/runtime` 虚拟模块：把 available 能力映射为可导出的 API。
/// 权限字符串形如 `http.get`（命名空间.方法）；导出存在性由 available 决定，
/// 实际放行由 granted 决定。
pub struct VirtualModule {
    available: HashSet<String>,
    granted: HashSet<String>,
}

impl VirtualModule {
    pub fn build(available: Vec<String>, granted: Vec<String>) -> Self {
        Self {
            available: available.into_iter().collect(),
            granted: granted.into_iter().collect(),
        }
    }

    pub fn has_export(&self, name: &str) -> bool {
        // 导出 = available 中任一能力的命名空间前缀或 final 方法段等于 name
        self.available.iter().any(|a| {
            a == name || a.starts_with(&format!("{}.", name)) || a.ends_with(&format!(".{}", name))
        })
    }

    pub fn is_granted(&self, capability: &str) -> bool {
        self.granted.contains(capability)
    }
}