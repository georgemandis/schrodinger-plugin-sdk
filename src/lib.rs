pub mod host_api;
pub mod types;

pub mod prelude {
    pub use crate::types::{
        AnalysisData, ClipEntry, EntryMeta, Output, OutputBuilder, PluginError,
    };
}
