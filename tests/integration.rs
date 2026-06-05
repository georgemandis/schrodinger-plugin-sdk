// tests/integration.rs
use schrodinger_plugin_sdk::prelude::*;
use schrodinger_plugin_sdk::schrodinger_plugin;

#[derive(Default)]
struct TestPlugin;

impl NativePlugin for TestPlugin {
    fn applies_to(&self, entry: &ClipEntry) -> bool {
        entry.meta.formats.contains(&"public.utf8-plain-text".to_string())
    }

    fn apply(
        &self,
        entry: &ClipEntry,
        _mode: Option<&str>,
    ) -> Result<Vec<Output>, PluginError> {
        Ok(vec![Output::text("transformed")])
    }
}

schrodinger_plugin!(TestPlugin);

#[test]
fn test_abi_version() {
    assert_eq!(plugin_abi_version(), 1);
}

#[test]
fn test_applies_to_with_text() {
    let json = r#"{"change_count":1,"timestamp":0,"formats":["public.utf8-plain-text"]}"#;
    let c_str = std::ffi::CString::new(json).unwrap();
    // Note: plugin_init not called, so host API unavailable — but applies_to
    // only checks meta.formats in this test plugin, so it works without it.
    // Full integration requires host API mocking which we test at the host level.
    assert!(plugin_applies_to(c_str.as_ptr()));
}

#[test]
fn test_applies_to_without_text() {
    let json = r#"{"change_count":1,"timestamp":0,"formats":["public.png"]}"#;
    let c_str = std::ffi::CString::new(json).unwrap();
    assert!(!plugin_applies_to(c_str.as_ptr()));
}

#[test]
fn test_applies_to_null_returns_false() {
    assert!(!plugin_applies_to(std::ptr::null()));
}
