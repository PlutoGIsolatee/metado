pub struct CapabilityMeta {
    pub name: String,
    pub permissions: Vec<String>,
    pub exports: Vec<String>,
}

pub trait CapabilitySet {
    fn meta(&self) -> CapabilityMeta;
}

pub struct CapabilityRegistry {
    capabilities: Vec<Box<dyn CapabilitySet>>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self {
            capabilities: Vec::new(),
        }
    }

    pub fn register(&mut self, cap: Box<dyn CapabilitySet>) {
        self.capabilities.push(cap);
    }

    pub fn get(&self, name: &str) -> Option<&dyn CapabilitySet> {
        self.capabilities
            .iter()
            .find(|c| c.meta().name == name)
            .map(|c| c.as_ref())
    }

    pub fn all_permissions(&self) -> Vec<String> {
        self.capabilities
            .iter()
            .flat_map(|c| c.meta().permissions)
            .collect()
    }

    pub fn all_exports(&self) -> Vec<String> {
        self.capabilities
            .iter()
            .flat_map(|c| c.meta().exports)
            .collect()
    }
}