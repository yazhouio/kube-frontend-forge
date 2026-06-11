use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::Deserialize;

use crate::registry::{
    DataSourceSource, DataSourceSourceGenerateCode, DataSourceSourceSchema, NodeSource,
    NodeSourceGenerateCode, NodeSourceMeta, NodeSourceSchema, Registry, StatSource, StatementScope,
    TemplateDefault, TemplateInput, TemplateOutput,
};
use crate::value::{DataSourceActionMode, DataSourceCallMode};

const NODE_SOURCE_JSON: &[(&str, &str)] = &[
    (
        "layout.json",
        include_str!("../sources/node-sources/layout.json"),
    ),
    (
        "text.json",
        include_str!("../sources/node-sources/text.json"),
    ),
    (
        "iframe.json",
        include_str!("../sources/node-sources/iframe.json"),
    ),
    (
        "crd-table.json",
        include_str!("../sources/node-sources/crd-table.json"),
    ),
];

const DATA_SOURCE_SOURCE_JSON: &[(&str, &str)] = &[
    (
        "static.json",
        include_str!("../sources/data-source-sources/static.json"),
    ),
    (
        "rest.json",
        include_str!("../sources/data-source-sources/rest.json"),
    ),
    (
        "crd-columns.json",
        include_str!("../sources/data-source-sources/crd-columns.json"),
    ),
    (
        "crd-page-state.json",
        include_str!("../sources/data-source-sources/crd-page-state.json"),
    ),
    (
        "workspace-crd-page-state.json",
        include_str!("../sources/data-source-sources/workspace-crd-page-state.json"),
    ),
];

pub fn default_registry() -> Registry {
    let mut registry = Registry::default();
    for (label, raw) in NODE_SOURCE_JSON {
        registry.register_node(parse_node_source(label, raw));
    }
    for (label, raw) in DATA_SOURCE_SOURCE_JSON {
        registry.register_data_source(parse_data_source_source(label, raw));
    }
    registry
}

fn parse_node_source(label: &str, raw: &str) -> NodeSource {
    let source = serde_json::from_str::<RawNodeSource>(raw)
        .unwrap_or_else(|source| panic!("failed to parse bundled NodeSource {label}: {source}"));
    source.into_node_source()
}

fn parse_data_source_source(label: &str, raw: &str) -> DataSourceSource {
    let source = serde_json::from_str::<RawDataSourceSource>(raw).unwrap_or_else(|source| {
        panic!("failed to parse bundled DataSourceSource {label}: {source}")
    });
    source.into_data_source_source()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawNodeSource {
    id: String,
    #[serde(default)]
    schema: RawNodeSourceSchema,
    generate_code: RawNodeSourceGenerateCode,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawNodeSourceSchema {
    #[serde(default)]
    template_inputs: BTreeMap<String, RawTemplateInput>,
    #[serde(default)]
    runtime_props: BTreeMap<String, RawTemplateInput>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawNodeSourceGenerateCode {
    #[serde(default)]
    imports: Vec<String>,
    #[serde(default)]
    stats: Vec<RawStatSource>,
    #[serde(default)]
    jsx: Option<String>,
    #[serde(default)]
    meta: Option<RawNodeSourceMeta>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDataSourceSource {
    id: String,
    #[serde(default)]
    schema: RawDataSourceSourceSchema,
    generate_code: RawDataSourceSourceGenerateCode,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDataSourceSourceSchema {
    #[serde(default)]
    template_inputs: BTreeMap<String, RawTemplateInput>,
    #[serde(default)]
    outputs: BTreeMap<String, RawTemplateOutput>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDataSourceSourceGenerateCode {
    #[serde(default)]
    imports: Vec<String>,
    #[serde(default)]
    stats: Vec<RawStatSource>,
    #[serde(default)]
    meta: Option<RawNodeSourceMeta>,
    #[serde(default)]
    call_mode: Option<RawDataSourceCallMode>,
    #[serde(default)]
    action_mode: Option<RawDataSourceActionMode>,
    #[serde(default)]
    defaults: BTreeMap<String, RawTemplateDefault>,
}

#[derive(Deserialize)]
struct RawTemplateInput {
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize)]
struct RawTemplateOutput {
    #[serde(rename = "type")]
    ty: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawStatSource {
    id: String,
    scope: RawStatementScope,
    code: String,
    #[serde(default)]
    output: Vec<String>,
    #[serde(default)]
    depends: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum RawStatementScope {
    ModuleImport,
    ModuleDecl,
    ModuleInit,
    FunctionDecl,
    FunctionBody,
    Block,
    ControlFlow,
    Jsx,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawNodeSourceMeta {
    #[serde(default)]
    input_paths: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    runtime_deps: Vec<String>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum RawDataSourceCallMode {
    Hook,
    Value,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum RawDataSourceActionMode {
    Set,
    Request,
    Mutate,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawTemplateDefault {
    Expr(String),
    Object {
        #[serde(rename = "type")]
        ty: String,
    },
}

impl RawNodeSource {
    fn into_node_source(self) -> NodeSource {
        NodeSource::new(
            leak(self.id),
            NodeSourceGenerateCode {
                imports: self.generate_code.imports.into_iter().map(leak).collect(),
                stats: self
                    .generate_code
                    .stats
                    .into_iter()
                    .map(RawStatSource::into_stat_source)
                    .collect(),
                jsx: self.generate_code.jsx.map(leak),
                meta: self.generate_code.meta.map(RawNodeSourceMeta::into_meta),
            },
        )
        .with_schema(NodeSourceSchema {
            template_inputs: template_inputs(self.schema.template_inputs),
            runtime_props: template_inputs(self.schema.runtime_props),
        })
    }
}

impl RawDataSourceSource {
    fn into_data_source_source(self) -> DataSourceSource {
        DataSourceSource::new(
            leak(self.id),
            DataSourceSourceGenerateCode {
                imports: self.generate_code.imports.into_iter().map(leak).collect(),
                stats: self
                    .generate_code
                    .stats
                    .into_iter()
                    .map(RawStatSource::into_stat_source)
                    .collect(),
                meta: self.generate_code.meta.map(RawNodeSourceMeta::into_meta),
                call_mode: self
                    .generate_code
                    .call_mode
                    .map(Into::into)
                    .unwrap_or_default(),
                action_mode: self
                    .generate_code
                    .action_mode
                    .map(Into::into)
                    .unwrap_or_default(),
                defaults: self
                    .generate_code
                    .defaults
                    .into_iter()
                    .map(|(key, value)| (leak(key), value.into_template_default()))
                    .collect(),
            },
        )
        .with_schema(DataSourceSourceSchema {
            template_inputs: template_inputs(self.schema.template_inputs),
            outputs: self
                .schema
                .outputs
                .into_iter()
                .map(|(key, output)| (leak(key), output.into_template_output()))
                .collect(),
        })
    }
}

impl RawTemplateInput {
    fn into_template_input(self) -> TemplateInput {
        TemplateInput {
            ty: leak(self.ty),
            description: leak(self.description),
        }
    }
}

impl RawTemplateOutput {
    fn into_template_output(self) -> TemplateOutput {
        TemplateOutput { ty: leak(self.ty) }
    }
}

impl RawStatSource {
    fn into_stat_source(self) -> StatSource {
        StatSource {
            id: leak(self.id),
            scope: self.scope.into(),
            code: leak(self.code),
            output: self.output.into_iter().map(leak).collect(),
            depends: self.depends.into_iter().map(leak).collect(),
        }
    }
}

impl From<RawStatementScope> for StatementScope {
    fn from(scope: RawStatementScope) -> Self {
        match scope {
            RawStatementScope::ModuleImport => Self::ModuleImport,
            RawStatementScope::ModuleDecl => Self::ModuleDecl,
            RawStatementScope::ModuleInit => Self::ModuleInit,
            RawStatementScope::FunctionDecl => Self::FunctionDecl,
            RawStatementScope::FunctionBody => Self::FunctionBody,
            RawStatementScope::Block => Self::Block,
            RawStatementScope::ControlFlow => Self::ControlFlow,
            RawStatementScope::Jsx => Self::Jsx,
        }
    }
}

impl RawNodeSourceMeta {
    fn into_meta(self) -> NodeSourceMeta {
        NodeSourceMeta {
            input_paths: self
                .input_paths
                .into_iter()
                .map(|(key, paths)| (leak(key), paths.into_iter().map(leak).collect()))
                .collect(),
            runtime_deps: self.runtime_deps.into_iter().map(leak).collect(),
        }
    }
}

impl From<RawDataSourceCallMode> for DataSourceCallMode {
    fn from(mode: RawDataSourceCallMode) -> Self {
        match mode {
            RawDataSourceCallMode::Hook => Self::Hook,
            RawDataSourceCallMode::Value => Self::Value,
        }
    }
}

impl From<RawDataSourceActionMode> for DataSourceActionMode {
    fn from(mode: RawDataSourceActionMode) -> Self {
        match mode {
            RawDataSourceActionMode::Set => Self::Set,
            RawDataSourceActionMode::Request => Self::Request,
            RawDataSourceActionMode::Mutate => Self::Mutate,
        }
    }
}

impl RawTemplateDefault {
    fn into_template_default(self) -> TemplateDefault {
        match self {
            RawTemplateDefault::Expr(code) => TemplateDefault::Expr(leak(code)),
            RawTemplateDefault::Object { ty } if ty == "dataSourceIdJson" => {
                TemplateDefault::DataSourceIdJson
            }
            RawTemplateDefault::Object { ty } => {
                panic!("unsupported template default type `{ty}`")
            }
        }
    }
}

fn template_inputs(
    inputs: BTreeMap<String, RawTemplateInput>,
) -> IndexMap<&'static str, TemplateInput> {
    inputs
        .into_iter()
        .map(|(key, input)| (leak(key), input.into_template_input()))
        .collect()
}

fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

#[cfg(test)]
mod tests {
    use jsonschema::validator_for;
    use serde_json::Value;

    use super::{DATA_SOURCE_SOURCE_JSON, NODE_SOURCE_JSON};
    use crate::schema::{data_source_source_schema, node_source_schema};

    #[test]
    fn bundled_node_sources_match_node_source_schema() {
        let schema = node_source_schema();
        let validator = validator_for(&schema).unwrap();

        for (label, raw) in NODE_SOURCE_JSON {
            let value = serde_json::from_str::<Value>(raw).unwrap();
            validator
                .validate(&value)
                .unwrap_or_else(|err| panic!("{label} does not match node-source schema: {err}"));
        }
    }

    #[test]
    fn bundled_data_source_sources_match_data_source_source_schema() {
        let schema = data_source_source_schema();
        let validator = validator_for(&schema).unwrap();

        for (label, raw) in DATA_SOURCE_SOURCE_JSON {
            let value = serde_json::from_str::<Value>(raw).unwrap();
            validator.validate(&value).unwrap_or_else(|err| {
                panic!("{label} does not match data-source-source schema: {err}")
            });
        }
    }
}
