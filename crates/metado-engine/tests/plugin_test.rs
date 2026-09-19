//! Task 1.8: Plugin lifecycle state machine tests (§7)

use metado_engine::plugin::{Plugin, PluginState};

#[test]
fn test_lifecycle_transitions() {
    let mut plugin = Plugin::new("test-plugin", "signer123");
    assert_eq!(plugin.state, PluginState::Absent);

    plugin.transition(PluginState::Installing).unwrap();
    assert_eq!(plugin.state, PluginState::Installing);

    plugin.transition(PluginState::Installed).unwrap();
    assert_eq!(plugin.state, PluginState::Installed);

    plugin.transition(PluginState::Loading).unwrap();
    assert_eq!(plugin.state, PluginState::Loading);

    plugin.transition(PluginState::Pending).unwrap();
    assert_eq!(plugin.state, PluginState::Pending);

    plugin.transition(PluginState::Active).unwrap();
    assert_eq!(plugin.state, PluginState::Active);
}

#[test]
fn test_invalid_transition_fails() {
    let mut plugin = Plugin::new("test", "signer");
    assert!(plugin.transition(PluginState::Active).is_err()); // absent → active 非法
}

#[test]
fn test_evicted_can_reactivate() {
    let mut plugin = Plugin::new("test", "signer");
    plugin.transition(PluginState::Installing).unwrap();
    plugin.transition(PluginState::Installed).unwrap();
    plugin.transition(PluginState::Loading).unwrap();
    plugin.transition(PluginState::Pending).unwrap();
    plugin.transition(PluginState::Active).unwrap();
    plugin.transition(PluginState::Evicted).unwrap();
    plugin.transition(PluginState::Active).unwrap(); // 重建 realm
    assert_eq!(plugin.state, PluginState::Active);
}

#[test]
fn test_quarantine_from_active() {
    let mut plugin = Plugin::new("test", "signer");
    plugin.transition(PluginState::Installing).unwrap();
    plugin.transition(PluginState::Installed).unwrap();
    plugin.transition(PluginState::Loading).unwrap();
    plugin.transition(PluginState::Pending).unwrap();
    plugin.transition(PluginState::Active).unwrap();
    plugin.transition(PluginState::Quarantined).unwrap();
    assert_eq!(plugin.state, PluginState::Quarantined);
}

#[test]
fn test_uninstall_from_any_state() {
    let mut plugin = Plugin::new("test", "signer");
    plugin.transition(PluginState::Installing).unwrap();
    plugin.transition(PluginState::Uninstalled).unwrap();
    assert_eq!(plugin.state, PluginState::Uninstalled);
}