use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use forge_core::BuildPlan;
use forge_project_generator::EXTERNAL_PACKAGES;
use rspack::builder::{
    Builder as _, ExperimentsBuilder, ModuleOptionsBuilder, OutputOptionsBuilder,
};
use rspack_core::{
    CacheOptions, Compiler, ExternalItem, ExternalItemFnResult, ExternalItemValue, Filename,
    LibraryOptions, Mode, ModuleOptions, ModuleRule, ModuleRuleEffect, ModuleRuleUse,
    ModuleRuleUseLoader, PublicPath, Resolve, RuleSetCondition,
};
use rspack_paths::Utf8PathBuf;
use rspack_plugin_javascript::define_plugin::{DefinePlugin, DefineValue};
use rspack_regex::RspackRegex;
use serde_json::{Value, json};

pub const RSPACK_VERSION: &str = "0.100.8";

pub async fn build_project(project_dir: &Path, plan: &BuildPlan) -> Result<(), String> {
    let dist_dir = project_dir.join(&plan.expectations.dist_dir);
    remove_dir_if_exists(&dist_dir).map_err(|source| {
        format!(
            "failed to remove rspack dist directory {}: {source}",
            dist_dir.display()
        )
    })?;

    let mut compiler = compiler(project_dir, plan)?;
    compiler
        .build()
        .await
        .map_err(|source| source.to_string())?;

    let errors = compiler
        .compilation
        .get_errors()
        .map(|diagnostic| {
            diagnostic
                .render_report(false)
                .unwrap_or_else(|_| format!("{:?}", diagnostic.error))
        })
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        return Err(format!("rspack compilation failed:\n{}", errors.join("\n")));
    }

    Ok(())
}

fn compiler(project_dir: &Path, plan: &BuildPlan) -> Result<Compiler, String> {
    let context = utf8_path(project_dir)?;
    let dist_dir = utf8_path(&project_dir.join(&plan.expectations.dist_dir))?;
    let mut output = OutputOptionsBuilder::default();
    output
        .path(dist_dir)
        .filename(Filename::from("index.js"))
        .chunk_filename(Filename::from("assets/[name]-[contenthash].js"))
        .asset_module_filename(Filename::from("assets/[name]-[contenthash][ext]"))
        .css_filename(Filename::from("style.css"))
        .css_chunk_filename(Filename::from("assets/[name]-[contenthash].css"))
        .public_path(PublicPath::from("auto".to_owned()));

    if plan.expectations.systemjs {
        output.library(LibraryOptions {
            name: None,
            export: None,
            library_type: "system".to_owned(),
            umd_named_define: None,
            auxiliary_comment: None,
            amd_container: None,
        });
    } else {
        output.module(true).iife(false);
    }

    let mut builder = Compiler::builder();
    builder
        .context(context)
        .target(vec!["web".to_owned()])
        .mode(Mode::Production)
        .cache(CacheOptions::Disabled)
        .entry("main", format!("./{}", plan.entry))
        .resolve(Resolve {
            extensions: Some(vec![
                ".tsx".to_owned(),
                ".ts".to_owned(),
                ".jsx".to_owned(),
                ".js".to_owned(),
                ".json".to_owned(),
            ]),
            ..Default::default()
        })
        .output(output)
        .module(tsx_module_options()?)
        .experiments(ExperimentsBuilder::default().css(true))
        .externals(external_packages())
        .externals_type(if plan.expectations.systemjs {
            "system".to_owned()
        } else {
            "module-import".to_owned()
        })
        .enable_loader_swc()
        .plugin(Box::new(DefinePlugin::new(define_plugin_values())));

    builder.build().map_err(|source| source.to_string())
}

fn define_plugin_values() -> DefineValue {
    let production = json!("production").to_string();
    let mut import_meta_env = serde_json::Map::new();
    import_meta_env.insert("MODE".to_owned(), Value::String(production.clone()));
    import_meta_env.insert("DEV".to_owned(), Value::Bool(false));
    import_meta_env.insert("PROD".to_owned(), Value::Bool(true));
    import_meta_env.insert("SSR".to_owned(), Value::Bool(false));

    let mut values = DefineValue::default();
    values.insert(
        "process.env.NODE_ENV".to_owned(),
        Value::String(production.clone()),
    );
    values.insert("import.meta.env".to_owned(), Value::Object(import_meta_env));
    values.insert("import.meta.env.MODE".to_owned(), Value::String(production));
    values.insert("import.meta.env.DEV".to_owned(), Value::Bool(false));
    values.insert("import.meta.env.PROD".to_owned(), Value::Bool(true));
    values.insert("import.meta.env.SSR".to_owned(), Value::Bool(false));
    values
}

fn tsx_module_options() -> Result<ModuleOptionsBuilder, String> {
    let mut module = ModuleOptions::builder();
    module.rule(ModuleRule {
        test: Some(RuleSetCondition::Regexp(regex(r"\.tsx?$")?)),
        effect: ModuleRuleEffect {
            r#use: ModuleRuleUse::Array(vec![ModuleRuleUseLoader {
                loader: "builtin:swc-loader".to_owned(),
                options: Some(
                    json!({
                        "jsc": {
                            "parser": {
                                "syntax": "typescript",
                                "tsx": true
                            },
                            "target": "es2022",
                            "transform": {
                                "react": {
                                    "runtime": "classic",
                                    "pragma": "React.createElement",
                                    "pragmaFrag": "React.Fragment",
                                    "throwIfNamespace": true,
                                    "useBuiltins": false
                                }
                            }
                        },
                        "module": {
                            "type": "es6"
                        }
                    })
                    .to_string(),
                ),
            }]),
            ..Default::default()
        },
        ..Default::default()
    });
    Ok(module)
}

fn external_packages() -> ExternalItem {
    let packages = Arc::new(
        EXTERNAL_PACKAGES
            .iter()
            .map(|package| (*package).to_owned())
            .collect::<Vec<_>>(),
    );
    ExternalItem::Fn(Box::new(move |ctx| {
        let packages = Arc::clone(&packages);
        Box::pin(async move {
            let request = ctx.request;
            let is_external = packages
                .iter()
                .any(|package| request == *package || request.starts_with(&format!("{package}/")));
            Ok(ExternalItemFnResult {
                external_type: None,
                result: is_external.then_some(ExternalItemValue::String(request)),
            })
        })
    }))
}

fn regex(pattern: &str) -> Result<RspackRegex, String> {
    RspackRegex::new(pattern).map_err(|source| source.to_string())
}

fn utf8_path(path: &Path) -> Result<Utf8PathBuf, String> {
    Utf8PathBuf::from_path_buf(PathBuf::from(path)).map_err(|path| {
        format!(
            "rspack requires UTF-8 paths, got {}",
            path.to_string_lossy()
        )
    })
}

fn remove_dir_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
