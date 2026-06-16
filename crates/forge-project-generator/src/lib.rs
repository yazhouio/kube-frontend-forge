mod error;
mod runtime_contract;
mod schema;
mod shims;
mod types;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::LazyLock,
    time::Instant,
};

use include_dir::{Dir, include_dir};
use oxc_allocator::Allocator;
use oxc_ast::ast::Statement;
use oxc_parser::Parser;
use oxc_span::SourceType;
use regex::Regex;
use serde_json::Value;

pub use error::{Error, Result};
pub use runtime_contract::{
    BuildConfigSummary, EXTERNAL_PACKAGES, PackageDependency, TreeShakingSummary,
    build_config_summary,
};
pub use schema::manifest_schema;
pub use shims::{
    USE_SYNC_EXTERNAL_STORE_SHIM_SOURCE, USE_SYNC_EXTERNAL_STORE_WITH_SELECTOR_SOURCE,
};
pub use types::{
    BuildFormat, BuildMeta, ExtensionManifest, GenerateProjectFilesOptions,
    GenerateProjectFilesResult, LocaleMeta, ManifestEnvelope, MenuMeta, PageMeta, RouteMeta,
    VirtualFile,
};

static SCAFFOLD_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/scaffold");
static SCAFFOLD_CACHE: LazyLock<ScaffoldCache> =
    LazyLock::new(|| build_scaffold_cache().expect("bundled forge project scaffold must be valid"));

struct ScaffoldCache {
    files: Vec<VirtualFile>,
    templates: BTreeMap<String, String>,
    default_locales: Vec<LocaleMeta>,
}

pub type PageRenderer<'a> = dyn Fn(&PageMeta, &ExtensionManifest) -> Result<String> + 'a;

pub fn unwrap_manifest(value: Value) -> Result<ExtensionManifest> {
    if value.get("manifest").is_some() {
        serde_json::from_value::<ManifestEnvelope>(value)
            .map(|envelope| envelope.manifest)
            .map_err(|source| Error::ParseJson {
                path: "<input>".to_owned(),
                source,
            })
    } else {
        serde_json::from_value::<ExtensionManifest>(value).map_err(|source| Error::ParseJson {
            path: "<input>".to_owned(),
            source,
        })
    }
}

pub fn generate_project_files<R>(
    manifest: &ExtensionManifest,
    renderer: R,
    options: GenerateProjectFilesOptions,
) -> Result<GenerateProjectFilesResult>
where
    R: Fn(&PageMeta, &ExtensionManifest) -> Result<String>,
{
    let started = Instant::now();
    tracing::debug!(
        manifest_name = %manifest.name,
        manifest_version = %manifest.version,
        page_count = manifest.pages.len(),
        route_count = manifest.routes.len(),
        menu_count = manifest.menus.len(),
        locale_count = manifest.locales.len(),
        "generating project files"
    );
    validate_manifest(manifest)?;
    let normalized = normalize_manifest(manifest);

    let scaffold = scaffold_cache();
    let templates = &scaffold.templates;
    let merged_locales = merge_locales(scaffold.default_locales.clone(), &normalized.locales);

    let mut out = Vec::<VirtualFile>::new();
    let excluded = BTreeSet::from([
        "build.mjs.tpl",
        "package.json.tpl",
        "src/extensionConfig.ts.tpl",
        "src/routes.tsx.tpl",
        "src/routes.ts.tpl",
        "src/locales/index.ts.tpl",
        "src/pages/__PAGE__/index.tsx.tpl",
        "src/pages/__PAGE__/page.tsx.tpl",
    ]);

    for file in &scaffold.files {
        if excluded.contains(file.path.as_str()) {
            continue;
        }
        if file.path.starts_with("src/pages/__PAGE__/")
            || file.path.starts_with("src/locales/defaults/")
        {
            continue;
        }
        out.push(file.clone());
    }

    render_build_config(&normalized, templates, &mut out)?;

    out.push(VirtualFile {
        path: "src/extensionConfig.ts".to_owned(),
        content: render_template(
            required_template(templates, "src/extensionConfig.ts.tpl")?,
            &BTreeMap::from([(
                "MENUS".to_owned(),
                serde_json::to_string_pretty(&normalized.menus)
                    .map_err(|source| Error::SerializeJson { source })?,
            )]),
        ),
    });

    render_routes(&normalized, templates, &mut out)?;
    render_locales(&merged_locales, templates, &mut out)?;
    render_pages(&normalized, templates, &renderer, &mut out)?;

    let dependencies = render_dependency_entries(&out)?;
    out.push(VirtualFile {
        path: "package.json".to_owned(),
        content: render_template(
            required_template(templates, "package.json.tpl")?,
            &BTreeMap::from([
                ("NAME".to_owned(), normalized.name.clone()),
                ("VERSION".to_owned(), normalized.version.clone()),
                ("DEPENDENCIES".to_owned(), dependencies),
            ]),
        ),
    });

    let warnings = warnings_for(options);
    tracing::debug!(
        manifest_name = %normalized.name,
        file_count = out.len(),
        warning_count = warnings.len(),
        elapsed_ms = elapsed_ms(started),
        "generated project files"
    );

    Ok(GenerateProjectFilesResult {
        files: out,
        warnings,
    })
}

fn render_build_config(
    manifest: &ExtensionManifest,
    scaffold: &BTreeMap<String, String>,
    out: &mut Vec<VirtualFile>,
) -> Result<()> {
    tracing::debug!("rendering build config");
    let replacements = BTreeMap::from([
        (
            "BUILD_FORMAT".to_owned(),
            build_format(manifest).as_str().to_owned(),
        ),
        (
            "EXTERNAL_PACKAGES".to_owned(),
            serde_json::to_string_pretty(&runtime_contract::EXTERNAL_PACKAGES)
                .map_err(|source| Error::SerializeJson { source })?,
        ),
        (
            "USE_SYNC_EXTERNAL_STORE_SHIM_SOURCE".to_owned(),
            json_string_literal(USE_SYNC_EXTERNAL_STORE_SHIM_SOURCE)?,
        ),
        (
            "USE_SYNC_EXTERNAL_STORE_WITH_SELECTOR_SOURCE".to_owned(),
            json_string_literal(USE_SYNC_EXTERNAL_STORE_WITH_SELECTOR_SOURCE)?,
        ),
    ]);

    out.push(VirtualFile {
        path: "build.mjs".to_owned(),
        content: render_template(required_template(scaffold, "build.mjs.tpl")?, &replacements),
    });

    Ok(())
}

pub fn build_format(manifest: &ExtensionManifest) -> BuildFormat {
    manifest
        .build
        .as_ref()
        .and_then(|build| build.format)
        .or_else(|| {
            manifest
                .build
                .as_ref()
                .and_then(|build| build.systemjs)
                .map(|systemjs| {
                    if systemjs {
                        BuildFormat::Systemjs
                    } else {
                        BuildFormat::Esm
                    }
                })
        })
        .unwrap_or(BuildFormat::Systemjs)
}

impl BuildFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            BuildFormat::Esm => "esm",
            BuildFormat::Systemjs => "systemjs",
        }
    }

    pub fn is_systemjs(self) -> bool {
        matches!(self, BuildFormat::Systemjs)
    }
}

fn validate_manifest(manifest: &ExtensionManifest) -> Result<()> {
    if manifest.version != "1.0" {
        return Err(Error::InvalidManifestVersion);
    }
    assert_non_empty(&manifest.name, "manifest.name")?;

    for (index, route) in manifest.routes.iter().enumerate() {
        assert_non_empty(&route.path, &format!("routes[{index}].path"))?;
        assert_non_empty(&route.page_id, &format!("routes[{index}].pageId"))?;
    }

    for (index, menu) in manifest.menus.iter().enumerate() {
        assert_non_empty(&menu.parent, &format!("menus[{index}].parent"))?;
        assert_non_empty(&menu.name, &format!("menus[{index}].name"))?;
        assert_non_empty(&menu.title, &format!("menus[{index}].title"))?;
    }

    for (index, locale) in manifest.locales.iter().enumerate() {
        assert_non_empty(&locale.lang, &format!("locales[{index}].lang"))?;
        for (key, value) in &locale.messages {
            if !value.is_string() {
                return Err(Error::LocaleMessageNotString {
                    label: format!("locales[{index}].messages"),
                    key: key.clone(),
                });
            }
        }
    }

    for (index, page) in manifest.pages.iter().enumerate() {
        assert_non_empty(&page.id, &format!("pages[{index}].id"))?;
        assert_non_empty(
            &page.entry_component,
            &format!("pages[{index}].entryComponent"),
        )?;
    }

    if let Some(build) = &manifest.build
        && build.target != "kubesphere-extension"
    {
        return Err(Error::InvalidBuildTarget);
    }

    let mut page_ids = BTreeSet::new();
    for page in &manifest.pages {
        if !page_ids.insert(page.id.as_str()) {
            return Err(Error::DuplicatePageId);
        }
    }

    let mut locale_langs = BTreeSet::new();
    for locale in &manifest.locales {
        if !locale_langs.insert(locale.lang.as_str()) {
            return Err(Error::DuplicateLocaleLang);
        }
    }

    for (index, route) in manifest.routes.iter().enumerate() {
        if !page_ids.contains(route.page_id.as_str()) {
            return Err(Error::RoutePageNotFound {
                index,
                page_id: route.page_id.clone(),
            });
        }
    }

    Ok(())
}

fn normalize_manifest(manifest: &ExtensionManifest) -> ExtensionManifest {
    ExtensionManifest {
        version: "1.0".to_owned(),
        name: manifest.name.clone(),
        display_name: manifest.display_name.clone(),
        description: manifest.description.clone(),
        routes: manifest.routes.clone(),
        menus: manifest
            .menus
            .iter()
            .cloned()
            .map(|mut menu| {
                menu.skip_workspace_auth = Some(true);
                menu
            })
            .collect(),
        locales: manifest.locales.clone(),
        pages: manifest.pages.clone(),
        build: manifest.build.as_ref().map(|build| BuildMeta {
            target: "kubesphere-extension".to_owned(),
            module_name: build.module_name.clone(),
            namespace: None,
            cluster: None,
            systemjs: build.systemjs,
            format: build.format,
        }),
    }
}

fn scaffold_cache() -> &'static ScaffoldCache {
    &SCAFFOLD_CACHE
}

fn build_scaffold_cache() -> Result<ScaffoldCache> {
    let files = collect_scaffold_files()?;
    let templates = files
        .iter()
        .map(|file| (file.path.clone(), file.content.clone()))
        .collect::<BTreeMap<_, _>>();
    let default_locales = read_scaffold_default_locales(&templates)?;
    Ok(ScaffoldCache {
        files,
        templates,
        default_locales,
    })
}

fn collect_scaffold_files() -> Result<Vec<VirtualFile>> {
    let mut files = Vec::new();
    collect_dir_files(&SCAFFOLD_DIR, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn collect_dir_files(dir: &Dir<'_>, files: &mut Vec<VirtualFile>) -> Result<()> {
    for file in dir.files() {
        let rel_path = normalize_rel_path(&file.path().display().to_string())?;
        files.push(VirtualFile {
            path: rel_path,
            content: file.contents_utf8().unwrap_or_default().to_owned(),
        });
    }
    for child in dir.dirs() {
        collect_dir_files(child, files)?;
    }
    Ok(())
}

fn required_template<'a>(scaffold: &'a BTreeMap<String, String>, path: &str) -> Result<&'a str> {
    let path = normalize_rel_path(path)?;
    scaffold
        .get(&path)
        .map(String::as_str)
        .ok_or(Error::ScaffoldTemplateNotFound { path })
}

fn render_routes(
    manifest: &ExtensionManifest,
    scaffold: &BTreeMap<String, String>,
    out: &mut Vec<VirtualFile>,
) -> Result<()> {
    let template_path = if scaffold.contains_key("src/routes.tsx.tpl") {
        "src/routes.tsx.tpl"
    } else if scaffold.contains_key("src/routes.ts.tpl") {
        "src/routes.ts.tpl"
    } else {
        return Err(Error::ScaffoldRoutesTemplateNotFound);
    };
    tracing::debug!(
        manifest_name = %manifest.name,
        template_path = %template_path,
        route_count = manifest.routes.len(),
        page_count = manifest.pages.len(),
        "rendering routes"
    );
    let routes_tpl = required_template(scaffold, template_path)?;

    if template_path.ends_with(".tsx.tpl") {
        let mut used = BTreeSet::<String>::new();
        let mut name_by_page_id = BTreeMap::<String, String>::new();
        for page in &manifest.pages {
            let base = to_component_identifier(&page.id, "Page")?;
            let mut name = base.clone();
            let mut index = 2;
            while used.contains(&name) {
                name = format!("{base}_{index}");
                index += 1;
            }
            used.insert(name.clone());
            name_by_page_id.insert(page.id.clone(), name);
        }

        let mut import_page_ids = Vec::<String>::new();
        let mut seen = BTreeSet::<String>::new();
        for route in &manifest.routes {
            if seen.insert(route.page_id.clone()) {
                import_page_ids.push(route.page_id.clone());
            }
        }

        let imports = import_page_ids
            .iter()
            .map(|page_id| {
                format!(
                    "import {} from './pages/{page_id}';",
                    name_by_page_id
                        .get(page_id)
                        .map(String::as_str)
                        .unwrap_or("Page")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let routes = manifest
            .routes
            .iter()
            .map(|route| {
                let component = name_by_page_id
                    .get(&route.page_id)
                    .map(String::as_str)
                    .unwrap_or("Page");
                let parent_route_line = resolve_parent_route(&route.path)
                    .map(|parent| format!("    parentRoute: {},\n", json_string(&parent)))
                    .unwrap_or_default();
                format!(
                    "  {{\n{parent_route_line}    path: {},\n    element: <{component} />,\n  }},",
                    json_string(&route.path)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        out.push(VirtualFile {
            path: "src/routes.tsx".to_owned(),
            content: render_template(
                routes_tpl,
                &BTreeMap::from([
                    ("ROUTE_IMPORTS".to_owned(), imports),
                    ("ROUTE_ENTRIES".to_owned(), routes),
                ]),
            ),
        });
    } else {
        let routes = manifest
            .routes
            .iter()
            .map(|route| {
                let mut value = serde_json::Map::new();
                if let Some(parent) = resolve_parent_route(&route.path) {
                    value.insert("parentRoute".to_owned(), Value::String(parent));
                }
                value.insert("path".to_owned(), Value::String(route.path.clone()));
                value.insert(
                    "component".to_owned(),
                    Value::String(format!("./pages/{}", route.page_id)),
                );
                Value::Object(value)
            })
            .collect::<Vec<_>>();
        out.push(VirtualFile {
            path: "src/routes.ts".to_owned(),
            content: render_template(
                routes_tpl,
                &BTreeMap::from([(
                    "ROUTES".to_owned(),
                    serde_json::to_string_pretty(&routes)
                        .map_err(|source| Error::SerializeJson { source })?,
                )]),
            ),
        });
    }

    Ok(())
}

fn render_locales(
    locales: &[LocaleMeta],
    scaffold: &BTreeMap<String, String>,
    out: &mut Vec<VirtualFile>,
) -> Result<()> {
    tracing::debug!(locale_count = locales.len(), "rendering locales");
    let locales_index_tpl = required_template(scaffold, "src/locales/index.ts.tpl")?;
    let mut locale_infos = Vec::<(String, String, String)>::new();
    let mut variable_names = BTreeSet::<String>::new();

    for locale in locales {
        ensure_safe_file_name(&locale.lang, "locale lang")?;
        let variable_name = to_identifier(&locale.lang);
        if !variable_names.insert(variable_name.clone()) {
            return Err(Error::DuplicateLocaleIdentifier);
        }
        locale_infos.push((
            locale.lang.clone(),
            variable_name,
            format!("{}.json", locale.lang),
        ));
    }

    for locale in locales {
        out.push(VirtualFile {
            path: normalize_rel_path(&format!("src/locales/{}.json", locale.lang))?,
            content: serde_json::to_string_pretty(&locale.messages)
                .map_err(|source| Error::SerializeJson { source })?
                + "\n",
        });
    }

    let locale_imports = locale_infos
        .iter()
        .map(|(_, variable_name, file_name)| {
            format!("import {variable_name} from './{file_name}';")
        })
        .collect::<Vec<_>>()
        .join("\n");

    let locale_exports = locale_infos
        .iter()
        .map(|(lang, variable_name, _)| {
            if is_valid_identifier(lang) {
                format!("  {variable_name},")
            } else {
                format!("  '{}': {variable_name},", lang)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    out.push(VirtualFile {
        path: "src/locales/index.ts".to_owned(),
        content: render_template(
            locales_index_tpl,
            &BTreeMap::from([
                ("LOCALE_IMPORTS".to_owned(), locale_imports),
                ("LOCALE_EXPORTS".to_owned(), locale_exports),
            ]),
        ),
    });
    Ok(())
}

fn render_pages<R>(
    manifest: &ExtensionManifest,
    scaffold: &BTreeMap<String, String>,
    renderer: &R,
    out: &mut Vec<VirtualFile>,
) -> Result<()>
where
    R: Fn(&PageMeta, &ExtensionManifest) -> Result<String>,
{
    let page_index_template = required_template(scaffold, "src/pages/__PAGE__/index.tsx.tpl")?;
    let page_content_template = required_template(scaffold, "src/pages/__PAGE__/page.tsx.tpl")?;
    let page_component_file =
        if page_index_template.contains("'./page'") || page_index_template.contains("\"./page\"") {
            "page.tsx"
        } else {
            "Page.tsx"
        };

    for page in &manifest.pages {
        let started = Instant::now();
        tracing::debug!(
            manifest_name = %manifest.name,
            page_id = %page.id,
            entry_component = %page.entry_component,
            "rendering manifest page"
        );
        let page_content = renderer(page, manifest)?;
        out.push(VirtualFile {
            path: normalize_rel_path(&format!("src/pages/{}/index.tsx", page.id))?,
            content: render_template(
                page_index_template,
                &BTreeMap::from([("PAGE_ID".to_owned(), page.id.clone())]),
            ),
        });
        let content = if page_content_template.contains("__PAGE_CONTENT__") {
            render_template(
                page_content_template,
                &BTreeMap::from([("PAGE_CONTENT".to_owned(), page_content)]),
            )
        } else {
            page_content
        };
        out.push(VirtualFile {
            path: normalize_rel_path(&format!("src/pages/{}/{}", page.id, page_component_file))?,
            content,
        });
        tracing::debug!(
            manifest_name = %manifest.name,
            page_id = %page.id,
            entry_component = %page.entry_component,
            elapsed_ms = elapsed_ms(started),
            "rendered manifest page"
        );
    }
    Ok(())
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn read_scaffold_default_locales(scaffold: &BTreeMap<String, String>) -> Result<Vec<LocaleMeta>> {
    let mut locales = Vec::<LocaleMeta>::new();
    for (path, content) in scaffold {
        if !(path.starts_with("src/locales/defaults/") && path.ends_with(".json")) {
            continue;
        }
        let lang = path
            .trim_start_matches("src/locales/defaults/")
            .trim_end_matches(".json")
            .to_owned();
        ensure_safe_file_name(&lang, "default locale lang")?;
        let parsed = serde_json::from_str::<Value>(content).map_err(|source| {
            Error::InvalidScaffoldLocaleJson {
                path: path.clone(),
                message: source.to_string(),
            }
        })?;
        let messages = parsed.as_object().ok_or_else(|| Error::NonEmptyString {
            label: format!("{path} messages"),
        })?;
        let mut normalized = serde_json::Map::new();
        for (key, value) in messages {
            if !value.is_string() {
                return Err(Error::LocaleMessageNotString {
                    label: format!("{path} messages"),
                    key: key.clone(),
                });
            }
            normalized.insert(key.clone(), value.clone());
        }
        locales.push(LocaleMeta {
            lang,
            messages: normalized,
        });
    }
    Ok(locales)
}

fn merge_locales(defaults: Vec<LocaleMeta>, locales: &[LocaleMeta]) -> Vec<LocaleMeta> {
    let mut order = Vec::<String>::new();
    let mut merged = BTreeMap::<String, serde_json::Map<String, Value>>::new();

    for locale in defaults {
        if !merged.contains_key(&locale.lang) {
            order.push(locale.lang.clone());
        }
        merged.insert(locale.lang, locale.messages);
    }

    for locale in locales {
        if !merged.contains_key(&locale.lang) {
            order.push(locale.lang.clone());
        }
        let entry = merged.entry(locale.lang.clone()).or_default();
        for (key, value) in &locale.messages {
            entry.insert(key.clone(), value.clone());
        }
    }

    order
        .into_iter()
        .map(|lang| LocaleMeta {
            messages: merged.remove(&lang).unwrap_or_default(),
            lang,
        })
        .collect()
}

fn warnings_for(options: GenerateProjectFilesOptions) -> Vec<String> {
    let mut warnings = Vec::new();
    if options.build {
        warnings.push("build is not implemented".to_owned());
    }
    if options.archive {
        warnings.push("archive is not implemented".to_owned());
    }
    warnings
}

fn render_dependency_entries(files: &[VirtualFile]) -> Result<String> {
    let versions = dependency_versions();
    Ok(collect_source_dependency_packages(files)?
        .into_iter()
        .filter_map(|package| {
            versions
                .get(package.as_str())
                .map(|version| format!("    {package:?}: {version:?}"))
        })
        .collect::<Vec<_>>()
        .join(",\n"))
}

fn dependency_versions() -> BTreeMap<&'static str, &'static str> {
    runtime_contract::DEPENDENCIES
        .iter()
        .map(|dependency| (dependency.name, dependency.version))
        .collect()
}

fn collect_source_dependency_packages(files: &[VirtualFile]) -> Result<BTreeSet<String>> {
    let mut packages = BTreeSet::<String>::new();
    for file in files {
        if !is_source_dependency_file(&file.path) {
            continue;
        }
        for specifier in collect_module_specifiers(&file.path, &file.content)? {
            if let Some(package) = package_name_for_module(&specifier) {
                packages.insert(package);
            }
        }
    }
    Ok(packages)
}

fn is_source_dependency_file(path: &str) -> bool {
    if !path.starts_with("src/") {
        return false;
    }
    matches!(
        path.rsplit_once('.').map(|(_, ext)| ext),
        Some("js" | "jsx" | "mjs" | "ts" | "tsx")
    )
}

fn collect_module_specifiers(path: &str, content: &str) -> Result<BTreeSet<String>> {
    let mut specifiers = BTreeSet::<String>::new();

    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, content, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() || parsed.panicked {
        return Err(Error::ParseModuleImports {
            path: path.to_owned(),
            message: format_oxc_errors(&parsed.errors, parsed.panicked),
        });
    }

    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(declaration) => {
                specifiers.insert(declaration.source.value.as_str().to_owned());
            }
            Statement::ExportAllDeclaration(declaration) => {
                specifiers.insert(declaration.source.value.as_str().to_owned());
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if let Some(source) = &declaration.source {
                    specifiers.insert(source.value.as_str().to_owned());
                }
            }
            _ => {}
        }
    }
    Ok(specifiers)
}

fn package_name_for_module(specifier: &str) -> Option<String> {
    if specifier.starts_with('.')
        || specifier.starts_with('/')
        || specifier.starts_with("node:")
        || specifier.contains("://")
    {
        return None;
    }
    if specifier.starts_with('@') {
        let mut parts = specifier.split('/');
        let scope = parts.next()?;
        let name = parts.next()?;
        return Some(format!("{scope}/{name}"));
    }
    specifier.split('/').next().map(str::to_owned)
}

fn render_template(content: &str, vars: &BTreeMap<String, String>) -> String {
    let mut out = content.to_owned();
    for (key, value) in vars {
        out = out.replace(&format!("__{key}__"), value);
    }
    out
}

fn json_string_literal(value: &str) -> Result<String> {
    serde_json::to_string(value).map_err(|source| Error::SerializeJson { source })
}

fn assert_non_empty(value: &str, label: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::NonEmptyString {
            label: label.to_owned(),
        });
    }
    Ok(())
}

fn ensure_safe_file_name(value: &str, label: &str) -> Result<()> {
    let re = Regex::new(r"^[A-Za-z0-9._-]+$").expect("valid filename regex");
    if !re.is_match(value) {
        return Err(Error::UnsafeFileName {
            label: label.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn normalize_rel_path(path: &str) -> Result<String> {
    if path.is_empty() {
        return Err(Error::InvalidFilePath);
    }
    let path = path.replace('\\', "/");
    if path.starts_with('/') {
        return Err(Error::AbsolutePath);
    }
    let mut parts = Vec::<&str>::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                return Err(Error::PathTraversal);
            }
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        return Err(Error::InvalidFilePath);
    }
    Ok(parts.join("/"))
}

fn resolve_parent_route(route_path: &str) -> Option<String> {
    let normalized = route_path.trim_end_matches('/');
    if normalized == "/clusters/:cluster" || normalized.starts_with("/clusters/:cluster/") {
        return Some("/clusters/:cluster".to_owned());
    }
    if normalized == "/workspaces/:workspace" || normalized.starts_with("/workspaces/:workspace/") {
        return Some("/workspaces/:workspace".to_owned());
    }
    None
}

fn is_valid_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn to_identifier(value: &str) -> String {
    let mut safe = value
        .chars()
        .map(|ch| {
            if ch == '_' || ch.is_ascii_alphanumeric() {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if safe.is_empty() {
        return "lang".to_owned();
    }
    if !safe
        .chars()
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
    {
        safe.insert_str(0, "lang_");
    }
    safe
}

fn to_component_identifier(value: &str, fallback: &str) -> Result<String> {
    let re = Regex::new(r"[A-Za-z0-9]+").expect("valid component regex");
    let parts = re.find_iter(value).map(|m| m.as_str()).collect::<Vec<_>>();
    if parts.is_empty() {
        return Ok(fallback.to_owned());
    }
    let mut pascal = String::new();
    for part in parts {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            pascal.push(first.to_ascii_uppercase());
            pascal.extend(chars);
        }
    }
    if !pascal
        .chars()
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
    {
        return Ok(format!("{fallback}{pascal}"));
    }
    Ok(pascal)
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned())
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
    use super::*;

    fn sample_manifest() -> ExtensionManifest {
        serde_json::from_value(serde_json::json!({
            "version": "1.0",
            "name": "ff-test",
            "routes": [
                { "path": "/sample", "pageId": "SamplePage" },
                { "path": "/clusters/:cluster/widgets", "pageId": "SamplePage2" }
            ],
            "menus": [{
                "parent": "top",
                "name": "widgets",
                "title": "Widgets"
            }],
            "locales": [{ "lang": "en", "messages": { "HELLO": "Hello" } }],
            "pages": [
                {
                    "id": "SamplePage",
                    "entryComponent": "SamplePage",
                    "componentsTree": {}
                },
                {
                    "id": "SamplePage2",
                    "entryComponent": "SamplePage2",
                    "componentsTree": {}
                }
            ],
            "build": { "target": "kubesphere-extension", "moduleName": "ff-test", "systemjs": true }
        }))
        .unwrap()
    }

    #[test]
    fn generates_project_files_from_scaffold() {
        let manifest = sample_manifest();
        let result = generate_project_files(
            &manifest,
            |page: &PageMeta, _manifest: &ExtensionManifest| {
                Ok(format!(
                    "export default function {}() {{ return null; }}",
                    page.entry_component
                ))
            },
            GenerateProjectFilesOptions::default(),
        )
        .unwrap();

        let paths = result
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();

        assert!(paths.contains(&"package.json"));
        assert!(paths.contains(&"src/extensionConfig.ts"));
        assert!(paths.contains(&"src/routes.tsx"));
        assert!(paths.contains(&"build.mjs"));
        assert!(paths.contains(&"src/locales/en.json"));
        assert!(paths.contains(&"src/locales/zh.json"));
        assert!(paths.contains(&"src/pages/SamplePage/index.tsx"));
        assert!(paths.contains(&"src/pages/SamplePage/Page.tsx"));
        assert!(paths.contains(&"src/pages/SamplePage2/Page.tsx"));

        let routes = result
            .files
            .iter()
            .find(|file| file.path == "src/routes.tsx")
            .unwrap();
        assert!(
            routes
                .content
                .contains("import SamplePage from './pages/SamplePage';")
        );
        assert!(
            routes
                .content
                .contains("parentRoute: \"/clusters/:cluster\"")
        );

        let extension_config = result
            .files
            .iter()
            .find(|file| file.path == "src/extensionConfig.ts")
            .unwrap();
        assert!(
            extension_config
                .content
                .contains("\"skipWorkspaceAuth\": true")
        );

        let package_json = result
            .files
            .iter()
            .find(|file| file.path == "package.json")
            .unwrap();
        let package_json = serde_json::from_str::<Value>(&package_json.content).unwrap();
        let dependencies = package_json["dependencies"].as_object().unwrap();
        assert!(dependencies.contains_key("@frontend-forge/forge-components"));
        assert!(dependencies.contains_key("@ks-console/shared"));
        assert!(dependencies.contains_key("react"));
        assert!(dependencies.contains_key("react-router-dom"));
        assert!(!dependencies.contains_key("@kubed/charts"));
        assert!(!dependencies.contains_key("@kubed/hooks"));
        assert!(!dependencies.contains_key("react-dom"));
        assert!(!dependencies.contains_key("swr"));
        assert!(!dependencies.contains_key("zustand"));

        let build_config = result
            .files
            .iter()
            .find(|file| file.path == "build.mjs")
            .unwrap();
        assert!(
            build_config
                .content
                .contains("const buildFormat = 'systemjs'")
        );
        assert!(build_config.content.contains("\"@ks-console/shared\""));
        assert!(build_config.content.contains("\"react\""));
        assert!(!build_config.content.contains("'zustand'"));
        assert!(
            build_config
                .content
                .contains("resolveForgeComponentsSource()")
        );
        assert!(build_config.content.contains("resolveReactCjsShims()"));
        assert!(
            build_config
                .content
                .contains("useSyncExternalStoreWithSelector")
        );
        assert!(
            build_config
                .content
                .contains("'use-sync-external-store/index.js'")
        );
        assert!(
            !build_config
                .content
                .contains("__USE_SYNC_EXTERNAL_STORE_SHIM_SOURCE__")
        );
        assert!(
            !build_config
                .content
                .contains("__USE_SYNC_EXTERNAL_STORE_WITH_SELECTOR_SOURCE__")
        );
        assert!(
            build_config
                .content
                .contains("@frontend-forge/forge-components/src/index.ts")
        );
        assert!(build_config.content.contains("build({"));
        assert!(build_config.content.contains("absWorkingDir: rootDir"));
        assert!(build_config.content.contains("format: 'esm'"));
        assert!(build_config.content.contains("minify: true"));
        assert!(build_config.content.contains("type: 'systemjs'"));
        assert!(build_config.content.contains("esbuild_minify"));
        assert!(
            build_config
                .content
                .contains("export async function runBuild")
        );
        assert!(build_config.content.contains("charset: 'utf8'"));
        assert!(build_config.content.contains("process.env.NODE_ENV"));
        assert!(!build_config.content.contains("__MODULE_NAME_JSON__"));
    }

    #[test]
    fn package_dependencies_follow_generated_page_imports() {
        let manifest = sample_manifest();
        let result = generate_project_files(
            &manifest,
            |page: &PageMeta, _manifest: &ExtensionManifest| {
                Ok(format!(
                    r#"import useSWR from "swr";
import {{ get }} from "es-toolkit/compat";
import {{ CodeEditor }} from "@kubed/code-editor";

export default function {}() {{ return null; }}"#,
                    page.entry_component
                ))
            },
            GenerateProjectFilesOptions::default(),
        )
        .unwrap();

        let package_json = result
            .files
            .iter()
            .find(|file| file.path == "package.json")
            .unwrap();
        let package_json = serde_json::from_str::<Value>(&package_json.content).unwrap();
        let dependencies = package_json["dependencies"].as_object().unwrap();

        assert!(dependencies.contains_key("@kubed/code-editor"));
        assert!(dependencies.contains_key("es-toolkit"));
        assert!(dependencies.contains_key("swr"));
        assert!(!dependencies.contains_key("@kubed/charts"));
        assert!(!dependencies.contains_key("@kubed/hooks"));
    }

    #[test]
    fn package_dependency_scan_ignores_comments_and_strings() {
        let manifest = sample_manifest();
        let result = generate_project_files(
            &manifest,
            |page: &PageMeta, _manifest: &ExtensionManifest| {
                Ok(format!(
                    r#"// import useSWR from "swr";
const text = 'import {{ Chart }} from "@kubed/charts"';

export default function {}() {{ return null; }}"#,
                    page.entry_component
                ))
            },
            GenerateProjectFilesOptions::default(),
        )
        .unwrap();

        let package_json = result
            .files
            .iter()
            .find(|file| file.path == "package.json")
            .unwrap();
        let package_json = serde_json::from_str::<Value>(&package_json.content).unwrap();
        let dependencies = package_json["dependencies"].as_object().unwrap();

        assert!(!dependencies.contains_key("@kubed/charts"));
        assert!(!dependencies.contains_key("swr"));
    }

    #[test]
    fn returns_build_and_archive_warnings() {
        let manifest = sample_manifest();
        let result = generate_project_files(
            &manifest,
            |_: &PageMeta, _: &ExtensionManifest| Ok(String::new()),
            GenerateProjectFilesOptions {
                build: true,
                archive: true,
            },
        )
        .unwrap();

        assert_eq!(
            result.warnings,
            vec![
                "build is not implemented".to_owned(),
                "archive is not implemented".to_owned()
            ]
        );
    }

    #[test]
    fn rejects_route_for_missing_page() {
        let mut manifest = sample_manifest();
        manifest.routes[0].page_id = "Missing".to_owned();

        let err = generate_project_files(
            &manifest,
            |_: &PageMeta, _: &ExtensionManifest| Ok(String::new()),
            GenerateProjectFilesOptions::default(),
        )
        .unwrap_err()
        .to_string();

        assert_eq!(err, "routes[0].pageId not found in pages: Missing");
    }
}
