use std::collections::BTreeMap;

use swc_ecma_ast::{ModuleDecl, ModuleItem};

use super::swc_ast::{
    emit_module as swc_emit_module, parse_expr as swc_parse_expr,
    parse_module_items as swc_parse_module_items,
};
use super::swc_rename::{
    rename_expr_idents as swc_rename_expr_idents,
    rename_module_item_idents as swc_rename_module_item_idents,
};
use super::{JsCodeBackend, SplitModuleItems};
use crate::error::Result;

#[derive(Clone, Copy, Default)]
pub struct SwcCodeBackend;

impl JsCodeBackend for SwcCodeBackend {
    fn validate_expr(&self, code: &str) -> Result<()> {
        swc_parse_expr(code).map(|_| ())
    }

    fn validate_module_items(&self, code: &str) -> Result<()> {
        swc_parse_module_items(code).map(|_| ())
    }

    fn rename_expr_idents(
        &self,
        code: &str,
        replacements: &BTreeMap<String, String>,
    ) -> Result<String> {
        swc_rename_expr_idents(code, replacements)
    }

    fn rename_module_item_idents(
        &self,
        code: &str,
        replacements: &BTreeMap<String, String>,
    ) -> Result<String> {
        swc_rename_module_item_idents(code, replacements)
    }

    fn split_imports(&self, code: &str) -> Result<SplitModuleItems> {
        let mut split = SplitModuleItems::default();
        for item in swc_parse_module_items(code)? {
            let code = swc_emit_module(vec![item.clone()])?.trim().to_owned();
            if matches!(&item, ModuleItem::ModuleDecl(ModuleDecl::Import(_))) {
                split.imports.push(code);
            } else {
                split.rest.push(code);
            }
        }
        Ok(split)
    }

    fn emit_module(
        &self,
        imports: &[String],
        module_items: &[String],
        export_default: Option<&str>,
    ) -> Result<String> {
        let mut body = Vec::<ModuleItem>::new();
        for import in imports {
            body.extend(swc_parse_module_items(import)?);
        }
        for item in module_items {
            body.extend(swc_parse_module_items(item)?);
        }
        if let Some(export_default) = export_default {
            body.extend(swc_parse_module_items(&format!(
                "export default {export_default};"
            ))?);
        }
        swc_emit_module(body)
    }
}
