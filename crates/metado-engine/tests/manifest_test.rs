//! Task 1.3: Manifest parser tests (§4.2)

use metado_engine::manifest::{EntryDef, LifecycleMode, Manifest};

#[test]
fn test_parse_minimal_manifest() {
    let toml = r#"
        name = "myplugin"
        version = "1.0.0"
    "#;
    let m: Manifest = Manifest::from_toml(toml).unwrap();
    assert_eq!(m.name, "myplugin");
    assert_eq!(m.version, "1.0.0");
    assert!(m.permission.is_empty());
    assert!(m.entries.is_empty());
}

#[test]
fn test_parse_full_manifest() {
    let toml = r#"
        name = "myplugin"
        version = "2.0.0"
        permission = ["http.get.api.example", "storage.<signer>.write"]
        permission-set = ["standard"]
        lifecycle = "resident-high"

        [entries.onMessage]
        export = "onMessage"

        [entries.boot]
        export = "boot"
    "#;
    let m: Manifest = Manifest::from_toml(toml).unwrap();
    assert_eq!(m.name, "myplugin");
    assert_eq!(m.version, "2.0.0");
    assert_eq!(
        m.permission,
        vec!["http.get.api.example", "storage.<signer>.write"]
    );
    assert_eq!(m.permission_set, vec!["standard"]);
    assert_eq!(m.lifecycle, LifecycleMode::ResidentHigh);
    assert_eq!(m.entries.len(), 2);
    assert_eq!(m.entries["onMessage"].export, "onMessage");
    assert_eq!(m.entries["boot"].export, "boot");
}

#[test]
fn test_entry_def_export() {
    let e = EntryDef {
        export: "handle".into(),
    };
    assert_eq!(e.export, "handle");
}

#[test]
fn test_invalid_toml() {
    let toml = "this is not toml {{{";
    assert!(Manifest::from_toml(toml).is_err());
}