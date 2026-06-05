pub mod ffi;
pub mod host_api;
pub mod task;
pub mod types;

pub mod prelude {
    pub use crate::ffi::NativePlugin;
    pub use crate::types::{
        AnalysisData, ClipEntry, EntryMeta, Output, OutputBuilder, PluginError,
    };
}
