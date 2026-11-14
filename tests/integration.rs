// tests/integration.rs
use schrodinger_plugin_sdk::prelude::*;
use schrodinger_plugin_sdk::schrodinger_plugin;

#[derive(Default)]
struct TestPlugin;

impl NativePlugin for TestPlugin {
    fn applies_to(&self, entry: &ClipEntry) -> bool {
        entry.meta.formats.contains(&"public.utf8-plain-text".to_string())
    }

    fn applies_to_mode(&self, entry: &ClipEntry, mode: &str) -> bool {
        // "shout" only makes sense when the clip also carries HTML.
        mode != "shout" || entry.meta.formats.contains(&"public.html".to_string())
    }

    fn apply(
        &self,
        entry: &ClipEntry,
        _mode: Option<&str>,
    ) -> Result<Vec<Output>, PluginError> {
        Ok(vec![Output::text(entry.input().unwrap_or("transformed"))])
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

#[test]
fn test_applies_to_mode_is_answered_per_mode() {
    let mode = |m: &str| std::ffi::CString::new(m).unwrap();
    let text = std::ffi::CString::new(r#"{"change_count":1,"timestamp":0,"formats":["public.utf8-plain-text"]}"#).unwrap();
    let html = std::ffi::CString::new(r#"{"change_count":2,"timestamp":0,"formats":["public.utf8-plain-text","public.html"]}"#).unwrap();
    assert!(plugin_applies_to_mode(text.as_ptr(), mode("plain").as_ptr()));
    assert!(!plugin_applies_to_mode(text.as_ptr(), mode("shout").as_ptr()));
    assert!(plugin_applies_to_mode(html.as_ptr(), mode("shout").as_ptr()));
    // No mode to ask about: nothing is ruled out.
    assert!(plugin_applies_to_mode(text.as_ptr(), std::ptr::null()));
}

#[test]
fn test_typed_input_reaches_the_entry() {
    let with: EntryMeta = serde_json::from_str(r#"{"change_count":1,"timestamp":0,"formats":[],"input":"select 1"}"#).unwrap();
    assert_eq!(with.input.as_deref(), Some("select 1"));
    // A host that predates prompts sends no such field.
    let without: EntryMeta = serde_json::from_str(r#"{"change_count":1,"timestamp":0,"formats":[]}"#).unwrap();
    assert_eq!(without.input, None);
}
