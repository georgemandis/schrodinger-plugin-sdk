// src/host_api.rs
use std::ffi::{c_char, CStr, CString};

/// The raw C struct passed from the host at plugin_init.
/// Matches the host's HostAPI layout exactly.
#[repr(C)]
pub struct HostAPIRaw {
    pub version: u32,
    pub read_format: Option<
        unsafe extern "C" fn(
            format_name: *const c_char,
            out_buf: *mut *const u8,
            out_len: *mut usize,
        ) -> i32,
    >,
    pub get_config_json: Option<unsafe extern "C" fn() -> *const c_char>,
    pub log: Option<unsafe extern "C" fn(level: i32, message: *const c_char)>,
    pub platform: Option<unsafe extern "C" fn() -> *const c_char>,
}

/// Safe wrapper around the raw Host API.
pub struct HostAPI {
    raw: *const HostAPIRaw,
}

// Safety: The host guarantees the HostAPIRaw pointer is valid for the plugin's lifetime.
// The function pointers are thread-safe (host uses appropriate synchronization).
unsafe impl Send for HostAPI {}
unsafe impl Sync for HostAPI {}

impl HostAPI {
    /// Create from raw pointer. Called by macro-generated init code.
    ///
    /// # Safety
    /// The pointer must be valid for the lifetime of the plugin.
    pub unsafe fn from_raw(raw: *const HostAPIRaw) -> Self {
        HostAPI { raw }
    }

    /// Read clipboard format data. Returns owned bytes.
    pub fn read_format(&self, format_name: &str) -> Result<Vec<u8>, String> {
        let c_name = CString::new(format_name)
            .map_err(|e| format!("invalid format name: {}", e))?;
        let mut buf: *const u8 = std::ptr::null();
        let mut len: usize = 0;

        let raw = unsafe { &*self.raw };
        let read_fn = raw.read_format.ok_or("read_format not available")?;

        let status = unsafe { read_fn(c_name.as_ptr(), &mut buf, &mut len) };
        if status != 0 || buf.is_null() {
            return Err(format!("read_format failed for '{}'", format_name));
        }

        // Copy into owned Vec for safety (pointer valid until next read_format call)
        let data = unsafe { std::slice::from_raw_parts(buf, len) }.to_vec();
        Ok(data)
    }

    /// Get plugin configuration as parsed JSON.
    pub fn get_config(&self) -> serde_json::Value {
        let raw = unsafe { &*self.raw };
        let config_fn = match raw.get_config_json {
            Some(f) => f,
            None => return serde_json::Value::Object(Default::default()),
        };
        let ptr = unsafe { config_fn() };
        if ptr.is_null() {
            return serde_json::Value::Object(Default::default());
        }
        let c_str = unsafe { CStr::from_ptr(ptr) };
        serde_json::from_str(c_str.to_str().unwrap_or("{}"))
            .unwrap_or_else(|_| serde_json::Value::Object(Default::default()))
    }

    /// Log a message to the host.
    pub fn log(&self, level: i32, message: &str) {
        let raw = unsafe { &*self.raw };
        if let Some(log_fn) = raw.log {
            if let Ok(c_msg) = CString::new(message) {
                unsafe { log_fn(level, c_msg.as_ptr()) };
            }
        }
    }

    /// Get the current platform identifier.
    pub fn platform(&self) -> String {
        let raw = unsafe { &*self.raw };
        let plat_fn = match raw.platform {
            Some(f) => f,
            None => return String::new(),
        };
        let ptr = unsafe { plat_fn() };
        if ptr.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}
