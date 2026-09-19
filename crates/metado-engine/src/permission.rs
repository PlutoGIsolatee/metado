use std::collections::HashMap;
use std::collections::HashSet;

pub struct PermissionResolver {
    requested: Vec<String>,
    granted: HashSet<String>,
    available: Vec<String>,
    signer_id: Option<String>,
    domain_rules: Vec<Box<dyn Fn(&str) -> bool>>,
}

impl PermissionResolver {
    pub fn new(requested: Vec<String>, available: Vec<String>) -> Self {
        Self {
            requested,
            granted: HashSet::new(),
            available,
            signer_id: None,
            domain_rules: Vec::new(),
        }
    }

    pub fn bind_signer(&mut self, signer_id: &str) {
        self.signer_id = Some(signer_id.to_string());
    }

    pub fn can_grant(&self, perm: &str) -> bool {
        self.requested
            .iter()
            .any(|r| self.matches(r, perm))
            && self.available.iter().any(|a| self.matches(a, perm))
    }

    fn matches(&self, template: &str, perm: &str) -> bool {
        if template == perm {
            return true;
        }
        if template.contains("<signer>") {
            // requested/available 中的 `<signer>` 模板在绑定后对具体权限同样成立
            return self.resolve_signer_placeholder(template) == perm;
        }
        false
    }

    pub fn grant(&mut self, perms: Vec<String>) {
        for p in perms {
            if self.can_grant(&p) {
                self.granted.insert(p);
            }
        }
    }

    pub fn revoke(&mut self, perms: &[String]) {
        for p in perms {
            self.granted.remove(p);
        }
    }

    pub fn check(&self, capability_name: &str) -> Result<(), String> {
        if !self.granted.contains(capability_name) {
            return Err(format!("Permission denied: {}", capability_name));
        }
        for rule in &self.domain_rules {
            if !rule(capability_name) {
                return Err(format!("Domain rule denied: {}", capability_name));
            }
        }
        Ok(())
    }

    pub fn resolve_signer_placeholder(&self, perm: &str) -> String {
        if let Some(ref signer) = self.signer_id {
            perm.replace("<signer>", signer)
        } else {
            perm.to_string()
        }
    }
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