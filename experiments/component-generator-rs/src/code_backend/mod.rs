use std::collections::BTreeMap;

mod oxc;

#[cfg(feature = "swc")]
mod swc;

pub use oxc::OxcCodeBackend;

#[cfg(feature = "swc")]
pub use swc::SwcCodeBackend;

use crate::error::Result;

#[derive(Default)]
pub struct SplitModuleItems {
    pub imports: Vec<String>,
    pub rest: Vec<String>,
}

pub trait JsCodeBackend {
    fn rename_expr_idents(
        &self,
        code: &str,
        replacements: &BTreeMap<String, String>,
    ) -> Result<String>;

    fn rename_module_item_idents(
        &self,
        code: &str,
        replacements: &BTreeMap<String, String>,
    ) -> Result<String>;

    fn split_imports(&self, code: &str) -> Result<SplitModuleItems>;

    fn emit_module(
        &self,
        imports: &[String],
        module_items: &[String],
        export_default: Option<&str>,
    ) -> Result<String>;
}
