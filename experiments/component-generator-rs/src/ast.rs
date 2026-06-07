use swc_common::{DUMMY_SP, FileName, SourceMap, sync::Lrc};
use swc_ecma_ast::{EsVersion, Expr, Module, ModuleItem};
use swc_ecma_codegen::{Emitter, Node, text_writer::JsWriter};
use swc_ecma_parser::{Parser, StringInput, Syntax, TsSyntax, lexer::Lexer};

use crate::error::{Error, Result};

pub fn parse_expr(code: &str) -> Result<Expr> {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Anon.into(), code.to_owned());
    let lexer = Lexer::new(
        Syntax::Typescript(TsSyntax {
            tsx: true,
            ..Default::default()
        }),
        EsVersion::Es2022,
        StringInput::from(&*fm),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    parser
        .parse_expr()
        .map(|expr| *expr)
        .map_err(|err| Error::ParseExpression {
            code: code.to_owned(),
            message: format!("{err:?}"),
        })
}

pub fn parse_module_items(code: &str) -> Result<Vec<ModuleItem>> {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Anon.into(), code.to_owned());
    let lexer = Lexer::new(
        Syntax::Typescript(TsSyntax {
            tsx: true,
            ..Default::default()
        }),
        EsVersion::Es2022,
        StringInput::from(&*fm),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    parser
        .parse_module()
        .map(|module| module.body)
        .map_err(|err| Error::ParseModuleItem {
            code: code.to_owned(),
            message: format!("{err:?}"),
        })
}

pub fn emit_module(body: Vec<ModuleItem>) -> Result<String> {
    let cm: Lrc<SourceMap> = Default::default();
    let module = Module {
        span: DUMMY_SP,
        body,
        shebang: None,
    };
    let mut buf = Vec::new();
    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
        let mut emitter = Emitter {
            cfg: Default::default(),
            cm,
            comments: None,
            wr: writer,
        };
        emitter
            .emit_module(&module)
            .map_err(|source| Error::EmitModule {
                message: source.to_string(),
            })?;
    }
    String::from_utf8(buf).map_err(|source| Error::GeneratedUtf8 { source })
}

pub fn emit_expr(expr: &Expr) -> Result<String> {
    let cm: Lrc<SourceMap> = Default::default();
    let mut buf = Vec::new();
    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
        let mut emitter = Emitter {
            cfg: Default::default(),
            cm,
            comments: None,
            wr: writer,
        };
        expr.emit_with(&mut emitter)
            .map_err(|source| Error::EmitModule {
                message: source.to_string(),
            })?;
    }
    String::from_utf8(buf).map_err(|source| Error::GeneratedUtf8 { source })
}
