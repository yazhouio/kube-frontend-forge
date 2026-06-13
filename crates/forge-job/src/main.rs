use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::LazyLock,
    time::{Duration, Instant},
};

use flate2::{Compression, write::GzEncoder};
use forge_core::{BuildPlan, ForgeCore};
use forge_project_generator::{
    ExtensionManifest, TreeShakingSummary, VirtualFile, build_config_summary, unwrap_manifest,
};
use serde::Serialize;
use snafu::Snafu;
use tempfile::TempDir;

mod systemjs_validator;

type Result<T, E = Error> = std::result::Result<T, E>;

static MANIFEST_VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    let schema = ForgeCore::shared().manifest_schema();
    jsonschema::validator_for(&schema).expect("manifest schema must compile")
});

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = parse_args(env::args().skip(1))?;
    match args.command {
        CommandArgs::Build(args) => run_build(args),
        CommandArgs::PageCode(args) => run_page_code(args),
        CommandArgs::ProjectFiles(args) => run_project_files(args),
        CommandArgs::Schema(args) => run_schema(args),
    }
}

fn run_page_code(args: InputOutputArgs) -> Result<()> {
    let value = read_json(&args.input)?;
    let code = ForgeCore::shared().generate_page_code(value)?;
    write_text_or_stdout(args.out, &code)
}

fn run_project_files(args: InputOutputArgs) -> Result<()> {
    let manifest = read_manifest(&args.input)?;
    let result = ForgeCore::shared().generate_project_files(&manifest)?;
    let output = serde_json::to_string_pretty(&result).context(SerializeJsonSnafu)?;
    write_text_or_stdout(args.out, &(output + "\n"))
}

fn run_schema(args: SchemaArgs) -> Result<()> {
    fs::create_dir_all(&args.out_dir).context(CreateDirSnafu {
        path: args.out_dir.clone(),
    })?;
    let core = ForgeCore::shared();
    write_json_file(
        &args.out_dir.join("component-tree.schema.json"),
        &core.component_tree_schema(),
    )?;
    write_json_file(
        &args.out_dir.join("node-source.schema.json"),
        &core.node_source_schema(),
    )?;
    write_json_file(
        &args.out_dir.join("data-source-source.schema.json"),
        &core.data_source_source_schema(),
    )?;
    write_json_file(
        &args.out_dir.join("manifest.schema.json"),
        &core.manifest_schema(),
    )
}

fn run_build(args: BuildArgs) -> Result<()> {
    fs::create_dir_all(&args.out_dir).context(CreateDirSnafu {
        path: args.out_dir.clone(),
    })?;

    let started = Instant::now();
    let mut timings = Timings::default();
    let mut warnings = Vec::<String>::new();
    let mut result = JobResult::running();
    result.project_archive.enabled = args.emit_project_archive;

    let build_result = (|| -> Result<BuildArtifacts> {
        let manifest = read_manifest(&args.input)?;
        result.input = Some(InputSummary {
            manifest_name: manifest.name.clone(),
            module_name: manifest
                .build
                .as_ref()
                .and_then(|build| build.module_name.clone()),
            pages: manifest.pages.len(),
            routes: manifest.routes.len(),
        });

        let plan_started = Instant::now();
        let plan = ForgeCore::shared().create_build_plan(&manifest)?;
        timings.plan_ms = elapsed_ms(plan_started);
        timings.project_gen_ms = plan.timings.project_gen_ms;
        timings.component_gen_ms = plan.timings.component_gen_ms;
        warnings.extend(plan.warnings.clone());

        let work = TempDir::new().context(CreateTempDirSnafu)?;
        let project_dir = work.path().join("project");
        let write_started = Instant::now();
        write_virtual_files(&project_dir, &plan.files)?;
        link_node_modules(&project_dir)?;
        timings.write_project_ms = elapsed_ms(write_started);

        let project_archive = if args.emit_project_archive {
            let path = args.out_dir.join("project.tar.gz");
            let archive_started = Instant::now();
            archive_virtual_files(&plan.files, &path)?;
            timings.archive_ms += elapsed_ms(archive_started);
            Some(path)
        } else {
            None
        };

        let build_started = Instant::now();
        run_build_script(&project_dir)?;
        timings.build_ms = elapsed_ms(build_started);

        let dist_dir = project_dir.join(&plan.expectations.dist_dir);
        validate_dist(&dist_dir, plan.expectations.systemjs)?;
        let mut bundle_summary = summarize_dist(&dist_dir)?;

        let archive_started = Instant::now();
        let build_archive = args.out_dir.join("build.tar.gz");
        archive_directory(&dist_dir, &build_archive)?;
        bundle_summary.archive_bytes = Some(file_len(&build_archive)?);
        timings.archive_ms += elapsed_ms(archive_started);

        let dependencies = generated_dependencies(&plan.files)?;

        Ok(BuildArtifacts {
            plan,
            build_archive,
            project_archive,
            bundle_summary,
            dependencies,
        })
    })();

    timings.total_ms = elapsed_ms(started);
    result.timings = timings;
    result.warnings = warnings;
    result.versions = Versions::detect();

    match build_result {
        Ok(artifacts) => {
            result.status = JobStatus::Succeeded;
            result.artifacts.build_archive = Some(path_string(&artifacts.build_archive));
            result.artifacts.entry = Some(artifacts.plan.entry);
            result.artifacts.dist_dir = Some(artifacts.plan.expectations.dist_dir.clone());
            result.project_archive.path = artifacts
                .project_archive
                .as_ref()
                .map(|path| path_string(path));
            result.expectations = Some(artifacts.plan.expectations);
            result.build.dependencies = artifacts.dependencies;
            result.build.bundle = Some(artifacts.bundle_summary);
            write_result_json(&args.out_dir, &result)?;
            Ok(())
        }
        Err(error) => {
            result.status = JobStatus::Failed;
            result.errors.push(error.to_string());
            write_result_json(&args.out_dir, &result)?;
            Err(error)
        }
    }
}

fn run_build_script(project_dir: &Path) -> Result<()> {
    let timeout = env::var("FORGE_BUILD_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(30_000));

    let mut command = build_script_command();
    let command_label = "node build.mjs".to_owned();

    let mut child = command
        .current_dir(project_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context(SpawnCommandSnafu {
            command: command_label.clone(),
        })?;

    let started = Instant::now();
    loop {
        if started.elapsed() > timeout {
            let _ = child.kill();
            let output = child.wait_with_output().context(WaitCommandSnafu {
                command: command_label.clone(),
            })?;
            return Err(Error::CommandTimeout {
                command: command_label,
                timeout_ms: timeout.as_millis() as u64,
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        if child
            .try_wait()
            .context(WaitCommandSnafu {
                command: command_label.clone(),
            })?
            .is_some()
        {
            let output = child.wait_with_output().context(WaitCommandSnafu {
                command: command_label.clone(),
            })?;
            if output.status.success() {
                return Ok(());
            }
            return Err(Error::CommandFailed {
                command: command_label,
                status: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn validate_dist(dist_dir: &Path, expect_systemjs: bool) -> Result<()> {
    if !dist_dir.is_dir() {
        return Err(Error::MissingDistDir {
            path: dist_dir.to_path_buf(),
        });
    }

    let js_files = collect_files(dist_dir)?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "js"))
        .collect::<Vec<_>>();
    if js_files.is_empty() {
        return Err(Error::NoJsOutput {
            path: dist_dir.to_path_buf(),
        });
    }

    let mut has_system_register = false;
    for path in js_files {
        let content = fs::read_to_string(&path).context(ReadFileSnafu { path: path.clone() })?;
        if expect_systemjs {
            match systemjs_validator::validate_systemjs_code(&content) {
                Ok(validation) => {
                    has_system_register = has_system_register || validation.has_system_register;
                }
                Err(systemjs_validator::SystemJsValidationError::Parse { message }) => {
                    return Err(Error::ParseJsOutput { path, message });
                }
                Err(systemjs_validator::SystemJsValidationError::ForbiddenToken { token }) => {
                    return Err(Error::ForbiddenJsToken { path, token });
                }
            }
        } else {
            match systemjs_validator::validate_systemjs_code(&content) {
                Ok(_) => {}
                Err(systemjs_validator::SystemJsValidationError::Parse { message }) => {
                    return Err(Error::ParseJsOutput { path, message });
                }
                Err(systemjs_validator::SystemJsValidationError::ForbiddenToken { token }) => {
                    return Err(Error::ForbiddenJsToken { path, token });
                }
            }
        }
    }

    if expect_systemjs && !has_system_register {
        return Err(Error::MissingSystemRegister);
    }

    Ok(())
}

fn read_manifest(path: &Path) -> Result<ExtensionManifest> {
    let value = read_json(path)?;
    validate_manifest_json(path, &value)?;
    unwrap_manifest(value).map_err(Error::from)
}

fn read_json(path: &Path) -> Result<serde_json::Value> {
    let raw = fs::read_to_string(path).context(ReadFileSnafu {
        path: path.to_path_buf(),
    })?;
    serde_json::from_str(&raw).context(ParseJsonSnafu {
        path: path.to_path_buf(),
    })
}

fn validate_manifest_json(path: &Path, value: &serde_json::Value) -> Result<()> {
    MANIFEST_VALIDATOR
        .validate(value)
        .map_err(|err| Error::SchemaValidation {
            path: path.to_path_buf(),
            message: err.to_string(),
        })
}

fn write_json_file(path: &Path, value: &serde_json::Value) -> Result<()> {
    let output = serde_json::to_string_pretty(value).context(SerializeJsonSnafu)? + "\n";
    fs::write(path, output).context(WriteFileSnafu {
        path: path.to_path_buf(),
    })
}

fn write_virtual_files(root: &Path, files: &[VirtualFile]) -> Result<()> {
    fs::create_dir_all(root).context(CreateDirSnafu {
        path: root.to_path_buf(),
    })?;
    let mut stable = files.to_vec();
    stable.sort_by(|left, right| left.path.cmp(&right.path));
    for file in stable {
        let full = safe_join(root, &file.path)?;
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).context(CreateDirSnafu {
                path: parent.to_path_buf(),
            })?;
        }
        fs::write(&full, file.content).context(WriteFileSnafu { path: full })?;
    }
    Ok(())
}

fn link_node_modules(project_dir: &Path) -> Result<()> {
    let Some(node_modules_dir) = env::var("FORGE_NODE_MODULES_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
    else {
        return Ok(());
    };
    if !node_modules_dir.is_dir() {
        return Err(Error::MissingNodeModulesDir {
            path: node_modules_dir,
        });
    }
    let link_path = project_dir.join("node_modules");
    if link_path.exists() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&node_modules_dir, &link_path).context(SymlinkSnafu {
            source_path: node_modules_dir,
            link_path,
        })?;
    }
    #[cfg(not(unix))]
    {
        return Err(Error::UnsupportedNodeModulesLink {
            path: node_modules_dir,
        });
    }
    Ok(())
}

fn archive_virtual_files(files: &[VirtualFile], out_path: &Path) -> Result<()> {
    let file = fs::File::create(out_path).context(WriteFileSnafu {
        path: out_path.to_path_buf(),
    })?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = tar::Builder::new(encoder);

    let mut stable = files.to_vec();
    stable.sort_by(|left, right| left.path.cmp(&right.path));
    for file in stable {
        let path = normalize_rel_path(&file.path)?;
        let bytes = file.content.into_bytes();
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, path, bytes.as_slice())
            .context(AppendTarSnafu {
                path: out_path.to_path_buf(),
            })?;
    }
    builder.finish().context(FinishTarSnafu {
        path: out_path.to_path_buf(),
    })?;
    Ok(())
}

fn archive_directory(root: &Path, out_path: &Path) -> Result<()> {
    let file = fs::File::create(out_path).context(WriteFileSnafu {
        path: out_path.to_path_buf(),
    })?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for path in collect_files(root)? {
        let rel = path
            .strip_prefix(root)
            .map_err(|_| Error::InvalidFilePath {
                path: path_string(&path),
            })?;
        builder
            .append_path_with_name(&path, rel)
            .context(AppendTarSnafu {
                path: out_path.to_path_buf(),
            })?;
    }
    builder.finish().context(FinishTarSnafu {
        path: out_path.to_path_buf(),
    })?;
    Ok(())
}

fn summarize_dist(root: &Path) -> Result<BundleSummary> {
    let mut files = Vec::new();
    let mut total_bytes = 0;
    let mut js_bytes = 0;
    let mut css_bytes = 0;

    for path in collect_files(root)? {
        let rel = path
            .strip_prefix(root)
            .map_err(|_| Error::InvalidFilePath {
                path: path_string(&path),
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let size_bytes = file_len(&path)?;
        let kind = bundle_file_kind(&path);

        total_bytes += size_bytes;
        match kind {
            "js" => js_bytes += size_bytes,
            "css" => css_bytes += size_bytes,
            _ => {}
        }

        files.push(BundleFileSummary {
            path: rel,
            kind,
            size_bytes,
        });
    }

    Ok(BundleSummary {
        files,
        total_bytes,
        js_bytes,
        css_bytes,
        archive_bytes: None,
    })
}

fn bundle_file_kind(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("js") => "js",
        Some("css") => "css",
        Some("map") => "map",
        Some("json") => "json",
        Some("html") => "html",
        _ => "asset",
    }
}

fn file_len(path: &Path) -> Result<u64> {
    Ok(fs::metadata(path)
        .context(ReadMetadataSnafu {
            path: path.to_path_buf(),
        })?
        .len())
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_inner(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files_inner(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(root).context(ReadDirSnafu {
        path: root.to_path_buf(),
    })? {
        let entry = entry.context(ReadDirEntrySnafu {
            path: root.to_path_buf(),
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_inner(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn write_result_json(out_dir: &Path, result: &JobResult) -> Result<()> {
    let output = serde_json::to_string_pretty(result).context(SerializeJsonSnafu)? + "\n";
    fs::write(out_dir.join("result.json"), output).context(WriteFileSnafu {
        path: out_dir.join("result.json"),
    })
}

fn generated_dependencies(files: &[VirtualFile]) -> Result<BTreeMap<String, String>> {
    let Some(package_file) = files.iter().find(|file| file.path == "package.json") else {
        return Ok(BTreeMap::new());
    };
    let package_json = serde_json::from_str::<serde_json::Value>(&package_file.content)
        .context(ParseGeneratedPackageJsonSnafu)?;
    let dependencies = package_json
        .get("dependencies")
        .and_then(|value| value.as_object())
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(|(name, version)| {
            version
                .as_str()
                .map(|version| (name.clone(), version.to_owned()))
        })
        .collect();
    Ok(dependencies)
}

fn write_text_or_stdout(out: Option<PathBuf>, content: &str) -> Result<()> {
    if let Some(path) = out {
        fs::write(&path, content).context(WriteFileSnafu { path })
    } else {
        print!("{content}");
        io::stdout().flush().context(StdoutSnafu)
    }
}

fn safe_join(root: &Path, rel_path: &str) -> Result<PathBuf> {
    let normalized = normalize_rel_path(rel_path)?;
    Ok(root.join(normalized))
}

fn normalize_rel_path(path: &str) -> Result<PathBuf> {
    if path.is_empty() {
        return Err(Error::InvalidFilePath {
            path: path.to_owned(),
        });
    }
    let path = path.replace('\\', "/");
    if path.starts_with('/') {
        return Err(Error::AbsolutePath { path });
    }
    let mut out = PathBuf::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                return Err(Error::PathTraversal { path });
            }
            _ => out.push(part),
        }
    }
    if out.as_os_str().is_empty() {
        return Err(Error::InvalidFilePath {
            path: path.to_owned(),
        });
    }
    Ok(out)
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<ParsedArgs> {
    let mut args = args.into_iter();
    let command = args.next().ok_or(Error::MissingCommand)?;
    let command = match command.as_str() {
        "build" => CommandArgs::Build(parse_build_args(args)?),
        "page-code" => CommandArgs::PageCode(parse_input_output_args(args)?),
        "project-files" => CommandArgs::ProjectFiles(parse_input_output_args(args)?),
        "schema" => CommandArgs::Schema(parse_schema_args(args)?),
        _ => return Err(Error::UnknownCommand { command }),
    };
    Ok(ParsedArgs { command })
}

fn parse_build_args(args: impl IntoIterator<Item = String>) -> Result<BuildArgs> {
    let mut input = None;
    let mut out_dir = None;
    let mut emit_project_archive = env_bool("FORGE_EMIT_PROJECT_ARCHIVE").unwrap_or(false);
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => {
                input = Some(PathBuf::from(
                    args.next().ok_or(Error::MissingArgValue { arg })?,
                ));
            }
            "--out-dir" => {
                out_dir = Some(PathBuf::from(
                    args.next().ok_or(Error::MissingArgValue { arg })?,
                ));
            }
            "--emit-project-archive" => {
                let value = args.next().ok_or(Error::MissingArgValue { arg })?;
                emit_project_archive = parse_bool(&value).ok_or(Error::InvalidBool {
                    arg: "--emit-project-archive".to_owned(),
                    value,
                })?;
            }
            _ => return Err(Error::UnknownArgument { arg }),
        }
    }

    Ok(BuildArgs {
        input: input.ok_or(Error::MissingRequiredArg { arg: "--input" })?,
        out_dir: out_dir.ok_or(Error::MissingRequiredArg { arg: "--out-dir" })?,
        emit_project_archive,
    })
}

fn parse_input_output_args(args: impl IntoIterator<Item = String>) -> Result<InputOutputArgs> {
    let mut input = None;
    let mut out = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => {
                input = Some(PathBuf::from(
                    args.next().ok_or(Error::MissingArgValue { arg })?,
                ));
            }
            "--out" => {
                out = Some(PathBuf::from(
                    args.next().ok_or(Error::MissingArgValue { arg })?,
                ));
            }
            _ => return Err(Error::UnknownArgument { arg }),
        }
    }

    Ok(InputOutputArgs {
        input: input.ok_or(Error::MissingRequiredArg { arg: "--input" })?,
        out,
    })
}

fn parse_schema_args(args: impl IntoIterator<Item = String>) -> Result<SchemaArgs> {
    let mut out_dir = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out-dir" => {
                out_dir = Some(PathBuf::from(
                    args.next().ok_or(Error::MissingArgValue { arg })?,
                ));
            }
            _ => return Err(Error::UnknownArgument { arg }),
        }
    }

    Ok(SchemaArgs {
        out_dir: out_dir.ok_or(Error::MissingRequiredArg { arg: "--out-dir" })?,
    })
}

fn env_bool(name: &str) -> Option<bool> {
    env::var(name).ok().and_then(|value| parse_bool(&value))
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[derive(Debug)]
struct ParsedArgs {
    command: CommandArgs,
}

#[derive(Debug)]
enum CommandArgs {
    Build(BuildArgs),
    PageCode(InputOutputArgs),
    ProjectFiles(InputOutputArgs),
    Schema(SchemaArgs),
}

#[derive(Debug)]
struct BuildArgs {
    input: PathBuf,
    out_dir: PathBuf,
    emit_project_archive: bool,
}

#[derive(Debug)]
struct InputOutputArgs {
    input: PathBuf,
    out: Option<PathBuf>,
}

#[derive(Debug)]
struct SchemaArgs {
    out_dir: PathBuf,
}

struct BuildArtifacts {
    plan: BuildPlan,
    build_archive: PathBuf,
    project_archive: Option<PathBuf>,
    bundle_summary: BundleSummary,
    dependencies: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JobResult {
    status: JobStatus,
    input: Option<InputSummary>,
    artifacts: Artifacts,
    project_archive: ProjectArchive,
    expectations: Option<forge_core::BuildExpectations>,
    build: BuildSummary,
    timings: Timings,
    warnings: Vec<String>,
    errors: Vec<String>,
    versions: Versions,
}

impl JobResult {
    fn running() -> Self {
        Self {
            status: JobStatus::Running,
            input: None,
            artifacts: Artifacts::default(),
            project_archive: ProjectArchive::default(),
            expectations: None,
            build: BuildSummary::default(),
            timings: Timings::default(),
            warnings: Vec::new(),
            errors: Vec::new(),
            versions: Versions::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum JobStatus {
    Running,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct InputSummary {
    manifest_name: String,
    module_name: Option<String>,
    pages: usize,
    routes: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Artifacts {
    build_archive: Option<String>,
    entry: Option<String>,
    dist_dir: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectArchive {
    enabled: bool,
    path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildSummary {
    dependencies: BTreeMap<String, String>,
    external_packages: Vec<String>,
    minify: bool,
    tree_shaking: TreeShakingSummary,
    bundle: Option<BundleSummary>,
}

impl Default for BuildSummary {
    fn default() -> Self {
        let config = build_config_summary();
        Self {
            dependencies: BTreeMap::new(),
            external_packages: config
                .external_packages
                .into_iter()
                .map(str::to_owned)
                .collect(),
            minify: config.minify,
            tree_shaking: config.tree_shaking,
            bundle: None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct BundleSummary {
    files: Vec<BundleFileSummary>,
    total_bytes: u64,
    js_bytes: u64,
    css_bytes: u64,
    archive_bytes: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BundleFileSummary {
    path: String,
    kind: &'static str,
    size_bytes: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Timings {
    total_ms: u64,
    plan_ms: u64,
    project_gen_ms: u64,
    component_gen_ms: u64,
    write_project_ms: u64,
    build_ms: u64,
    archive_ms: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Versions {
    forge_job: String,
    forge_components: String,
    esbuild: String,
    swc: String,
    node: String,
    pnpm: String,
}

impl Versions {
    fn detect() -> Self {
        Self {
            forge_job: env!("CARGO_PKG_VERSION").to_owned(),
            forge_components: detect_forge_components_version().unwrap_or_else(|| "unknown".into()),
            esbuild: detect_node_package_version("esbuild").unwrap_or_else(|| "unknown".into()),
            swc: detect_node_package_version("@swc/core").unwrap_or_else(|| "unknown".into()),
            node: detect_node_version().unwrap_or_else(|| "unknown".into()),
            pnpm: detect_pnpm_version().unwrap_or_else(|| "unknown".into()),
        }
    }
}

fn detect_node_version() -> Option<String> {
    if let Ok(value) = env::var("FORGE_NODE_VERSION") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.trim_start_matches('v').to_owned());
        }
    }
    let command = env::var("FORGE_NODE_BIN").unwrap_or_else(|_| "node".into());
    command_version(&command, &["--version"]).map(|value| value.trim_start_matches('v').to_owned())
}

fn detect_pnpm_version() -> Option<String> {
    if let Ok(value) = env::var("FORGE_PNPM_VERSION") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }
    command_version("pnpm", &["--version"])
}

fn detect_forge_components_version() -> Option<String> {
    if let Ok(value) = env::var("FORGE_COMPONENTS_VERSION")
        && !value.trim().is_empty()
    {
        return Some(value);
    }
    let explicit = env::var("FORGE_COMPONENTS_PACKAGE_JSON")
        .ok()
        .map(PathBuf::from);
    let candidates = explicit
        .into_iter()
        .chain(find_upwards("packages/forge-components/package.json"));
    for path in candidates {
        if let Ok(raw) = fs::read_to_string(path) {
            let value = serde_json::from_str::<serde_json::Value>(&raw).ok()?;
            if let Some(version) = value.get("version").and_then(|value| value.as_str()) {
                return Some(version.to_owned());
            }
        }
    }
    None
}

fn find_upwards(rel_path: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(mut current) = env::current_dir() else {
        return out;
    };
    loop {
        out.push(current.join(rel_path));
        if !current.pop() {
            break;
        }
    }
    out
}

fn build_script_command() -> Command {
    let mut command = Command::new(env::var("FORGE_NODE_BIN").unwrap_or_else(|_| "node".into()));
    command.arg("build.mjs");
    command
}

fn detect_node_package_version(package_name: &str) -> Option<String> {
    if let Ok(node_modules_dir) = env::var("FORGE_NODE_MODULES_DIR") {
        let package_json = PathBuf::from(node_modules_dir)
            .join(package_name)
            .join("package.json");
        if let Some(version) = read_package_version(&package_json) {
            return Some(version);
        }
    }
    for package_json in find_upwards(&format!("node_modules/{package_name}/package.json")) {
        if let Some(version) = read_package_version(&package_json) {
            return Some(version);
        }
    }
    None
}

fn read_package_version(package_json: &Path) -> Option<String> {
    let raw = fs::read_to_string(package_json).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&raw).ok()?;
    value
        .get("version")
        .and_then(|value| value.as_str())
        .map(str::to_owned)
}

fn command_version(command: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(command).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if text.is_empty() { None } else { Some(text) }
}

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("usage: frontend-forge-job <build|page-code|project-files|schema> ..."))]
    MissingCommand,

    #[snafu(display("unknown command `{command}`"))]
    UnknownCommand { command: String },

    #[snafu(display("unknown argument `{arg}`"))]
    UnknownArgument { arg: String },

    #[snafu(display("missing value for {arg}"))]
    MissingArgValue { arg: String },

    #[snafu(display("missing required argument {arg}"))]
    MissingRequiredArg { arg: &'static str },

    #[snafu(display("{arg} must be true or false, got `{value}`"))]
    InvalidBool { arg: String, value: String },

    #[snafu(display("failed to read {}", path.display()))]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("failed to write {}", path.display()))]
    WriteFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("failed to read directory {}", path.display()))]
    ReadDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("failed to read directory entry in {}", path.display()))]
    ReadDirEntry {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("failed to read metadata for {}", path.display()))]
    ReadMetadata {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("failed to create directory {}", path.display()))]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("failed to create temp dir"))]
    CreateTempDir { source: std::io::Error },

    #[snafu(display("failed to parse json {}", path.display()))]
    ParseJson {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[snafu(display("failed to parse generated package.json: {source}"))]
    ParseGeneratedPackageJson { source: serde_json::Error },

    #[snafu(display("failed to serialize json: {source}"))]
    SerializeJson { source: serde_json::Error },

    #[snafu(display("manifest schema validation failed for {}: {message}", path.display()))]
    SchemaValidation { path: PathBuf, message: String },

    #[snafu(display("{source}"))]
    Core { source: forge_core::Error },

    #[snafu(display("{source}"))]
    ProjectGenerator {
        source: forge_project_generator::Error,
    },

    #[snafu(display("failed to spawn {command}: {source}"))]
    SpawnCommand {
        command: String,
        source: std::io::Error,
    },

    #[snafu(display("failed to wait for {command}: {source}"))]
    WaitCommand {
        command: String,
        source: std::io::Error,
    },

    #[snafu(display(
        "{command} failed with status {status:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    ))]
    CommandFailed {
        command: String,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },

    #[snafu(display(
        "{command} timed out after {timeout_ms}ms\nstdout:\n{stdout}\nstderr:\n{stderr}"
    ))]
    CommandTimeout {
        command: String,
        timeout_ms: u64,
        stdout: String,
        stderr: String,
    },

    #[snafu(display("missing build dist directory {}", path.display()))]
    MissingDistDir { path: PathBuf },

    #[snafu(display("FORGE_NODE_MODULES_DIR does not exist: {}", path.display()))]
    MissingNodeModulesDir { path: PathBuf },

    #[snafu(display(
        "failed to symlink node_modules from {} to {}: {source}",
        source_path.display(),
        link_path.display()
    ))]
    Symlink {
        source_path: PathBuf,
        link_path: PathBuf,
        source: std::io::Error,
    },

    #[cfg(not(unix))]
    #[snafu(display("FORGE_NODE_MODULES_DIR linking is not supported on this platform: {}", path.display()))]
    UnsupportedNodeModulesLink { path: PathBuf },

    #[snafu(display("build dist directory has no JavaScript output: {}", path.display()))]
    NoJsOutput { path: PathBuf },

    #[snafu(display("illegal output: missing System.register"))]
    MissingSystemRegister,

    #[snafu(display("illegal output {} failed JavaScript parse: {message}", path.display()))]
    ParseJsOutput { path: PathBuf, message: String },

    #[snafu(display("illegal output {} contains forbidden token `{token}`", path.display()))]
    ForbiddenJsToken { path: PathBuf, token: &'static str },

    #[snafu(display("invalid file path `{path}`"))]
    InvalidFilePath { path: String },

    #[snafu(display("absolute path is not allowed: {path}"))]
    AbsolutePath { path: String },

    #[snafu(display("path traversal is not allowed: {path}"))]
    PathTraversal { path: String },

    #[snafu(display("failed to append tar entry for {}", path.display()))]
    AppendTar {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("failed to finish tar archive {}", path.display()))]
    FinishTar {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("failed to write stdout: {source}"))]
    Stdout { source: std::io::Error },
}

impl From<forge_core::Error> for Error {
    fn from(source: forge_core::Error) -> Self {
        Self::Core { source }
    }
}

impl From<forge_project_generator::Error> for Error {
    fn from(source: forge_project_generator::Error) -> Self {
        Self::ProjectGenerator { source }
    }
}

trait Context<T, C> {
    fn context(self, context: C) -> Result<T>;
}

impl<T, E, C> Context<T, C> for std::result::Result<T, E>
where
    C: snafu::IntoError<Error, Source = E>,
{
    fn context(self, context: C) -> Result<T> {
        self.map_err(|source| context.into_error(source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_default_project_archive_to_false() {
        let parsed = parse_args([
            "build".to_owned(),
            "--input".to_owned(),
            "/input/manifest.json".to_owned(),
            "--out-dir".to_owned(),
            "/output".to_owned(),
        ])
        .unwrap();

        match parsed.command {
            CommandArgs::Build(args) => assert!(!args.emit_project_archive),
            _ => panic!("expected build args"),
        }
    }

    #[test]
    fn build_args_accept_project_archive_flag() {
        let parsed = parse_args([
            "build".to_owned(),
            "--input".to_owned(),
            "/input/manifest.json".to_owned(),
            "--out-dir".to_owned(),
            "/output".to_owned(),
            "--emit-project-archive".to_owned(),
            "true".to_owned(),
        ])
        .unwrap();

        match parsed.command {
            CommandArgs::Build(args) => assert!(args.emit_project_archive),
            _ => panic!("expected build args"),
        }
    }

    #[test]
    fn schema_args_accept_out_dir() {
        let parsed = parse_args([
            "schema".to_owned(),
            "--out-dir".to_owned(),
            "/output/schema".to_owned(),
        ])
        .unwrap();

        match parsed.command {
            CommandArgs::Schema(args) => assert_eq!(args.out_dir, PathBuf::from("/output/schema")),
            _ => panic!("expected schema args"),
        }
    }

    #[test]
    fn schema_command_writes_manifest_and_component_tree_schemas() {
        let dir = TempDir::new().unwrap();
        run_schema(SchemaArgs {
            out_dir: dir.path().to_path_buf(),
        })
        .unwrap();

        let manifest_schema: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(dir.path().join("manifest.schema.json")).unwrap(),
        )
        .unwrap();
        let component_tree_schema: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(dir.path().join("component-tree.schema.json")).unwrap(),
        )
        .unwrap();
        let node_source_schema: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(dir.path().join("node-source.schema.json")).unwrap(),
        )
        .unwrap();
        let data_source_source_schema: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(dir.path().join("data-source-source.schema.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(
            manifest_schema.get("$id").and_then(|value| value.as_str()),
            Some("https://frontend-forge.dev/schemas/manifest.schema.json")
        );
        assert!(manifest_schema.pointer("/$defs/componentTree").is_some());
        assert_eq!(
            component_tree_schema
                .get("$id")
                .and_then(|value| value.as_str()),
            Some("https://frontend-forge.dev/schemas/component-tree.schema.json")
        );
        assert!(
            component_tree_schema
                .pointer("/$defs/componentNode")
                .is_some()
        );
        assert_eq!(
            node_source_schema
                .get("$id")
                .and_then(|value| value.as_str()),
            Some("https://frontend-forge.dev/schemas/node-source.schema.json")
        );
        assert!(node_source_schema.pointer("/$defs/nodeSource").is_some());
        assert_eq!(
            data_source_source_schema
                .get("$id")
                .and_then(|value| value.as_str()),
            Some("https://frontend-forge.dev/schemas/data-source-source.schema.json")
        );
        assert!(
            data_source_source_schema
                .pointer("/$defs/dataSourceSource")
                .is_some()
        );
    }

    #[test]
    fn manifest_schema_accepts_full_example() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../examples/full.json")).unwrap();
        validate_manifest_json(Path::new("examples/full.json"), &manifest).unwrap();
    }

    #[test]
    fn rejects_path_traversal() {
        let error = normalize_rel_path("../index.js").unwrap_err().to_string();
        assert_eq!(error, "path traversal is not allowed: ../index.js");
    }
}
