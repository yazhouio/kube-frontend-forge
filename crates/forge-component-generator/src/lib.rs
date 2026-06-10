pub mod action_graph;
pub mod builtins;
pub mod code_backend;
pub mod error;
pub mod generator;
pub mod imports;
pub mod model;
pub mod names;
pub mod registry;
pub mod schema;
pub mod value;

#[cfg(feature = "swc")]
pub use code_backend::SwcCodeBackend;
pub use code_backend::{JsCodeBackend, OxcCodeBackend};
pub use error::{Error, Result};
pub use generator::ComponentGenerator;
pub use model::{PageConfig, unwrap_page_schema};
pub use schema::component_tree_schema;
