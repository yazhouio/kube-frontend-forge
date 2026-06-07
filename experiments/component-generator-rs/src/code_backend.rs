use std::collections::BTreeMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    BindingIdentifier, Expression, IdentifierName, IdentifierReference, Program, Statement,
};
use oxc_ast_visit::VisitMut;
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use swc_ecma_ast::{ModuleDecl, ModuleItem};

use crate::ast::{emit_module as swc_emit_module, parse_module_items as swc_parse_module_items};
use crate::error::Result;
use crate::rename::{
    rename_expr_idents as swc_rename_expr_idents,
    rename_module_item_idents as swc_rename_module_item_idents,
};

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

#[derive(Clone, Copy, Default)]
pub struct SwcCodeBackend;

impl JsCodeBackend for SwcCodeBackend {
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

#[derive(Clone, Copy, Default)]
pub struct OxcCodeBackend;

impl JsCodeBackend for OxcCodeBackend {
    fn rename_expr_idents(
        &self,
        code: &str,
        replacements: &BTreeMap<String, String>,
    ) -> Result<String> {
        if replacements.is_empty() {
            return Ok(code.to_owned());
        }

        with_oxc_expression(code, |allocator, expr| {
            let mut renamer = OxcIdentRenamer {
                allocator,
                replacements,
            };
            renamer.visit_expression(expr);
            emit_oxc_expression(expr)
        })
    }

    fn rename_module_item_idents(
        &self,
        code: &str,
        replacements: &BTreeMap<String, String>,
    ) -> Result<String> {
        if replacements.is_empty() {
            return Ok(code.to_owned());
        }

        with_oxc_program(code, |allocator, program| {
            let mut renamer = OxcIdentRenamer {
                allocator,
                replacements,
            };
            renamer.visit_program(program);
            Ok(emit_oxc_program(program))
        })
    }

    fn split_imports(&self, code: &str) -> Result<SplitModuleItems> {
        with_oxc_program(code, |_allocator, program| {
            let mut split = SplitModuleItems::default();
            for statement in &program.body {
                let span = statement.span();
                let code = code
                    .get(span.start as usize..span.end as usize)
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                if is_oxc_import_statement(statement) {
                    split.imports.push(code);
                } else {
                    split.rest.push(code);
                }
            }
            Ok(split)
        })
    }

    fn emit_module(
        &self,
        imports: &[String],
        module_items: &[String],
        export_default: Option<&str>,
    ) -> Result<String> {
        let mut code = String::new();
        for import in imports {
            code.push_str(import);
            code.push('\n');
        }
        for item in module_items {
            code.push_str(item);
            code.push('\n');
        }
        if let Some(export_default) = export_default {
            code.push_str(&format!("export default {export_default};\n"));
        }
        with_oxc_program(&code, |_allocator, program| Ok(emit_oxc_program(program)))
    }
}

struct OxcIdentRenamer<'a, 'r> {
    allocator: &'a Allocator,
    replacements: &'r BTreeMap<String, String>,
}

impl<'a> VisitMut<'a> for OxcIdentRenamer<'a, '_> {
    fn visit_identifier_name(&mut self, ident: &mut IdentifierName<'a>) {
        if let Some(replacement) = self.replacements.get(ident.name.as_str()) {
            ident.name = self.allocator.alloc_str(replacement).into();
        }
    }

    fn visit_identifier_reference(&mut self, ident: &mut IdentifierReference<'a>) {
        if let Some(replacement) = self.replacements.get(ident.name.as_str()) {
            ident.name = self.allocator.alloc_str(replacement).into();
        }
    }

    fn visit_binding_identifier(&mut self, ident: &mut BindingIdentifier<'a>) {
        if let Some(replacement) = self.replacements.get(ident.name.as_str()) {
            ident.name = self.allocator.alloc_str(replacement).into();
        }
    }
}

fn with_oxc_program<T>(
    code: &str,
    op: impl for<'a> FnOnce(&'a Allocator, &mut Program<'a>) -> Result<T>,
) -> Result<T> {
    let allocator = Allocator::default();
    let source_type = SourceType::tsx();
    let mut parsed = Parser::new(&allocator, code, source_type).parse();
    if !parsed.errors.is_empty() || parsed.panicked {
        return Err(crate::error::Error::ParseModuleItem {
            code: code.to_owned(),
            message: format_oxc_errors(&parsed.errors, parsed.panicked),
        });
    }
    op(&allocator, &mut parsed.program)
}

fn with_oxc_expression<T>(
    code: &str,
    op: impl for<'a> FnOnce(&'a Allocator, &mut Expression<'a>) -> Result<T>,
) -> Result<T> {
    let allocator = Allocator::default();
    let source_type = SourceType::tsx();
    let mut expr = Parser::new(&allocator, code, source_type)
        .parse_expression()
        .map_err(|errors| crate::error::Error::ParseExpression {
            code: code.to_owned(),
            message: format_oxc_errors(&errors, false),
        })?;
    op(&allocator, &mut expr)
}

fn emit_oxc_program(program: &Program<'_>) -> String {
    Codegen::new().build(program).code
}

fn emit_oxc_expression(expr: &Expression<'_>) -> Result<String> {
    let mut codegen = Codegen::new();
    codegen.print_expression(expr);
    Ok(codegen.into_source_text())
}

fn is_oxc_import_statement(statement: &Statement<'_>) -> bool {
    matches!(statement, Statement::ImportDeclaration(_))
}

fn format_oxc_errors(errors: &[impl std::fmt::Display], panicked: bool) -> String {
    if errors.is_empty() {
        return if panicked {
            "parser panicked".to_owned()
        } else {
            "unknown parser error".to_owned()
        };
    }
    let mut message = errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ");
    if panicked {
        message.push_str("; parser panicked");
    }
    message
}

#[cfg(test)]
mod tests {
    use super::{JsCodeBackend, OxcCodeBackend};

    #[test]
    fn oxc_backend_renames_module_and_expression_identifiers() {
        let backend = OxcCodeBackend;
        let replacements = [("value".to_owned(), "value2".to_owned())]
            .into_iter()
            .collect();

        let module = backend
            .rename_module_item_idents("const value = 1; const next = value + 1;", &replacements)
            .unwrap();
        let expr = backend
            .rename_expr_idents("<div>{value}</div>", &replacements)
            .unwrap();

        assert!(module.contains("const value2 = 1"));
        assert!(module.contains("const next = value2 + 1"));
        assert!(expr.contains("{value2}"));
    }

    #[test]
    fn oxc_backend_splits_imports_from_module_items() {
        let backend = OxcCodeBackend;
        let split = backend
            .split_imports(r#"import { useState } from "react"; const value = 1;"#)
            .unwrap();

        assert_eq!(split.imports.len(), 1);
        assert_eq!(split.rest.len(), 1);
        assert!(split.imports[0].contains("from \"react\""));
        assert!(split.rest[0].contains("const value = 1"));
    }
}
