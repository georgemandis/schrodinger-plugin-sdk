// src/ffi.rs

/// The trait that native plugin authors implement.
pub trait NativePlugin: Send + Sync + 'static {
    /// Heavyweight pre-scan. Override to cache analysis data for applies_to.
    fn analyze(
        &self,
        entry: &crate::types::ClipEntry,
    ) -> Result<crate::types::AnalysisData, crate::types::PluginError> {
        Ok(crate::types::AnalysisData::empty())
    }

    /// Should this plugin appear for this clipboard entry?
    fn applies_to(&self, entry: &crate::types::ClipEntry) -> bool;

    /// Perform the transformation.
    fn apply(
        &self,
        entry: &crate::types::ClipEntry,
        mode: Option<&str>,
    ) -> Result<Vec<crate::types::Output>, crate::types::PluginError>;
}

/// Macro that generates all extern "C" exports for a NativePlugin implementation.
///
/// Usage: `schrodinger_plugin!(MyPlugin);`
///
/// The type must implement Default + NativePlugin.
#[macro_export]
macro_rules! schrodinger_plugin {
    ($plugin_type:ty) => {
        // --- Global state managed by the macro ---
        static PLUGIN: std::sync::OnceLock<$plugin_type> = std::sync::OnceLock::new();
        static HOST_API: std::sync::OnceLock<std::sync::Arc<$crate::host_api::HostAPI>> =
            std::sync::OnceLock::new();
        static TASK_MANAGER: std::sync::OnceLock<$crate::task::TaskManager> =
            std::sync::OnceLock::new();
        static ANALYSIS_CACHE: std::sync::OnceLock<
            std::sync::Mutex<std::collections::HashMap<i64, $crate::types::AnalysisData>>,
        > = std::sync::OnceLock::new();

        fn get_plugin() -> &'static $plugin_type {
            PLUGIN.get_or_init(|| <$plugin_type>::default())
        }

        fn get_tasks() -> &'static $crate::task::TaskManager {
            TASK_MANAGER.get_or_init(|| $crate::task::TaskManager::new())
        }

        fn get_analysis_cache() -> &'static std::sync::Mutex<
            std::collections::HashMap<i64, $crate::types::AnalysisData>,
        > {
            ANALYSIS_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
        }

        fn parse_entry_meta(json: *const std::ffi::c_char) -> Option<$crate::types::EntryMeta> {
            if json.is_null() { return None; }
            let c_str = unsafe { std::ffi::CStr::from_ptr(json) };
            let s = c_str.to_str().ok()?;
            serde_json::from_str(s).ok()
        }

        fn build_clip_entry(
            meta: $crate::types::EntryMeta,
        ) -> $crate::types::ClipEntry {
            let api = HOST_API
                .get()
                .cloned()
                .unwrap_or_else(|| std::sync::Arc::new($crate::host_api::HostAPI::null()));
            let analysis = get_analysis_cache()
                .lock()
                .unwrap()
                .get(&meta.change_count)
                .cloned()
                .unwrap_or_else(|| $crate::types::AnalysisData::empty());
            $crate::types::ClipEntry::new(meta, api, analysis)
        }

        // --- Exported C ABI functions ---

        #[no_mangle]
        pub extern "C" fn plugin_abi_version() -> u32 {
            1
        }

        #[no_mangle]
        pub unsafe extern "C" fn plugin_init(
            api: *const $crate::host_api::HostAPIRaw,
            _api_version: u32,
        ) {
            if api.is_null() { return; }
            let host_api = unsafe { $crate::host_api::HostAPI::from_raw(api) };
            let _ = HOST_API.set(std::sync::Arc::new(host_api));
            let _ = PLUGIN.set(<$plugin_type>::default());
            let _ = TASK_MANAGER.set($crate::task::TaskManager::new());
            let _ = ANALYSIS_CACHE.set(
                std::sync::Mutex::new(std::collections::HashMap::new()),
            );
        }

        #[no_mangle]
        pub extern "C" fn plugin_applies_to(entry_json: *const std::ffi::c_char) -> bool {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let meta = match parse_entry_meta(entry_json) {
                    Some(m) => m,
                    None => return false,
                };
                let entry = build_clip_entry(meta);
                get_plugin().applies_to(&entry)
            }));
            result.unwrap_or(false)
        }

        #[no_mangle]
        pub extern "C" fn plugin_analyze(
            entry_json: *const std::ffi::c_char,
        ) -> u64 {
            let meta = match parse_entry_meta(entry_json) {
                Some(m) => m,
                None => return 0,
            };
            let change_count = meta.change_count;

            get_tasks().spawn(move || {
                let entry = build_clip_entry(meta);
                match get_plugin().analyze(&entry) {
                    Ok(analysis) => {
                        get_analysis_cache()
                            .lock()
                            .unwrap()
                            .insert(change_count, analysis);
                        $crate::task::TaskResult::AnalysisDone
                    }
                    Err(e) => $crate::task::TaskResult::Error(e.0),
                }
            })
        }

        #[no_mangle]
        pub extern "C" fn plugin_apply(
            entry_json: *const std::ffi::c_char,
            mode: *const std::ffi::c_char,
        ) -> u64 {
            let meta = match parse_entry_meta(entry_json) {
                Some(m) => m,
                None => return 0,
            };
            let mode_str = if mode.is_null() {
                None
            } else {
                unsafe { std::ffi::CStr::from_ptr(mode) }
                    .to_str()
                    .ok()
                    .map(|s| s.to_string())
            };

            get_tasks().spawn(move || {
                let entry = build_clip_entry(meta);
                match get_plugin().apply(&entry, mode_str.as_deref()) {
                    Ok(outputs) => $crate::task::TaskResult::Outputs(outputs),
                    Err(e) => $crate::task::TaskResult::Error(e.0),
                }
            })
        }

        #[no_mangle]
        pub extern "C" fn plugin_poll(handle: u64, progress: *mut f32) -> i32 {
            let (code, prog) = get_tasks().poll(handle);
            if !progress.is_null() {
                unsafe { *progress = prog; }
            }
            code
        }

        // --- Result access ---
        static COMPLETED_RESULTS: std::sync::OnceLock<
            std::sync::Mutex<
                std::collections::HashMap<u64, Vec<$crate::types::Output>>,
            >,
        > = std::sync::OnceLock::new();

        fn get_completed_results() -> &'static std::sync::Mutex<
            std::collections::HashMap<u64, Vec<$crate::types::Output>>,
        > {
            COMPLETED_RESULTS
                .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
        }

        fn materialize_results(handle: u64) {
            if get_completed_results().lock().unwrap().contains_key(&handle) {
                return;
            }
            if let Some(result) = get_tasks().take_result(handle) {
                if let $crate::task::TaskResult::Outputs(outputs) = result {
                    get_completed_results()
                        .lock()
                        .unwrap()
                        .insert(handle, outputs);
                }
            }
        }

        #[no_mangle]
        pub extern "C" fn plugin_result_count(handle: u64) -> u32 {
            materialize_results(handle);
            get_completed_results()
                .lock()
                .unwrap()
                .get(&handle)
                .map_or(0, |v| v.len() as u32)
        }

        #[no_mangle]
        pub extern "C" fn plugin_result_format(
            handle: u64,
            index: u32,
        ) -> *const std::ffi::c_char {
            materialize_results(handle);
            let results = get_completed_results().lock().unwrap();
            match results.get(&handle).and_then(|v| v.get(index as usize)) {
                Some(output) => {
                    match std::ffi::CString::new(output.format.clone()) {
                        Ok(cs) => cs.into_raw(),
                        Err(_) => std::ptr::null(),
                    }
                }
                None => std::ptr::null(),
            }
        }

        #[no_mangle]
        pub extern "C" fn plugin_result_data(
            handle: u64,
            index: u32,
            out_buf: *mut *const u8,
            out_len: *mut usize,
        ) -> i32 {
            if out_buf.is_null() || out_len.is_null() {
                return -1;
            }
            materialize_results(handle);
            let results = get_completed_results().lock().unwrap();
            match results.get(&handle).and_then(|v| v.get(index as usize)) {
                Some(output) => {
                    unsafe {
                        *out_buf = output.data.as_ptr();
                        *out_len = output.data.len();
                    }
                    0
                }
                None => -1,
            }
        }

        #[no_mangle]
        pub extern "C" fn plugin_result_error(handle: u64) -> *const std::ffi::c_char {
            if let Some(result) = get_tasks().take_result(handle) {
                if let $crate::task::TaskResult::Error(msg) = result {
                    match std::ffi::CString::new(msg) {
                        Ok(cs) => return cs.into_raw(),
                        Err(_) => {}
                    }
                }
            }
            std::ptr::null()
        }

        #[no_mangle]
        pub extern "C" fn plugin_free_result(handle: u64) {
            get_completed_results().lock().unwrap().remove(&handle);
            get_tasks().remove(handle);
        }

        #[no_mangle]
        pub extern "C" fn plugin_shutdown() {
            if let Some(results) = COMPLETED_RESULTS.get() {
                results.lock().unwrap().clear();
            }
            if let Some(cache) = ANALYSIS_CACHE.get() {
                cache.lock().unwrap().clear();
            }
        }
    };
}
