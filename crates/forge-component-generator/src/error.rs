use snafu::Snafu;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display(
        "usage: forge-component-generator <page-schema.json> [--backend oxc|swc] [--out page.tsx]"
    ))]
    MissingInputPath,

    #[snafu(display("invalid backend `{backend}`; expected `swc` or `oxc`"))]
    InvalidBackend { backend: String },

    #[snafu(display("backend `{backend}` requires cargo feature `{feature}`"))]
    BackendFeatureDisabled { backend: String, feature: String },

    #[snafu(display("missing value for --backend; expected `oxc` or `swc`"))]
    MissingBackendValue,

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

    #[snafu(display("pageSchema is required"))]
    MissingPageSchema,

    #[snafu(display("failed to parse expression `{code}`: {message}"))]
    ParseExpression { code: String, message: String },

    #[snafu(display("failed to parse module item `{code}`: {message}"))]
    ParseModuleItem { code: String, message: String },

    #[snafu(display("failed to emit generated module: {message}"))]
    EmitModule { message: String },

    #[snafu(display("generated module was not valid utf8"))]
    GeneratedUtf8 { source: std::string::FromUtf8Error },

    #[snafu(display("node definition not found: {id}"))]
    NodeNotFound { id: String },

    #[snafu(display("data source definition not found: {id}"))]
    DataSourceNotFound { id: String },

    #[snafu(display("binding source not found: {id}"))]
    BindingSourceNotFound { id: String },

    #[snafu(display("failed to render node {node_id} ({node_type}): {source}"))]
    RenderNode {
        node_id: String,
        node_type: String,
        source: Box<Error>,
    },

    #[snafu(display("failed to render data source {id} ({ty}): {source}"))]
    RenderDataSource {
        id: String,
        ty: String,
        source: Box<Error>,
    },

    #[snafu(display("expression value requires string code"))]
    ExpressionCodeRequired,

    #[snafu(display("binding value must be an object"))]
    BindingObjectRequired,

    #[snafu(display("invalid json value: {source}"))]
    JsonValue { source: serde_json::Error },

    #[snafu(display("component name cannot be empty"))]
    EmptyComponentName,

    #[snafu(display("node source {id} is missing jsx template"))]
    MissingJsxTemplate { id: String },

    #[snafu(display("failed to validate {owner} {part} template: {source}"))]
    TemplateValidation {
        owner: String,
        part: String,
        source: Box<Error>,
    },

    #[snafu(display("{owner} template meta references unknown target {target}"))]
    TemplateMetaTargetNotFound { owner: String, target: String },

    #[snafu(display(
        "{owner} {part} template placeholder %%{placeholder}%% is not declared in meta.inputPaths"
    ))]
    TemplatePlaceholderNotDeclared {
        owner: String,
        part: String,
        placeholder: String,
    },

    #[snafu(display("stat {owner}.{stat} depends on missing stat {dependency}"))]
    StatDependencyNotFound {
        owner: String,
        stat: String,
        dependency: String,
    },

    #[snafu(display("stat dependency cycle detected at {owner}.{stat}"))]
    StatDependencyCycle { owner: String, stat: String },

    #[snafu(display("invalid action trigger `{trigger}` in actionGraph {graph_id}"))]
    InvalidActionTrigger { graph_id: String, trigger: String },

    #[snafu(display("actionGraph not found: {id}"))]
    ActionGraphNotFound { id: String },

    #[snafu(display("actionGraph {graph_id}.{action_id} targets missing node {node_id}"))]
    ActionGraphTargetNodeNotFound {
        graph_id: String,
        action_id: String,
        node_id: String,
    },

    #[snafu(display(
        "actionGraph {graph_id}.{action_id} calls missing data source {data_source_id}"
    ))]
    ActionGraphDataSourceNotFound {
        graph_id: String,
        action_id: String,
        data_source_id: String,
    },

    #[snafu(display(
        "actionGraph {graph_id}.{action_id} cannot call non-hook data source {data_source_id}"
    ))]
    ActionGraphDataSourceNotCallable {
        graph_id: String,
        action_id: String,
        data_source_id: String,
    },

    #[snafu(display("binding source {source_id} is ambiguous (dataSource and actionGraph)"))]
    AmbiguousBindingSource { source_id: String },

    #[snafu(display("{owner} prop {prop} is not defined in schema"))]
    UnknownProp { owner: String, prop: String },

    #[snafu(display("{owner} prop {prop} expected {expected}"))]
    InvalidPropType {
        owner: String,
        prop: String,
        expected: String,
    },

    #[snafu(display("{owner} schema validation failed: {message}"))]
    JsonSchemaValidation { owner: String, message: String },

    #[snafu(display("{owner} schema compile failed: {message}"))]
    JsonSchemaCompile { owner: String, message: String },
}
