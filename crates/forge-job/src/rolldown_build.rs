use std::{
    borrow::Cow,
    env,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use forge_core::BuildPlan;
use forge_project_generator::{
    EXTERNAL_PACKAGES, USE_SYNC_EXTERNAL_STORE_SHIM_SOURCE,
    USE_SYNC_EXTERNAL_STORE_WITH_SELECTOR_SOURCE,
};
use rolldown::{
    BundlerOptions, CodeSplittingMode, InputItem, IsExternal, LegalComments, ModuleType,
    OutputFormat, Platform, RawMinifyOptions,
    plugin::{
        HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
        HookResolveIdReturn, HookUsage, HookWriteBundleArgs, Plugin, PluginContext,
        SharedLoadPluginContext,
    },
};
use swc_common::{FileName, GLOBALS, Globals, Mark, SourceMap, sync::Lrc};
use swc_ecma_ast::{EsVersion, Program};
use swc_ecma_codegen::{Emitter, text_writer::JsWriter};
use swc_ecma_parser::{Parser, StringInput, Syntax, lexer::Lexer};
use swc_ecma_transforms_base::{fixer::fixer, resolver};
use swc_ecma_transforms_module::{path::Resolver, system_js};

pub const ROLLDOWN_VERSION: &str = "1.1.1";

const ASSET_EXTENSIONS: &[&str] = &[
    ".avif", ".gif", ".jpg", ".jpeg", ".png", ".svg", ".webp", ".woff", ".woff2", ".ttf", ".eot",
];

const STYLE_FILE_NAME: &str = "style.css";

pub async fn build_project(project_dir: &Path, plan: &BuildPlan) -> Result<(), String> {
    let dist_dir = project_dir.join(&plan.expectations.dist_dir);
    remove_dir_if_exists(&dist_dir).await.map_err(|source| {
        format!(
            "failed to remove rolldown dist directory {}: {source}",
            dist_dir.display()
        )
    })?;

    let css_collector = CssCollector::default();
    let mut bundler = rolldown::Bundler::with_plugins(
        bundler_options(project_dir, plan)?,
        vec![
            Arc::new(ForgeResolvePlugin::new(project_dir)?),
            Arc::new(css_collector.clone()),
        ],
    )
    .map_err(format_build_error)?;

    bundler.write().await.map_err(format_build_error)?;
    bundler.close().await.map_err(format_build_error)?;

    if plan.expectations.systemjs {
        convert_dist_to_systemjs(&dist_dir).await?;
    }

    Ok(())
}

fn bundler_options(project_dir: &Path, plan: &BuildPlan) -> Result<BundlerOptions, String> {
    let dist_dir = path_string(&project_dir.join(&plan.expectations.dist_dir))?;
    Ok(BundlerOptions {
        input: Some(vec![InputItem {
            name: Some("main".to_owned()),
            import: plan.entry.clone(),
        }]),
        cwd: Some(project_dir.to_path_buf()),
        external: Some(external_packages()),
        platform: Some(Platform::Browser),
        entry_filenames: Some("index.js".to_owned().into()),
        chunk_filenames: Some("assets/[name]-[hash].js".to_owned().into()),
        asset_filenames: Some("assets/[name]-[hash][extname]".to_owned().into()),
        dir: Some(dist_dir),
        format: Some(OutputFormat::Esm),
        module_types: Some(
            ASSET_EXTENSIONS
                .iter()
                .map(|ext| ((*ext).to_owned(), ModuleType::Asset))
                .collect(),
        ),
        minify: Some(RawMinifyOptions::Bool(true)),
        define: Some(
            [
                (
                    "process.env.NODE_ENV".to_owned(),
                    "\"production\"".to_owned(),
                ),
                (
                    "import.meta.env".to_owned(),
                    "({ MODE: \"production\", DEV: false, PROD: true, SSR: false })".to_owned(),
                ),
                (
                    "import.meta.env.MODE".to_owned(),
                    "\"production\"".to_owned(),
                ),
                ("import.meta.env.DEV".to_owned(), "false".to_owned()),
                ("import.meta.env.PROD".to_owned(), "true".to_owned()),
                ("import.meta.env.SSR".to_owned(), "false".to_owned()),
            ]
            .into_iter()
            .collect(),
        ),
        code_splitting: Some(CodeSplittingMode::Bool(true)),
        legal_comments: Some(LegalComments::None),
        transform: Some(rolldown::BundlerTransformOptions {
            jsx: Some(rolldown::Either::Right(rolldown::JsxOptions {
                runtime: Some("classic".to_owned()),
                throw_if_namespace: Some(true),
                pragma: Some("React.createElement".to_owned()),
                pragma_frag: Some("React.Fragment".to_owned()),
                ..Default::default()
            })),
            target: Some(rolldown::Either::Left("es2022".to_owned())),
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn external_packages() -> IsExternal {
    let packages = Arc::new(
        EXTERNAL_PACKAGES
            .iter()
            .map(|package| (*package).to_owned())
            .collect::<Vec<_>>(),
    );
    IsExternal::Fn(Some(Arc::new(move |request, _importer, _is_resolved| {
        let packages = Arc::clone(&packages);
        let request = request.to_owned();
        Box::pin(async move {
            Ok(packages
                .iter()
                .any(|package| request == *package || request.starts_with(&format!("{package}/"))))
        })
    })))
}

#[derive(Debug)]
struct ForgeResolvePlugin {
    forge_components_source: Option<String>,
}

impl ForgeResolvePlugin {
    fn new(project_dir: &Path) -> Result<Self, String> {
        let forge_components_source = project_dir
            .join("node_modules")
            .join("@frontend-forge")
            .join("forge-components")
            .join("src")
            .join("index.ts");
        let forge_components_source = (is_forge_dev_mode() && forge_components_source.is_file())
            .then(|| path_string(&forge_components_source))
            .transpose()?;
        Ok(Self {
            forge_components_source,
        })
    }
}

impl Plugin for ForgeResolvePlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("frontend-forge-resolve")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ResolveId | HookUsage::Load
    }

    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        args: &HookResolveIdArgs<'_>,
    ) -> HookResolveIdReturn {
        if args.specifier == "@frontend-forge/forge-components"
            && let Some(path) = &self.forge_components_source
        {
            return Ok(Some(HookResolveIdOutput::from_id(path.clone())));
        }

        if react_cjs_shim_source(args.specifier).is_some() {
            return Ok(Some(HookResolveIdOutput::from_id(virtual_shim_id(
                args.specifier,
            ))));
        }

        Ok(None)
    }

    async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
        let Some(specifier) = args.id.strip_prefix(VIRTUAL_SHIM_PREFIX) else {
            return Ok(None);
        };
        let Some(source) = react_cjs_shim_source(specifier) else {
            return Ok(None);
        };
        Ok(Some(HookLoadOutput {
            code: source.into(),
            module_type: Some(ModuleType::Js),
            ..Default::default()
        }))
    }
}

const VIRTUAL_SHIM_PREFIX: &str = "\0frontend-forge/react-cjs-shim:";

fn virtual_shim_id(specifier: &str) -> String {
    format!("{VIRTUAL_SHIM_PREFIX}{specifier}")
}

fn react_cjs_shim_source(specifier: &str) -> Option<&'static str> {
    match specifier {
        "use-sync-external-store"
        | "use-sync-external-store/index.js"
        | "use-sync-external-store/shim"
        | "use-sync-external-store/shim/index.js" => Some(USE_SYNC_EXTERNAL_STORE_SHIM_SOURCE),
        "use-sync-external-store/with-selector"
        | "use-sync-external-store/with-selector.js"
        | "use-sync-external-store/shim/with-selector"
        | "use-sync-external-store/shim/with-selector.js" => {
            Some(USE_SYNC_EXTERNAL_STORE_WITH_SELECTOR_SOURCE)
        }
        _ => None,
    }
}

#[derive(Clone, Debug, Default)]
struct CssCollector {
    modules: Arc<Mutex<Vec<CssModule>>>,
}

#[derive(Debug)]
struct CssModule {
    content: String,
}

impl Plugin for CssCollector {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("frontend-forge-css-extract")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Load | HookUsage::WriteBundle
    }

    async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
        let Some(path) = css_path(args.id) else {
            return Ok(None);
        };
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|source| anyhow::anyhow!("failed to read css module {path}: {source}"))?;
        self.modules
            .lock()
            .expect("css collector mutex must not be poisoned")
            .push(CssModule { content });
        Ok(Some(HookLoadOutput {
            code: String::new().into(),
            module_type: Some(ModuleType::Js),
            ..Default::default()
        }))
    }

    async fn write_bundle(
        &self,
        _ctx: &PluginContext,
        args: &mut HookWriteBundleArgs<'_>,
    ) -> anyhow::Result<()> {
        let css = {
            let modules = self
                .modules
                .lock()
                .expect("css collector mutex must not be poisoned");
            if modules.is_empty() {
                return Ok(());
            }

            let mut css = String::new();
            for module in modules.iter() {
                css.push_str(&module.content);
                if !css.ends_with('\n') {
                    css.push('\n');
                }
            }
            css
        };

        let style_path = Path::new(&args.options.out_dir).join(STYLE_FILE_NAME);
        tokio::fs::write(&style_path, css).await.map_err(|source| {
            anyhow::anyhow!(
                "failed to write css bundle {}: {source}",
                style_path.display()
            )
        })?;
        Ok(())
    }
}

fn css_path(path: &str) -> Option<&str> {
    let clean = path
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(path)
        .split_once('#')
        .map(|(path, _)| path)
        .unwrap_or(path);
    Path::new(clean)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("css"))
        .then_some(clean)
}

async fn convert_dist_to_systemjs(dist_dir: &Path) -> Result<(), String> {
    for path in collect_js_files(dist_dir).await? {
        let code = tokio::fs::read_to_string(&path)
            .await
            .map_err(|source| format!("failed to read JavaScript {}: {source}", path.display()))?;
        let systemjs = to_systemjs(code, path.clone()).await?;
        tokio::fs::write(&path, systemjs)
            .await
            .map_err(|source| format!("failed to write SystemJS {}: {source}", path.display()))?;
    }
    Ok(())
}

async fn to_systemjs(code: String, path: PathBuf) -> Result<String, String> {
    let display_path = path.display().to_string();
    tokio::task::spawn_blocking(move || {
        GLOBALS.set(&Globals::new(), || to_systemjs_inner(&code, &path))
    })
    .await
    .map_err(|source| format!("SystemJS transform task failed for {display_path}: {source}"))?
}

fn to_systemjs_inner(code: &str, path: &Path) -> Result<String, String> {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Real(path.to_path_buf()).into(), code.to_owned());
    let lexer = Lexer::new(
        Syntax::Es(Default::default()),
        EsVersion::Es2022,
        StringInput::from(&*fm),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    let module = parser.parse_module().map_err(|error| {
        format!(
            "failed to parse rolldown output {} before SystemJS transform: {error:?}",
            path.display()
        )
    })?;
    let mut program = Program::Module(module);
    let unresolved_mark = Mark::new();
    let top_level_mark = Mark::new();
    program.mutate(resolver(unresolved_mark, top_level_mark, false));
    program.mutate(system_js(
        Resolver::Default,
        unresolved_mark,
        Default::default(),
    ));
    program.mutate(fixer(None));
    emit_program(cm, &program)
}

fn emit_program(cm: Lrc<SourceMap>, program: &Program) -> Result<String, String> {
    let mut buf = Vec::new();
    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
        let mut emitter = Emitter {
            cfg: swc_ecma_codegen::Config::default()
                .with_target(EsVersion::Es2022)
                .with_minify(true),
            cm,
            comments: None,
            wr: writer,
        };
        emitter
            .emit_program(program)
            .map_err(|source| format!("failed to emit SystemJS output: {source}"))?;
    }
    String::from_utf8(buf).map_err(|source| format!("SystemJS output is not UTF-8: {source}"))
}

async fn collect_js_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir_path) = stack.pop() {
        let mut dir = tokio::fs::read_dir(&dir_path).await.map_err(|source| {
            format!("failed to read directory {}: {source}", dir_path.display())
        })?;
        while let Some(entry) = dir.next_entry().await.map_err(|source| {
            format!(
                "failed to read directory entry in {}: {source}",
                dir_path.display()
            )
        })? {
            let path = entry.path();
            let file_type = entry.file_type().await.map_err(|source| {
                format!("failed to read file type for {}: {source}", path.display())
            })?;
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "js") {
                files.push(path);
            }
        }
    }

    files.sort();
    Ok(files)
}

fn is_forge_dev_mode() -> bool {
    env_truthy("FORGE_DEV_MODE") || env::var("NODE_ENV").is_ok_and(|value| value == "development")
}

fn env_truthy(name: &str) -> bool {
    env::var(name).is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on" | "dev" | "development"
        )
    })
}

fn path_string(path: &Path) -> Result<String, String> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        format!(
            "rolldown requires UTF-8 paths, got {}",
            path.to_string_lossy()
        )
    })
}

fn format_build_error(source: impl std::fmt::Display) -> String {
    source.to_string()
}

async fn remove_dir_if_exists(path: &Path) -> std::io::Result<()> {
    match tokio::fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
