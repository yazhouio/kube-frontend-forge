use snafu::Snafu;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("usage: forge-project-generator <manifest.json> [--out result.json]"))]
    MissingInputPath,

    #[snafu(display("missing value for --out"))]
    MissingOutPath,

    #[snafu(display("unknown argument `{arg}`"))]
    UnknownArgument { arg: String },

    #[snafu(display("unexpected extra input path `{path}`"))]
    UnexpectedInputPath { path: String },

    #[snafu(display("failed to read {path}"))]
    ReadFile {
        path: String,
        source: std::io::Error,
    },

    #[snafu(display("failed to write {path}"))]
    WriteFile {
        path: String,
        source: std::io::Error,
    },

    #[snafu(display("failed to parse json {path}"))]
    ParseJson {
        path: String,
        source: serde_json::Error,
    },

    #[snafu(display("failed to serialize generated project: {source}"))]
    SerializeJson { source: serde_json::Error },

    #[snafu(display("failed to render page {page_id}: {message}"))]
    RenderPage { page_id: String, message: String },

    #[snafu(display("failed to parse module imports in {path}: {message}"))]
    ParseModuleImports { path: String, message: String },

    #[snafu(display("manifest is required"))]
    MissingManifest,

    #[snafu(display("{label} must be a non-empty string"))]
    NonEmptyString { label: String },

    #[snafu(display("{label} contains unsupported characters: {value}"))]
    UnsafeFileName { label: String, value: String },

    #[snafu(display("manifest.version must be \"1.0\""))]
    InvalidManifestVersion,

    #[snafu(display("manifest.build.target must be \"kubesphere-extension\""))]
    InvalidBuildTarget,

    #[snafu(display("manifest.build.systemjs must be a boolean"))]
    InvalidBuildSystemjs,

    #[snafu(display("pages.id must be unique"))]
    DuplicatePageId,

    #[snafu(display("locales.lang must be unique"))]
    DuplicateLocaleLang,

    #[snafu(display("routes[{index}].pageId not found in pages: {page_id}"))]
    RoutePageNotFound { index: usize, page_id: String },

    #[snafu(display("invalid file path"))]
    InvalidFilePath,

    #[snafu(display("absolute path is not allowed"))]
    AbsolutePath,

    #[snafu(display("path traversal is not allowed"))]
    PathTraversal,

    #[snafu(display("scaffold template not found: {path}"))]
    ScaffoldTemplateNotFound { path: String },

    #[snafu(display("scaffold routes template not found"))]
    ScaffoldRoutesTemplateNotFound,

    #[snafu(display("invalid scaffold locale JSON: {path}: {message}"))]
    InvalidScaffoldLocaleJson { path: String, message: String },

    #[snafu(display("{label}[{key}] must be a string"))]
    LocaleMessageNotString { label: String, key: String },

    #[snafu(display("locales.lang results in duplicate identifiers"))]
    DuplicateLocaleIdentifier,
}
