use std::{
    env, fs, io,
    net::SocketAddr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    body::Bytes,
    extract::Path as AxumPath,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use flate2::{Compression, write::GzEncoder};
use forge_core::ForgeCore;
use forge_project_generator::{ExtensionManifest, VirtualFile, unwrap_manifest};
use serde::Deserialize;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower_http::services::ServeDir;

type Result<T, E = ServerError> = std::result::Result<T, E>;

const SCHEMA_FILES: &[(&str, &str)] = &[
    (
        "manifest.schema.json",
        include_str!("../../../schemas/manifest.schema.json"),
    ),
    (
        "component-tree.schema.json",
        include_str!("../../../schemas/component-tree.schema.json"),
    ),
    (
        "node-source.schema.json",
        include_str!("../../../schemas/node-source.schema.json"),
    ),
    (
        "data-source-source.schema.json",
        include_str!("../../../schemas/data-source-source.schema.json"),
    ),
];

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let options = parse_server_options()?;
    let config = load_server_config(options.config_path.as_deref())?;
    let app = build_router(config)?;
    let listener = tokio::net::TcpListener::bind(options.addr).await?;
    println!("frontend-forge-server listening on http://{}", options.addr);
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_router(config: ServerConfig) -> Result<Router> {
    let mut app = Router::new()
        .route("/schemas/{name}", get(schema_file))
        .route("/api/project/files", post(project_files))
        .route("/api/project/files.tar.gz", post(project_files_tar))
        .route("/api/project/build", post(project_build))
        .route("/api/project/build.tar.gz", post(project_build_tar));

    for route in config.static_routes {
        let prefix = normalize_static_prefix(&route.prefix)?;
        app = app.nest_service(&prefix, ServeDir::new(route.root));
    }

    Ok(app)
}

fn parse_server_options() -> Result<ServerOptions> {
    parse_server_options_from(env::args().skip(1))
}

fn parse_server_options_from<I, S>(args: I) -> Result<ServerOptions>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut addr =
        env::var("FRONTEND_FORGE_SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());
    let mut config_path = env::var("FRONTEND_FORGE_SERVER_CONFIG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    let mut args = args.into_iter().map(Into::into);
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--addr=") {
            addr = value.to_owned();
        } else if let Some(value) = arg.strip_prefix("--config=") {
            config_path = Some(PathBuf::from(value));
        } else {
            match arg.as_str() {
                "--addr" => {
                    addr = args
                        .next()
                        .ok_or_else(|| ServerError::bad_request("missing value for --addr"))?;
                }
                "--config" => {
                    config_path =
                        Some(PathBuf::from(args.next().ok_or_else(|| {
                            ServerError::bad_request("missing value for --config")
                        })?));
                }
                _ => {
                    return Err(ServerError::bad_request(format!(
                        "unknown argument `{arg}`"
                    )));
                }
            }
        }
    }
    let addr = addr
        .parse()
        .map_err(|source| ServerError::bad_request(format!("invalid addr `{addr}`: {source}")))?;
    Ok(ServerOptions { addr, config_path })
}

fn load_server_config(path: Option<&Path>) -> Result<ServerConfig> {
    let Some(path) = path else {
        return Ok(ServerConfig::default());
    };
    let content = fs::read_to_string(path).map_err(|source| {
        ServerError::internal(format!(
            "failed to read server config `{}`: {source}",
            path.display()
        ))
    })?;
    parse_server_config(&content).map_err(|source| {
        ServerError::bad_request(format!(
            "failed to parse server config `{}`: {source}",
            path.display()
        ))
    })
}

fn parse_server_config(content: &str) -> std::result::Result<ServerConfig, toml::de::Error> {
    toml::from_str(content)
}

fn normalize_static_prefix(prefix: &str) -> Result<String> {
    let prefix = prefix.trim();
    if prefix.is_empty() {
        return Err(ServerError::bad_request("static prefix cannot be empty"));
    }
    if !prefix.starts_with('/') {
        return Err(ServerError::bad_request(format!(
            "static prefix must start with `/`: {prefix}"
        )));
    }
    if prefix.contains('*') {
        return Err(ServerError::bad_request(format!(
            "static prefix cannot contain `*`: {prefix}"
        )));
    }
    if prefix != "/" && prefix.contains("//") {
        return Err(ServerError::bad_request(format!(
            "static prefix cannot contain empty path segments: {prefix}"
        )));
    }
    let normalized = if prefix == "/" {
        prefix.to_owned()
    } else {
        prefix.trim_end_matches('/').to_owned()
    };
    Ok(normalized)
}

#[derive(Debug)]
struct ServerOptions {
    addr: SocketAddr,
    config_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerConfig {
    #[serde(default, rename = "static")]
    static_routes: Vec<StaticRouteConfig>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StaticRouteConfig {
    root: PathBuf,
    prefix: String,
}

async fn schema_file(AxumPath(name): AxumPath<String>) -> Result<Response> {
    let Some((_, content)) = SCHEMA_FILES.iter().find(|(file, _)| *file == name) else {
        return Err(ServerError::not_found(format!("schema `{name}` not found")));
    };
    Ok(json_response_bytes(
        content.as_bytes().to_vec(),
        "application/schema+json",
    ))
}

async fn project_files(Json(value): Json<Value>) -> Result<Json<Vec<VirtualFile>>> {
    let files = spawn_blocking(move || create_project_files(value)).await?;
    Ok(Json(files))
}

async fn project_files_tar(Json(value): Json<Value>) -> Result<Response> {
    let bytes = spawn_blocking(move || {
        let files = create_project_files(value)?;
        archive_virtual_files(&files)
    })
    .await?;
    Ok(tar_response(bytes, "project.tar.gz"))
}

async fn project_build(Json(value): Json<Value>) -> Result<Json<Vec<VirtualFile>>> {
    let files = spawn_blocking(move || build_project(value).map(|result| result.files)).await?;
    Ok(Json(files))
}

async fn project_build_tar(Json(value): Json<Value>) -> Result<Response> {
    let bytes = spawn_blocking(move || {
        let result = build_project(value)?;
        archive_directory(&result.dist_dir)
    })
    .await?;
    Ok(tar_response(bytes, "build.tar.gz"))
}

async fn spawn_blocking<T, F>(task: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(task).await?
}

fn create_project_files(value: Value) -> Result<Vec<VirtualFile>> {
    let manifest = parse_manifest(value)?;
    let result = ForgeCore::try_new()?.generate_project_files(&manifest)?;
    Ok(result.files)
}

fn build_project(value: Value) -> Result<BuildOutput> {
    let manifest = parse_manifest(value)?;
    let plan = ForgeCore::try_new()?.create_build_plan(&manifest)?;
    let work = TempDir::new()?;
    let project_dir = work.path().join("project");
    write_virtual_files(&project_dir, &plan.files)?;
    link_node_modules(&project_dir)?;
    run_rollup_build(&project_dir)?;
    let dist_dir = project_dir.join(&plan.expectations.dist_dir);
    validate_systemjs_dist(&dist_dir)?;
    let files = collect_virtual_files(&dist_dir)?;
    Ok(BuildOutput {
        _work: work,
        dist_dir,
        files,
    })
}

fn parse_manifest(value: Value) -> Result<ExtensionManifest> {
    validate_manifest_json(&value)?;
    unwrap_manifest(value).map_err(ServerError::from)
}

fn validate_manifest_json(value: &Value) -> Result<()> {
    let schema = ForgeCore::try_new()?.manifest_schema();
    let validator = jsonschema::validator_for(&schema).map_err(|source| {
        ServerError::internal(format!("failed to compile manifest.schema.json: {source}"))
    })?;
    validator.validate(value).map_err(|source| {
        ServerError::bad_request(format!("manifest schema validation failed: {source}"))
    })
}

fn write_virtual_files(root: &Path, files: &[VirtualFile]) -> Result<()> {
    fs::create_dir_all(root)?;
    let mut stable = files.to_vec();
    stable.sort_by(|left, right| left.path.cmp(&right.path));
    for file in stable {
        let full = safe_join(root, &file.path)?;
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full, file.content)?;
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
        return Err(ServerError::internal(format!(
            "FORGE_NODE_MODULES_DIR does not exist: {}",
            node_modules_dir.display()
        )));
    }
    let link_path = project_dir.join("node_modules");
    if link_path.exists() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&node_modules_dir, &link_path)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = project_dir;
        Err(ServerError::internal(
            "FORGE_NODE_MODULES_DIR linking is not supported on this platform",
        ))
    }
}

fn run_rollup_build(project_dir: &Path) -> Result<()> {
    let timeout = env::var("FORGE_BUILD_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(30_000));

    let mut command = rollup_command(&["-c"]);
    let command_label = "rollup -c".to_owned();
    let mut child = command
        .current_dir(project_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let started = Instant::now();

    loop {
        if started.elapsed() > timeout {
            let _ = child.kill();
            let output = child.wait_with_output()?;
            return Err(ServerError::internal(format!(
                "{command_label} timed out after {}ms\nstdout:\n{}\nstderr:\n{}",
                timeout.as_millis(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        if child.try_wait()?.is_some() {
            let output = child.wait_with_output()?;
            if output.status.success() {
                return Ok(());
            }
            return Err(ServerError::internal(format!(
                "{command_label} failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn rollup_command(args: &[&str]) -> Command {
    if let Ok(rollup_js) = env::var("FORGE_ROLLUP_JS") {
        let mut command =
            Command::new(env::var("FORGE_NODE_BIN").unwrap_or_else(|_| "node".into()));
        command.arg(rollup_js);
        command.args(args);
        return command;
    }
    if let Ok(rollup_bin) = env::var("FORGE_ROLLUP_BIN") {
        let mut command = Command::new(rollup_bin);
        command.args(args);
        return command;
    }
    let mut command = Command::new("pnpm");
    command.args(["exec", "rollup"]);
    command.args(args);
    command
}

fn validate_systemjs_dist(dist_dir: &Path) -> Result<()> {
    if !dist_dir.is_dir() {
        return Err(ServerError::internal(format!(
            "missing Rollup dist directory {}",
            dist_dir.display()
        )));
    }
    let js_files = collect_files(dist_dir)?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "js"))
        .collect::<Vec<_>>();
    if js_files.is_empty() {
        return Err(ServerError::internal(format!(
            "Rollup dist directory has no JavaScript output: {}",
            dist_dir.display()
        )));
    }

    let mut has_system_register = false;
    for path in js_files {
        let content = fs::read_to_string(&path)?;
        has_system_register = has_system_register || content.contains("System.register");
        let executable_content = strip_js_comments(&content);
        for token in ["__webpack_require__", "webpackChunk", "import("] {
            if executable_content.contains(token) {
                return Err(ServerError::internal(format!(
                    "illegal output {} contains forbidden token `{token}`",
                    path.display()
                )));
            }
        }
    }
    if !has_system_register {
        return Err(ServerError::internal(
            "illegal output: missing System.register",
        ));
    }
    Ok(())
}

fn strip_js_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                out.push(' ');
                out.push(' ');
                for comment_ch in chars.by_ref() {
                    if comment_ch == '\n' {
                        out.push('\n');
                        break;
                    }
                    out.push(' ');
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                out.push(' ');
                out.push(' ');
                while let Some(comment_ch) = chars.next() {
                    if comment_ch == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        out.push(' ');
                        out.push(' ');
                        break;
                    }
                    if comment_ch == '\n' {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                }
            }
            _ => out.push(ch),
        }
    }

    out
}

fn archive_virtual_files(files: &[VirtualFile]) -> Result<Vec<u8>> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = tar::Builder::new(encoder);
    let mut stable = files.to_vec();
    stable.sort_by(|left, right| left.path.cmp(&right.path));
    for file in stable {
        append_tar_bytes(&mut builder, &file.path, file.content.as_bytes())?;
    }
    let encoder = builder.into_inner()?;
    Ok(encoder.finish()?)
}

fn archive_directory(root: &Path) -> Result<Vec<u8>> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for path in collect_files(root)? {
        let rel = path
            .strip_prefix(root)
            .map_err(|_| ServerError::internal(format!("invalid file path `{}`", path.display())))?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = fs::read(&path)?;
        append_tar_bytes(&mut builder, &rel, &bytes)?;
    }
    let encoder = builder.into_inner()?;
    Ok(encoder.finish()?)
}

fn append_tar_bytes<W: io::Write>(
    builder: &mut tar::Builder<W>,
    path: &str,
    bytes: &[u8],
) -> Result<()> {
    let path = normalize_rel_path(path)?;
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, path, bytes)?;
    Ok(())
}

fn collect_virtual_files(root: &Path) -> Result<Vec<VirtualFile>> {
    let mut out = Vec::new();
    for path in collect_files(root)? {
        let rel = path
            .strip_prefix(root)
            .map_err(|_| ServerError::internal(format!("invalid file path `{}`", path.display())))?
            .to_string_lossy()
            .replace('\\', "/");
        let content = fs::read_to_string(&path)?;
        out.push(VirtualFile { path: rel, content });
    }
    Ok(out)
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_inner(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files_inner(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files_inner(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn safe_join(root: &Path, rel_path: &str) -> Result<PathBuf> {
    let normalized = normalize_rel_path(rel_path)?;
    Ok(root.join(normalized))
}

fn normalize_rel_path(path: &str) -> Result<PathBuf> {
    if path.is_empty() {
        return Err(ServerError::bad_request("invalid empty file path"));
    }
    let path = path.replace('\\', "/");
    if path.starts_with('/') {
        return Err(ServerError::bad_request(format!(
            "absolute path is not allowed: {path}"
        )));
    }
    let mut out = PathBuf::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(ServerError::bad_request(format!("path traversal: {path}"))),
            _ => out.push(part),
        }
    }
    if out.as_os_str().is_empty() {
        return Err(ServerError::bad_request(format!(
            "invalid file path `{path}`"
        )));
    }
    Ok(out)
}

fn tar_response(bytes: Vec<u8>, filename: &'static str) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/gzip"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static(match filename {
            "project.tar.gz" => "attachment; filename=\"project.tar.gz\"",
            "build.tar.gz" => "attachment; filename=\"build.tar.gz\"",
            _ => "attachment",
        }),
    );
    (headers, Bytes::from(bytes)).into_response()
}

fn json_response_bytes(bytes: Vec<u8>, content_type: &'static str) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    (headers, Bytes::from(bytes)).into_response()
}

struct BuildOutput {
    _work: TempDir,
    dist_dir: PathBuf,
    files: Vec<VirtualFile>,
}

#[derive(Debug)]
struct ServerError {
    status: StatusCode,
    message: String,
}

impl ServerError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ServerError {}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

impl From<forge_core::Error> for ServerError {
    fn from(source: forge_core::Error) -> Self {
        Self::internal(source.to_string())
    }
}

impl From<forge_project_generator::Error> for ServerError {
    fn from(source: forge_project_generator::Error) -> Self {
        Self::bad_request(source.to_string())
    }
}

impl From<io::Error> for ServerError {
    fn from(source: io::Error) -> Self {
        Self::internal(source.to_string())
    }
}

impl From<tokio::task::JoinError> for ServerError {
    fn from(source: tokio::task::JoinError) -> Self {
        Self::internal(source.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        normalize_static_prefix, parse_server_config, parse_server_options_from, strip_js_comments,
    };

    #[test]
    fn js_comment_import_type_does_not_count_as_dynamic_import() {
        let stripped = strip_js_comments(
            "System.register('x', [], function () {});\n/** @type {import('./types').Thing} */\nconst value = 1;",
        );

        assert!(stripped.contains("System.register"));
        assert!(!stripped.contains("import("));
    }

    #[test]
    fn parses_static_server_config() {
        let config = parse_server_config(
            r#"
[[static]]
root = "/app/server/v4dist"
prefix = "/dist/frontend-forge/"
"#,
        )
        .expect("server config should parse");

        assert_eq!(config.static_routes.len(), 1);
        assert_eq!(
            config.static_routes[0].root,
            PathBuf::from("/app/server/v4dist")
        );
        assert_eq!(config.static_routes[0].prefix, "/dist/frontend-forge/");
    }

    #[test]
    fn normalizes_static_prefix_trailing_slash() {
        assert_eq!(
            normalize_static_prefix("/dist/frontend-forge/").expect("prefix should normalize"),
            "/dist/frontend-forge"
        );
        assert_eq!(
            normalize_static_prefix("/").expect("root prefix should be allowed"),
            "/"
        );
    }

    #[test]
    fn rejects_invalid_static_prefix() {
        assert!(normalize_static_prefix("dist/frontend-forge/").is_err());
        assert!(normalize_static_prefix("/dist//frontend-forge/").is_err());
        assert!(normalize_static_prefix("/dist/*").is_err());
    }

    #[test]
    fn parses_server_options_config_arg() {
        let options = parse_server_options_from([
            "--addr",
            "127.0.0.1:3901",
            "--config",
            "examples/server.toml",
        ])
        .expect("server options should parse");

        assert_eq!(options.addr.to_string(), "127.0.0.1:3901");
        assert_eq!(
            options.config_path,
            Some(PathBuf::from("examples/server.toml"))
        );
    }
}
