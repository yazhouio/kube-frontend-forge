use std::collections::BTreeMap;

use swc_ecma_ast::Ident;
use swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::ast::{emit_expr, emit_module, parse_expr, parse_module_items};
use crate::error::Result;

pub fn rename_module_item_idents(
    code: &str,
    replacements: &BTreeMap<String, String>,
) -> Result<String> {
    if replacements.is_empty() {
        return Ok(code.to_owned());
    }

    let mut items = parse_module_items(code)?;
    let mut renamer = IdentRenamer { replacements };
    items.visit_mut_with(&mut renamer);
    emit_module(items).map(|code| code.trim().to_owned())
}

pub fn rename_expr_idents(code: &str, replacements: &BTreeMap<String, String>) -> Result<String> {
    if replacements.is_empty() {
        return Ok(code.to_owned());
    }

    let mut expr = parse_expr(code)?;
    let mut renamer = IdentRenamer { replacements };
    expr.visit_mut_with(&mut renamer);
    emit_expr(&expr).map(|code| code.trim().to_owned())
}

struct IdentRenamer<'a> {
    replacements: &'a BTreeMap<String, String>,
}

impl VisitMut for IdentRenamer<'_> {
    fn visit_mut_ident(&mut self, ident: &mut Ident) {
        if let Some(replacement) = self.replacements.get(ident.sym.as_ref()) {
            ident.sym = replacement.clone().into();
        }
    }
}
