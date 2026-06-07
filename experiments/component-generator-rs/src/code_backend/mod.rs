use std::collections::BTreeMap;

mod oxc;

#[cfg(feature = "swc")]
mod swc;
#[cfg(feature = "swc")]
mod swc_ast;
#[cfg(feature = "swc")]
mod swc_rename;

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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    #[cfg(feature = "swc")]
    use super::SwcCodeBackend;
    use super::{JsCodeBackend, OxcCodeBackend};

    fn compact_code(value: &str) -> String {
        value.chars().filter(|ch| !ch.is_whitespace()).collect()
    }

    fn assert_code_contains(code: &str, expected: &str) {
        if code.contains(expected) {
            return;
        }
        assert!(
            compact_code(code).contains(&compact_code(expected)),
            "expected generated code to contain `{expected}`\n\n{code}"
        );
    }

    fn assert_backend_contract(backend: impl JsCodeBackend) {
        let replacements = BTreeMap::from([("value".to_owned(), "renamedValue".to_owned())]);

        let module = backend
            .rename_module_item_idents(
                "const value = 1; function read() { return value; }",
                &replacements,
            )
            .unwrap();
        assert_code_contains(&module, "const renamedValue = 1");
        assert_code_contains(&module, "return renamedValue");

        let expr = backend
            .rename_expr_idents("<div>{value}</div>", &replacements)
            .unwrap();
        assert_code_contains(&expr, "{renamedValue}");

        let split = backend
            .split_imports(r#"import * as React from "react"; const local = 1;"#)
            .unwrap();
        assert_eq!(split.imports.len(), 1);
        assert_eq!(split.rest.len(), 1);
        assert_code_contains(&split.imports[0], r#"from "react""#);
        assert_code_contains(&split.rest[0], "const local = 1");

        let module = backend
            .emit_module(
                &[r#"import * as React from "react";"#.to_owned()],
                &[
                    "const title = \"Hello\";".to_owned(),
                    "function SamplePage() { return <div>{title}</div>; }".to_owned(),
                ],
                Some("SamplePage"),
            )
            .unwrap();
        assert_code_contains(&module, r#"import * as React from "react""#);
        assert_code_contains(&module, "function SamplePage()");
        assert_code_contains(&module, "export default SamplePage");
    }

    #[test]
    fn oxc_backend_satisfies_contract() {
        assert_backend_contract(OxcCodeBackend);
    }

    #[cfg(feature = "swc")]
    #[test]
    fn swc_backend_satisfies_contract() {
        assert_backend_contract(SwcCodeBackend);
    }
}
