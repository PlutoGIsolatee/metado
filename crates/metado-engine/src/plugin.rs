use crate::manifest::Manifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    Absent,
    Installing,
    Installed,
    Loading,
    Pending,
    Active,
    Evicted,
    Quarantined,
    Uninstalled,
}

pub struct Plugin {
    pub id: String,
    pub signer_id: String,
    pub manifest: Option<Manifest>,
    pub state: PluginState,
    pub granted: Vec<String>,
}

impl Plugin {
    pub fn new(id: &str, signer_id: &str) -> Self {
        Self {
            id: id.to_string(),
            signer_id: signer_id.to_string(),
            manifest: None,
            state: PluginState::Absent,
            granted: Vec::new(),
        }
    }

    pub fn transition(&mut self, new_state: PluginState) -> Result<(), String> {
        let valid = matches!(
            (&self.state, &new_state),
            (PluginState::Absent, PluginState::Installing)
                | (PluginState::Installing, PluginState::Installed)
                | (PluginState::Installed, PluginState::Loading)
                | (PluginState::Loading, PluginState::Pending)
                | (PluginState::Pending, PluginState::Active)
                | (PluginState::Active, PluginState::Evicted)
                | (PluginState::Active, PluginState::Quarantined)
                | (PluginState::Evicted, PluginState::Active)
                | (PluginState::Quarantined, PluginState::Active)
                // 任何状态 → Uninstalled
                | (_, PluginState::Uninstalled)
        );

        if valid {
            self.state = new_state;
            Ok(())
        } else {
            Err(format!(
                "invalid transition: {:?} \u{2192} {:?}",
                self.state, new_state
            ))
        }
    }
}