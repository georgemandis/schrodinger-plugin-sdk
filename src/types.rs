use serde::Deserialize;
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Error type for plugin operations.
#[derive(Debug)]
pub struct PluginError(pub String);

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for PluginError {}

impl From<String> for PluginError {
    fn from(s: String) -> Self { PluginError(s) }
}

impl From<&str> for PluginError {
    fn from(s: &str) -> Self { PluginError(s.to_string()) }
}

/// A single output from a plugin's apply() call.
pub struct Output {
    pub format: String,
    pub data: Vec<u8>,
}

impl Output {
    pub fn new(format: &str) -> OutputBuilder {
        OutputBuilder { format: format.to_string(), data: Vec::new() }
    }

    pub fn text(text: &str) -> Self {
        Output {
            format: "public.utf8-plain-text".to_string(),
            data: text.as_bytes().to_vec(),
        }
    }

    pub fn image(data: Vec<u8>) -> Self {
        Output { format: "public.png".to_string(), data }
    }
}

pub struct OutputBuilder {
    format: String,
    data: Vec<u8>,
}

impl OutputBuilder {
    pub fn data(self, data: Vec<u8>) -> Output {
        Output { format: self.format, data }
    }
}

/// Metadata about a clipboard entry, parsed from the host's JSON.
#[derive(Debug, Clone, Deserialize)]
pub struct EntryMeta {
    pub change_count: i64,
    pub timestamp: u64,
    pub formats: Vec<String>,
    #[serde(default)]
    pub source_app: Option<String>,
}

/// Thread-safe key-value store for caching analysis results.
#[derive(Clone, Default)]
pub struct AnalysisData {
    inner: Arc<Mutex<HashMap<String, Box<dyn Any + Send>>>>,
}

impl AnalysisData {
    pub fn empty() -> Self { Self::default() }

    pub fn new() -> Self { Self::default() }

    pub fn set<T: Any + Send + 'static>(self, key: &str, value: T) -> Self {
        self.inner.lock().unwrap().insert(key.to_string(), Box::new(value));
        self
    }

    pub fn get_cloned<T: Any + Send + Clone + 'static>(&self, key: &str) -> Option<T> {
        self.inner.lock().unwrap().get(key)
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
    }

    pub fn has(&self, key: &str) -> bool {
        self.inner.lock().unwrap().contains_key(key)
    }
}
