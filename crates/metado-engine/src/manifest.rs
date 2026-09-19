use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum LifecycleMode {
    #[serde(rename = "resident-high")]
    ResidentHigh,
    #[serde(rename = "resident-low")]
    ResidentLow,
    #[serde(rename = "cold")]
    Cold,
}

impl Default for LifecycleMode {
    fn default() -> Self {
        Self::ResidentLow
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntryDef {
    pub export: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub permission: Vec<String>,
    #[serde(default, rename = "permission-set")]
    pub permission_set: Vec<String>,
    #[serde(default)]
    pub lifecycle: LifecycleMode,
    #[serde(default)]
    pub entries: HashMap<String, EntryDef>,
}

impl Manifest {
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }
}