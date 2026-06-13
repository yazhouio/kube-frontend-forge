use std::{
    env, fs, io,
    net::SocketAddr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use flate2::{Compression, write::GzEncoder};
use forge_core::ForgeCore;
use forge_project_generator::{ExtensionManifest, VirtualFile, unwrap_manifest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempfile::TempDir;
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::Level;
use tracing_subscriber::{EnvFilter, fmt};

mod systemjs_validator;

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
    init_logging();
    if let Err(error) = run().await {
        tracing::error!("{error}");
        std::process::exit(1);
    }
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "frontend_forge_server=info,tower_http=info,forge_project_generator=info,forge_component_generator=info",
        )
    });
    let _ = fmt().with_env_filter(filter).try_init();
}

async fn run() -> Result<()> {
    let options = parse_server_options()?;
    let config = load_server_config(&options)?;
    let addr = parse_bind_addr(&config.addr)?;
    let app = build_router(config)?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("frontend-forge-server listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_router(config: ServerConfig) -> Result<Router> {
    let state = AppState::try_new()?;
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

    Ok(app.with_state(state).layer(
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
            .on_response(DefaultOnResponse::new().level(Level::INFO)),
    ))
}

fn parse_server_options() -> Result<ServerOptions> {
    parse_server_options_from(env::args().skip(1))
}

fn parse_server_options_from<I, S>(args: I) -> Result<ServerOptions>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut addr = None;
    let mut config_path = None;
    let mut args = args.into_iter().map(Into::into);
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--addr=") {
            addr = Some(value.to_owned());
        } else if let Some(value) = arg.strip_prefix("--config=") {
            config_path = Some(PathBuf::from(value));
        } else {
            match arg.as_str() {
                "--addr" => {
                    addr = Some(
                        args.next()
                            .ok_or_else(|| ServerError::bad_request("missing value for --addr"))?,
                    );
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
    Ok(ServerOptions { addr, config_path })
}

fn load_server_config(options: &ServerOptions) -> Result<ServerConfig> {
    let config_path = options.config_path.clone().or_else(env_server_config_path);

    let mut figment = Figment::from(Serialized::defaults(ServerConfig::default()));
    if let Some(path) = config_path {
        figment = figment.merge(Toml::file(path));
    }
    figment = figment.merge(Env::prefixed("FRONTEND_FORGE_SERVER_").ignore(&["config"]));
    if let Some(addr) = &options.addr {
        figment = figment.merge(Serialized::default("addr", addr));
    }

    figment.extract().map_err(|source| {
        ServerError::bad_request(format!("failed to load server config: {source}"))
    })
}

fn env_server_config_path() -> Option<PathBuf> {
    Figment::from(Env::prefixed("FRONTEND_FORGE_SERVER_"))
        .extract_inner::<String>("config")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
fn parse_server_config(content: &str) -> std::result::Result<ServerConfig, Box<figment::Error>> {
    Figment::from(Serialized::defaults(ServerConfig::default()))
        .merge(Toml::string(content))
        .extract()
        .map_err(Box::new)
}

fn parse_bind_addr(addr: &str) -> Result<SocketAddr> {
    addr.parse()
        .map_err(|source| ServerError::bad_request(format!("invalid addr `{addr}`: {source}")))
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
    addr: Option<String>,
    config_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerConfig {
    #[serde(default = "default_server_addr")]
    addr: String,
    #[serde(default, rename = "static")]
    static_routes: Vec<StaticRouteConfig>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            addr: default_server_addr(),
            static_routes: Vec::new(),
        }
    }
}

fn default_server_addr() -> String {
    "127.0.0.1:3000".to_owned()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticRouteConfig {
    root: PathBuf,
    prefix: String,
}

#[derive(Clone)]
struct AppState {
    core: Arc<ForgeCore>,
    manifest_validator: Arc<jsonschema::Validator>,
}

impl AppState {
    fn try_new() -> Result<Self> {
        let core = Arc::new(ForgeCore::try_new()?);
        let schema = core.manifest_schema();
        let manifest_validator = jsonschema::validator_for(&schema).map_err(|source| {
            ServerError::internal(format!("failed to compile manifest.schema.json: {source}"))
        })?;
        Ok(Self {
            core,
            manifest_validator: Arc::new(manifest_validator),
        })
    }
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

async fn project_files(
    State(state): State<AppState>,
    Json(value): Json<Value>,
) -> Result<Json<ApiSuccess<ProjectFilesPayload>>> {
    let files = spawn_blocking(move || create_project_files(&state, value)).await?;
    Ok(Json(ApiSuccess::new(ProjectFilesPayload { files })))
}

async fn project_files_tar(
    State(state): State<AppState>,
    Json(value): Json<Value>,
) -> Result<Response> {
    let bytes = spawn_blocking(move || {
        let files = create_project_files(&state, value)?;
        archive_virtual_files(&files)
    })
    .await?;
    Ok(tar_response(bytes, "project.tar.gz"))
}

async fn project_build(
    State(state): State<AppState>,
    Json(value): Json<Value>,
) -> Result<Json<ApiSuccess<ProjectFilesPayload>>> {
    let files =
        spawn_blocking(move || build_project(&state, value).map(|result| result.files)).await?;
    Ok(Json(ApiSuccess::new(ProjectFilesPayload { files })))
}

async fn project_build_tar(
    State(state): State<AppState>,
    Json(value): Json<Value>,
) -> Result<Response> {
    let bytes = spawn_blocking(move || {
        let result = build_project(&state, value)?;
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

fn create_project_files(state: &AppState, value: Value) -> Result<Vec<VirtualFile>> {
    let manifest = parse_manifest(state, value)?;
    let result = state.core.generate_project_files(&manifest)?;
    Ok(result.files)
}

fn build_project(state: &AppState, value: Value) -> Result<BuildOutput> {
    let manifest = parse_manifest(state, value)?;
    let plan = state.core.create_build_plan(&manifest)?;
    let work = TempDir::new()?;
    let project_dir = work.path().join("project");
    write_virtual_files(&project_dir, &plan.files)?;
    link_node_modules(&project_dir)?;
    run_build_script(&project_dir)?;
    let dist_dir = project_dir.join(&plan.expectations.dist_dir);
    validate_dist(&dist_dir, plan.expectations.systemjs)?;
    let files = collect_virtual_files(&dist_dir)?;
    Ok(BuildOutput {
        _work: work,
        dist_dir,
        files,
    })
}

fn parse_manifest(state: &AppState, value: Value) -> Result<ExtensionManifest> {
    validate_manifest_json(state, &value)?;
    unwrap_manifest(value).map_err(ServerError::from)
}

fn validate_manifest_json(state: &AppState, value: &Value) -> Result<()> {
    state.manifest_validator.validate(value).map_err(|source| {
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
            log_build_script_output(&output.stdout, &output.stderr);
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

fn log_build_script_output(stdout: &[u8], stderr: &[u8]) {
    for line in String::from_utf8_lossy(stdout).lines() {
        if !line.trim().is_empty() {
            tracing::info!(stream = "stdout", "{line}");
        }
    }
    for line in String::from_utf8_lossy(stderr).lines() {
        if !line.trim().is_empty() {
            tracing::info!(stream = "stderr", "{line}");
        }
    }
}

fn build_script_command() -> Command {
    let mut command = Command::new(env::var("FORGE_NODE_BIN").unwrap_or_else(|_| "node".into()));
    command.arg("build.mjs");
    command
}

fn validate_dist(dist_dir: &Path, expect_systemjs: bool) -> Result<()> {
    if !dist_dir.is_dir() {
        return Err(ServerError::internal(format!(
            "missing build dist directory {}",
            dist_dir.display()
        )));
    }
    let js_files = collect_files(dist_dir)?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "js"))
        .collect::<Vec<_>>();
    if js_files.is_empty() {
        return Err(ServerError::internal(format!(
            "build dist directory has no JavaScript output: {}",
            dist_dir.display()
        )));
    }

    let mut has_system_register = false;
    for path in js_files {
        let content = fs::read_to_string(&path)?;
        if expect_systemjs {
            match systemjs_validator::validate_systemjs_code(&content) {
                Ok(validation) => {
                    has_system_register = has_system_register || validation.has_system_register;
                }
                Err(systemjs_validator::SystemJsValidationError::Parse { message }) => {
                    return Err(ServerError::internal(format!(
                        "illegal output {} failed JavaScript parse: {message}",
                        path.display()
                    )));
                }
                Err(systemjs_validator::SystemJsValidationError::ForbiddenToken { token }) => {
                    return Err(ServerError::internal(format!(
                        "illegal output {} contains forbidden token `{token}`",
                        path.display()
                    )));
                }
            }
        } else {
            match systemjs_validator::validate_systemjs_code(&content) {
                Ok(_) => {}
                Err(systemjs_validator::SystemJsValidationError::Parse { message }) => {
                    return Err(ServerError::internal(format!(
                        "illegal output {} failed JavaScript parse: {message}",
                        path.display()
                    )));
                }
                Err(systemjs_validator::SystemJsValidationError::ForbiddenToken { token }) => {
                    return Err(ServerError::internal(format!(
                        "illegal output {} contains forbidden token `{token}`",
                        path.display()
                    )));
                }
            }
        }
    }
    if expect_systemjs && !has_system_register {
        return Err(ServerError::internal(
            "illegal output: missing System.register",
        ));
    }
    Ok(())
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

#[derive(Debug, Serialize)]
struct ApiSuccess<T> {
    ok: bool,
    #[serde(flatten)]
    data: T,
}

impl<T> ApiSuccess<T> {
    fn new(data: T) -> Self {
        Self { ok: true, data }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectFilesPayload {
    files: Vec<VirtualFile>,
}

#[derive(Debug, Serialize)]
struct ApiErrorBody {
    ok: bool,
    error: String,
    message: String,
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
        let message = self.message;
        (
            self.status,
            Json(ApiErrorBody {
                ok: false,
                error: message.clone(),
                message,
            }),
        )
            .into_response()
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

    use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
    use forge_project_generator::VirtualFile;
    use serde_json::json;

    use super::{
        ApiSuccess, ProjectFilesPayload, ServerError, normalize_static_prefix, parse_server_config,
        parse_server_options_from,
    };

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

        assert_eq!(config.addr, "127.0.0.1:3000");
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

        assert_eq!(options.addr.as_deref(), Some("127.0.0.1:3901"));
        assert_eq!(
            options.config_path,
            Some(PathBuf::from("examples/server.toml"))
        );
    }

    #[test]
    fn parses_server_config_addr() {
        let config = parse_server_config(
            r#"
addr = "0.0.0.0:3000"
"#,
        )
        .expect("server config should parse");

        assert_eq!(config.addr, "0.0.0.0:3000");
    }

    #[test]
    fn project_files_response_matches_legacy_node_shape() {
        let response = ApiSuccess::new(ProjectFilesPayload {
            files: vec![VirtualFile {
                path: "src/index.ts".to_owned(),
                content: "export {};".to_owned(),
            }],
        });

        let value = serde_json::to_value(response).expect("response should serialize");

        assert_eq!(
            value,
            json!({
                "ok": true,
                "files": [
                    {
                        "path": "src/index.ts",
                        "content": "export {};"
                    }
                ]
            })
        );
    }

    #[tokio::test]
    async fn server_error_response_matches_legacy_node_shape() {
        let response = ServerError::bad_request("manifest is required").into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable");
        let value: serde_json::Value =
            serde_json::from_slice(&body).expect("response body should be JSON");

        assert_eq!(
            value,
            json!({
                "ok": false,
                "error": "manifest is required",
                "message": "manifest is required"
            })
        );
    }
}
