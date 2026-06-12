use oxc_allocator::Allocator;
use oxc_ast::ast::{
    BindingIdentifier, CallExpression, Expression, IdentifierName, IdentifierReference,
    ImportExpression,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::SourceType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemJsValidation {
    pub has_system_register: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemJsValidationError {
    Parse { message: String },
    ForbiddenToken { token: &'static str },
}

impl std::fmt::Display for SystemJsValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse { message } => write!(f, "failed to parse JavaScript output: {message}"),
            Self::ForbiddenToken { token } => {
                write!(f, "JavaScript output contains forbidden token `{token}`")
            }
        }
    }
}

impl std::error::Error for SystemJsValidationError {}

pub fn validate_systemjs_code(code: &str) -> Result<SystemJsValidation, SystemJsValidationError> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, code, SourceType::script()).parse();
    if !parsed.errors.is_empty() || parsed.panicked {
        return Err(SystemJsValidationError::Parse {
            message: format_oxc_errors(&parsed.errors, parsed.panicked),
        });
    }

    let mut visitor = SystemJsVisitor::default();
    visitor.visit_program(&parsed.program);
    if let Some(token) = visitor.forbidden_token {
        return Err(SystemJsValidationError::ForbiddenToken { token });
    }

    Ok(SystemJsValidation {
        has_system_register: visitor.has_system_register,
    })
}

#[derive(Default)]
struct SystemJsVisitor {
    has_system_register: bool,
    forbidden_token: Option<&'static str>,
}

impl SystemJsVisitor {
    fn check_ident(&mut self, value: &str) {
        if self.forbidden_token.is_some() {
            return;
        }
        if value == "__webpack_require__" {
            self.forbidden_token = Some("__webpack_require__");
        } else if value.starts_with("webpackChunk") {
            self.forbidden_token = Some("webpackChunk");
        }
    }

    fn forbid_dynamic_import(&mut self) {
        if self.forbidden_token.is_none() {
            self.forbidden_token = Some("import(");
        }
    }
}

impl<'a> Visit<'a> for SystemJsVisitor {
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if is_system_register_callee(&it.callee) {
            self.has_system_register = true;
        }
        walk::walk_call_expression(self, it);
    }

    fn visit_import_expression(&mut self, it: &ImportExpression<'a>) {
        self.forbid_dynamic_import();
        walk::walk_import_expression(self, it);
    }

    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        self.check_ident(it.name.as_str());
    }

    fn visit_identifier_name(&mut self, it: &IdentifierName<'a>) {
        self.check_ident(it.name.as_str());
    }

    fn visit_binding_identifier(&mut self, it: &BindingIdentifier<'a>) {
        self.check_ident(it.name.as_str());
    }
}

fn is_system_register_callee(expr: &Expression<'_>) -> bool {
    let Expression::StaticMemberExpression(member) = expr else {
        return false;
    };
    member.property.name.as_str() == "register" && is_identifier(&member.object, "System")
}

fn is_identifier(expr: &Expression<'_>, name: &str) -> bool {
    matches!(expr, Expression::Identifier(ident) if ident.name.as_str() == name)
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
    use super::{SystemJsValidationError, validate_systemjs_code};

    #[test]
    fn finds_system_register_calls() {
        let validation = validate_systemjs_code(
            r#"System.register("x", [], function () { return { execute() {} }; });"#,
        )
        .unwrap();

        assert!(validation.has_system_register);
    }

    #[test]
    fn ignores_system_register_in_strings() {
        let validation = validate_systemjs_code(r#"const value = "System.register";"#).unwrap();

        assert!(!validation.has_system_register);
    }

    #[test]
    fn js_comment_import_type_does_not_count_as_dynamic_import() {
        validate_systemjs_code(
            "System.register('x', [], function () {});\n/** @type {import('./types').Thing} */\nconst value = 1;",
        )
        .unwrap();
    }

    #[test]
    fn string_url_does_not_hide_dynamic_import() {
        let error = validate_systemjs_code(
            r#"System.register("x", [], function () {}); const url = "https://example.com"; import("./next");"#,
        )
        .unwrap_err();

        assert_eq!(
            error,
            SystemJsValidationError::ForbiddenToken { token: "import(" }
        );
    }

    #[test]
    fn template_expression_dynamic_import_is_forbidden() {
        let error = validate_systemjs_code(
            r#"System.register("x", [], function () {}); const value = `${import("./next")}`;"#,
        )
        .unwrap_err();

        assert_eq!(
            error,
            SystemJsValidationError::ForbiddenToken { token: "import(" }
        );
    }

    #[test]
    fn webpack_runtime_identifiers_are_forbidden() {
        let require_error = validate_systemjs_code(
            r#"System.register("x", [], function () {}); __webpack_require__(1);"#,
        )
        .unwrap_err();
        let chunk_error = validate_systemjs_code(
            r#"System.register("x", [], function () {}); self.webpackChunkapp = self.webpackChunkapp || [];"#,
        )
        .unwrap_err();

        assert_eq!(
            require_error,
            SystemJsValidationError::ForbiddenToken {
                token: "__webpack_require__"
            }
        );
        assert_eq!(
            chunk_error,
            SystemJsValidationError::ForbiddenToken {
                token: "webpackChunk"
            }
        );
    }
}
