use std::sync::OnceLock;

use indexmap::IndexMap;
use serde_json::Value;

use crate::code_backend::JsCodeBackend;
use crate::error::{Error, Result};
use crate::imports::ImportRegistry;
use crate::model::{ComponentNode, DataSourceNode};
use crate::names::{NameAllocator, sanitize_ident};
use crate::value::{
    BindingContext, BindingUse, DataSourceCallMode, collect_binding_uses,
    value_to_expr_code_with_context,
};

#[derive(Default)]
pub struct RenderContext {
    pub imports: ImportRegistry,
    pub module_items: Vec<String>,
    pub bindings: BindingContext,
    pub rendered_data_sources: indexmap::IndexSet<String>,
    pub module_names: NameAllocator,
    pub rendered_action_graph_stores: indexmap::IndexSet<String>,
}

pub struct NodeRender {
    pub jsx: String,
    pub stats: Vec<RenderedStat>,
    pub binding_uses: Vec<BindingUse>,
}

pub struct RenderedStat {
    pub code: String,
    pub outputs: Vec<String>,
    pub scope: StatementScope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatementScope {
    ModuleImport,
    ModuleDecl,
    ModuleInit,
    FunctionDecl,
    FunctionBody,
    Block,
    ControlFlow,
    Jsx,
}

impl StatementScope {
    pub fn is_module_scope(self) -> bool {
        matches!(
            self,
            Self::ModuleImport | Self::ModuleDecl | Self::ModuleInit | Self::FunctionDecl
        )
    }
}

impl NodeRender {
    pub fn new(jsx: impl Into<String>) -> Self {
        Self {
            jsx: jsx.into(),
            stats: Vec::new(),
            binding_uses: Vec::new(),
        }
    }

    pub fn with_stats(jsx: impl Into<String>, stats: Vec<String>) -> Self {
        Self {
            jsx: jsx.into(),
            stats: stats
                .into_iter()
                .map(|code| RenderedStat {
                    code,
                    outputs: Vec::new(),
                    scope: StatementScope::FunctionBody,
                })
                .collect(),
            binding_uses: Vec::new(),
        }
    }
}

pub trait NodeDefinition: Send + Sync {
    fn id(&self) -> &'static str;
    fn runtime_prop_names(&self) -> Vec<&'static str> {
        Vec::new()
    }
    fn validate_templates(&self, _backend: &dyn JsCodeBackend) -> Result<()> {
        Ok(())
    }
    fn render(
        &self,
        node: &ComponentNode,
        children: Vec<String>,
        ctx: &mut RenderContext,
    ) -> Result<NodeRender>;
}

pub struct NodeSource {
    pub id: &'static str,
    pub schema: NodeSourceSchema,
    pub generate_code: NodeSourceGenerateCode,
    props_validator: OnceLock<jsonschema::Validator>,
}

impl Clone for NodeSource {
    fn clone(&self) -> Self {
        let props_validator = OnceLock::new();
        if let Some(validator) = self.props_validator.get() {
            let _ = props_validator.set(validator.clone());
        }
        Self {
            id: self.id,
            schema: self.schema.clone(),
            generate_code: self.generate_code.clone(),
            props_validator,
        }
    }
}

#[derive(Clone, Default)]
pub struct NodeSourceSchema {
    pub template_inputs: IndexMap<&'static str, TemplateInput>,
    pub runtime_props: IndexMap<&'static str, TemplateInput>,
}

#[derive(Clone)]
pub struct TemplateInput {
    pub ty: &'static str,
    pub description: &'static str,
}

#[derive(Clone)]
pub struct TemplateOutput {
    pub ty: &'static str,
}

#[derive(Clone, Default)]
pub struct NodeSourceGenerateCode {
    pub imports: Vec<&'static str>,
    pub stats: Vec<StatSource>,
    pub jsx: Option<&'static str>,
    pub meta: Option<NodeSourceMeta>,
}

#[derive(Clone)]
pub struct StatSource {
    pub id: &'static str,
    pub scope: StatementScope,
    pub code: &'static str,
    pub output: Vec<&'static str>,
    pub depends: Vec<&'static str>,
}

#[derive(Clone, Default)]
pub struct NodeSourceMeta {
    pub input_paths: IndexMap<&'static str, Vec<&'static str>>,
    pub runtime_deps: Vec<&'static str>,
}

impl NodeDefinition for NodeSource {
    fn id(&self) -> &'static str {
        self.id
    }

    fn runtime_prop_names(&self) -> Vec<&'static str> {
        self.schema.runtime_props.keys().copied().collect()
    }

    fn validate_templates(&self, backend: &dyn JsCodeBackend) -> Result<()> {
        self.validate_definition(backend)
    }

    fn render(
        &self,
        node: &ComponentNode,
        children: Vec<String>,
        ctx: &mut RenderContext,
    ) -> Result<NodeRender> {
        self.validate_props(&node.id, &node.props)?;
        for import in &self.generate_code.imports {
            ctx.imports.add_source(import);
        }

        let ordered_stats = order_stat_sources(self.id, &self.generate_code.stats)?;
        let stats = ordered_stats
            .into_iter()
            .map(|stat| {
                let input_paths = self.input_paths(stat.id);
                self.render_template(stat.code, input_paths, &node.props, &children, ctx)
                    .map(|code| RenderedStat {
                        code,
                        scope: stat.scope,
                        outputs: stat
                            .output
                            .iter()
                            .map(|output| (*output).to_owned())
                            .collect(),
                    })
            })
            .collect::<Result<Vec<_>>>()?;

        let jsx_template = self
            .generate_code
            .jsx
            .ok_or_else(|| Error::MissingJsxTemplate {
                id: self.id.to_owned(),
            })?;
        let jsx = self.render_template(
            jsx_template,
            self.input_paths("$jsx"),
            &node.props,
            &children,
            ctx,
        )?;

        let mut binding_uses = Vec::new();
        if self
            .generate_code
            .meta
            .as_ref()
            .is_some_and(|meta| meta.runtime_deps.iter().any(|dep| *dep == "runtime"))
        {
            binding_uses.push(BindingUse::Runtime);
        }
        for input in self.all_input_paths() {
            if let Some(value) = node.props.get(input) {
                binding_uses.extend(collect_binding_uses(value, &ctx.bindings));
            }
        }

        Ok(NodeRender {
            jsx,
            stats,
            binding_uses,
        })
    }
}

impl NodeSource {
    pub fn new(id: &'static str, generate_code: NodeSourceGenerateCode) -> Self {
        Self {
            id,
            schema: NodeSourceSchema::default(),
            generate_code,
            props_validator: OnceLock::new(),
        }
    }

    pub fn with_schema(mut self, schema: NodeSourceSchema) -> Self {
        self.schema = schema;
        self
    }

    fn input_paths(&self, key: &str) -> &[&'static str] {
        self.generate_code
            .meta
            .as_ref()
            .and_then(|meta| meta.input_paths.get(key))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn all_input_paths(&self) -> Vec<&'static str> {
        let mut out = indexmap::IndexSet::new();
        if let Some(meta) = &self.generate_code.meta {
            for paths in meta.input_paths.values() {
                for path in paths {
                    out.insert(*path);
                }
            }
        }
        out.into_iter().collect()
    }

    fn validate_definition(&self, backend: &dyn JsCodeBackend) -> Result<()> {
        let owner = node_source_owner(self.id);
        self.ensure_props_validator(&owner)?;
        validate_meta_targets(
            &owner,
            self.generate_code
                .meta
                .as_ref()
                .map(|meta| &meta.input_paths),
            self.generate_code.jsx.is_some(),
            self.generate_code.stats.iter().map(|stat| stat.id),
        )?;
        order_stat_sources(self.id, &self.generate_code.stats)?;

        for (index, import) in self.generate_code.imports.iter().enumerate() {
            backend
                .validate_module_items(import)
                .map_err(|source| Error::TemplateValidation {
                    owner: owner.clone(),
                    part: format!("import[{index}]"),
                    source: Box::new(source),
                })?;
        }

        for stat in &self.generate_code.stats {
            let part = format!("stat {}", stat.id);
            let code = render_validation_template(
                &owner,
                &part,
                stat.code,
                self.input_paths(stat.id),
                node_validation_replacement,
            )?;
            backend
                .validate_module_items(&code)
                .map_err(|source| Error::TemplateValidation {
                    owner: owner.clone(),
                    part,
                    source: Box::new(source),
                })?;
        }

        if let Some(jsx) = self.generate_code.jsx {
            let part = "jsx".to_owned();
            let code = render_validation_template(
                &owner,
                &part,
                jsx,
                self.input_paths("$jsx"),
                node_validation_replacement,
            )?;
            backend
                .validate_expr(&code)
                .map_err(|source| Error::TemplateValidation {
                    owner,
                    part,
                    source: Box::new(source),
                })?;
        }

        Ok(())
    }

    fn render_template(
        &self,
        template: &str,
        input_paths: &[&str],
        props: &std::collections::BTreeMap<String, Value>,
        children: &[String],
        ctx: &RenderContext,
    ) -> Result<String> {
        let mut out = template.to_owned();
        let child_jsx = children.join("");
        out = out.replace("<__ENGINE_CHILDREN__ />", &child_jsx);
        out = out.replace("<__ENGINE_CHILDREN__/>", &child_jsx);
        out = out.replace("__ENGINE_CHILDREN__", &child_jsx);

        for input in input_paths {
            let replacement =
                value_to_expr_code_with_context(props.get(*input), "undefined", &ctx.bindings)?;
            out = out.replace(&format!("%%{input}%%"), &replacement);
        }

        Ok(out)
    }

    fn validate_props(
        &self,
        node_id: &str,
        props: &std::collections::BTreeMap<String, Value>,
    ) -> Result<()> {
        let schemas = node_prop_schemas(&self.schema);
        if schemas.is_empty() {
            return Ok(());
        }
        let owner = node_instance_owner(node_id, self.id);
        validate_props(&owner, props, &schemas, &self.props_validator)
    }

    fn ensure_props_validator(&self, owner: &str) -> Result<Option<&jsonschema::Validator>> {
        let schemas = node_prop_schemas(&self.schema);
        if schemas.is_empty() {
            return Ok(None);
        }
        cached_props_validator(&self.props_validator, owner, &schemas).map(Some)
    }
}

pub trait DataSourceDefinition: Send + Sync {
    fn id(&self) -> &'static str;
    fn outputs(&self) -> Vec<&'static str> {
        Vec::new()
    }
    fn call_mode(&self) -> DataSourceCallMode {
        DataSourceCallMode::Hook
    }
    fn validate_templates(&self, _backend: &dyn JsCodeBackend) -> Result<()> {
        Ok(())
    }
    fn render(
        &self,
        data_source: &DataSourceNode,
        ctx: &mut RenderContext,
    ) -> Result<Vec<RenderedStat>>;
}

pub struct DataSourceSource {
    pub id: &'static str,
    pub schema: DataSourceSourceSchema,
    pub generate_code: DataSourceSourceGenerateCode,
    config_validator: OnceLock<jsonschema::Validator>,
}

impl Clone for DataSourceSource {
    fn clone(&self) -> Self {
        let config_validator = OnceLock::new();
        if let Some(validator) = self.config_validator.get() {
            let _ = config_validator.set(validator.clone());
        }
        Self {
            id: self.id,
            schema: self.schema.clone(),
            generate_code: self.generate_code.clone(),
            config_validator,
        }
    }
}

#[derive(Clone, Default)]
pub struct DataSourceSourceSchema {
    pub template_inputs: IndexMap<&'static str, TemplateInput>,
    pub outputs: IndexMap<&'static str, TemplateOutput>,
}

#[derive(Clone, Default)]
pub struct DataSourceSourceGenerateCode {
    pub imports: Vec<&'static str>,
    pub stats: Vec<StatSource>,
    pub meta: Option<NodeSourceMeta>,
    pub call_mode: DataSourceCallMode,
    pub defaults: IndexMap<&'static str, TemplateDefault>,
}

#[derive(Clone)]
pub enum TemplateDefault {
    Expr(&'static str),
    DataSourceIdJson,
}

impl DataSourceSource {
    pub fn new(id: &'static str, generate_code: DataSourceSourceGenerateCode) -> Self {
        Self {
            id,
            schema: DataSourceSourceSchema::default(),
            generate_code,
            config_validator: OnceLock::new(),
        }
    }

    pub fn with_schema(mut self, schema: DataSourceSourceSchema) -> Self {
        self.schema = schema;
        self
    }

    fn input_paths(&self, key: &str) -> &[&'static str] {
        self.generate_code
            .meta
            .as_ref()
            .and_then(|meta| meta.input_paths.get(key))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn render_template(
        &self,
        template: &str,
        stat: &StatSource,
        data_source: &DataSourceNode,
        ctx: &RenderContext,
    ) -> Result<String> {
        let mut out = template.to_owned();
        let identifier_inputs = stat
            .output
            .iter()
            .copied()
            .collect::<indexmap::IndexSet<_>>();
        for input in self.input_paths(stat.id) {
            let replacement = if identifier_inputs.contains(input) || is_identifier_input(input) {
                self.identifier_input(input, data_source, ctx)?
            } else if *input == "AUTO_LOAD" && !data_source.config.contains_key(*input) {
                match data_source.auto_load {
                    Some(auto_load) => value_to_expr_code_with_context(
                        Some(&serde_json::Value::Bool(auto_load)),
                        "true",
                        &ctx.bindings,
                    )?,
                    None => {
                        let default = self.default_input(input, data_source)?;
                        value_to_expr_code_with_context(
                            None,
                            default.as_deref().unwrap_or("undefined"),
                            &ctx.bindings,
                        )?
                    }
                }
            } else {
                let default = self.default_input(input, data_source)?;
                value_to_expr_code_with_context(
                    data_source.config.get(*input),
                    default.as_deref().unwrap_or("undefined"),
                    &ctx.bindings,
                )?
            };
            out = out.replace(&format!("%%{input}%%"), &replacement);
        }
        Ok(out)
    }

    fn validate_definition(&self, backend: &dyn JsCodeBackend) -> Result<()> {
        let owner = data_source_source_owner(self.id);
        self.ensure_config_validator(&owner)?;
        validate_meta_targets(
            &owner,
            self.generate_code
                .meta
                .as_ref()
                .map(|meta| &meta.input_paths),
            false,
            self.generate_code.stats.iter().map(|stat| stat.id),
        )?;
        order_stat_sources(self.id, &self.generate_code.stats)?;

        for (index, import) in self.generate_code.imports.iter().enumerate() {
            backend
                .validate_module_items(import)
                .map_err(|source| Error::TemplateValidation {
                    owner: owner.clone(),
                    part: format!("import[{index}]"),
                    source: Box::new(source),
                })?;
        }

        for stat in &self.generate_code.stats {
            let identifier_inputs = stat
                .output
                .iter()
                .copied()
                .collect::<indexmap::IndexSet<_>>();
            let part = format!("stat {}", stat.id);
            let code = render_validation_template(
                &owner,
                &part,
                stat.code,
                self.input_paths(stat.id),
                |input| data_source_validation_replacement(input, &identifier_inputs),
            )?;
            backend
                .validate_module_items(&code)
                .map_err(|source| Error::TemplateValidation {
                    owner: owner.clone(),
                    part,
                    source: Box::new(source),
                })?;
        }

        Ok(())
    }

    fn default_input(&self, input: &str, data_source: &DataSourceNode) -> Result<Option<String>> {
        self.generate_code
            .defaults
            .get(input)
            .map(|default| match default {
                TemplateDefault::Expr(code) => Ok((*code).to_owned()),
                TemplateDefault::DataSourceIdJson => serde_json::to_string(&data_source.id)
                    .map_err(|source| Error::JsonValue { source }),
            })
            .transpose()
    }

    fn identifier_input(
        &self,
        input: &str,
        data_source: &DataSourceNode,
        ctx: &RenderContext,
    ) -> Result<String> {
        if let Some(value) = data_source
            .config
            .get(input)
            .and_then(|value| value.as_str())
        {
            return sanitize_ident(value);
        }
        if input == "HOOK_NAME" {
            if let Some(info) = ctx.bindings.data_sources.get(&data_source.id) {
                return sanitize_ident(&info.hook_name);
            }
        }
        if input == "FETCHER_NAME" {
            if let Some(info) = ctx.bindings.data_sources.get(&data_source.id) {
                return sanitize_ident(&info.fetcher_name);
            }
        }
        sanitize_ident(input)
    }
}

fn is_identifier_input(input: &str) -> bool {
    matches!(input, "HOOK_NAME" | "FETCHER_NAME")
}

impl DataSourceDefinition for DataSourceSource {
    fn id(&self) -> &'static str {
        self.id
    }

    fn outputs(&self) -> Vec<&'static str> {
        self.schema.outputs.keys().copied().collect()
    }

    fn call_mode(&self) -> DataSourceCallMode {
        self.generate_code.call_mode
    }

    fn validate_templates(&self, backend: &dyn JsCodeBackend) -> Result<()> {
        self.validate_definition(backend)
    }

    fn render(
        &self,
        data_source: &DataSourceNode,
        ctx: &mut RenderContext,
    ) -> Result<Vec<RenderedStat>> {
        self.validate_config(&data_source.id, &data_source.config)?;
        for import in &self.generate_code.imports {
            ctx.imports.add_source(import);
        }

        let mut rendered_stats = Vec::new();
        for stat in order_stat_sources(self.id, &self.generate_code.stats)? {
            let code = self.render_template(stat.code, stat, data_source, ctx)?;
            let mut outputs = Vec::new();
            for output in &stat.output {
                let actual = self.stat_output_name(stat, output, data_source, ctx)?;
                outputs.push(actual.clone());
            }
            rendered_stats.push(RenderedStat {
                code,
                outputs,
                scope: stat.scope,
            });
        }
        Ok(rendered_stats)
    }
}

impl DataSourceSource {
    fn stat_output_name(
        &self,
        stat: &StatSource,
        output: &str,
        data_source: &DataSourceNode,
        ctx: &RenderContext,
    ) -> Result<String> {
        if self.input_paths(stat.id).contains(&output) {
            self.identifier_input(output, data_source, ctx)
        } else {
            sanitize_ident(output)
        }
    }

    fn validate_config(
        &self,
        data_source_id: &str,
        config: &std::collections::BTreeMap<String, Value>,
    ) -> Result<()> {
        if self.schema.template_inputs.is_empty() {
            return Ok(());
        }
        let schemas = self
            .schema
            .template_inputs
            .iter()
            .map(|(key, schema)| (*key, schema))
            .collect::<IndexMap<_, _>>();
        let owner = data_source_instance_owner(data_source_id, self.id);
        validate_props(&owner, config, &schemas, &self.config_validator)
    }

    fn ensure_config_validator(&self, owner: &str) -> Result<Option<&jsonschema::Validator>> {
        if self.schema.template_inputs.is_empty() {
            return Ok(None);
        }
        let schemas = self
            .schema
            .template_inputs
            .iter()
            .map(|(key, schema)| (*key, schema))
            .collect::<IndexMap<_, _>>();
        cached_props_validator(&self.config_validator, owner, &schemas).map(Some)
    }
}

fn validate_props(
    owner: &str,
    props: &std::collections::BTreeMap<String, Value>,
    schemas: &IndexMap<&str, &TemplateInput>,
    validator: &OnceLock<jsonschema::Validator>,
) -> Result<()> {
    let instance = serde_json::to_value(props).map_err(|source| Error::JsonValue { source })?;
    let validator = cached_props_validator(validator, owner, schemas)?;
    validator.validate(&instance).map_err(|err| {
        let legacy = legacy_prop_error(owner, props, schemas);
        legacy.unwrap_or_else(|| Error::JsonSchemaValidation {
            owner: owner.to_owned(),
            message: err.to_string(),
        })
    })
}

fn cached_props_validator<'a>(
    cache: &'a OnceLock<jsonschema::Validator>,
    owner: &str,
    schemas: &IndexMap<&str, &TemplateInput>,
) -> Result<&'a jsonschema::Validator> {
    if let Some(validator) = cache.get() {
        return Ok(validator);
    }
    let validator = compile_props_validator(owner, schemas)?;
    let _ = cache.set(validator);
    Ok(cache
        .get()
        .expect("props validator cache was just initialized"))
}

fn compile_props_validator(
    owner: &str,
    schemas: &IndexMap<&str, &TemplateInput>,
) -> Result<jsonschema::Validator> {
    let schema = props_schema(schemas);
    jsonschema::validator_for(&schema).map_err(|err| Error::JsonSchemaCompile {
        owner: owner.to_owned(),
        message: err.to_string(),
    })
}

fn node_prop_schemas(schema: &NodeSourceSchema) -> IndexMap<&str, &TemplateInput> {
    let mut schemas = IndexMap::<&str, &TemplateInput>::new();
    schemas.extend(
        schema
            .template_inputs
            .iter()
            .map(|(key, schema)| (*key, schema)),
    );
    schemas.extend(
        schema
            .runtime_props
            .iter()
            .map(|(key, schema)| (*key, schema)),
    );
    schemas
}

fn props_schema(schemas: &IndexMap<&str, &TemplateInput>) -> Value {
    let properties = schemas
        .iter()
        .map(|(key, schema)| ((*key).to_owned(), prop_schema(schema)))
        .collect::<serde_json::Map<_, _>>();
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "additionalProperties": false,
    })
}

fn prop_schema(schema: &TemplateInput) -> Value {
    serde_json::json!({
        "anyOf": [
            binding_value_schema(),
            expression_value_schema(),
            data_schema(schema),
            { "type": "null" }
        ]
    })
}

fn data_schema(schema: &TemplateInput) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("type".to_owned(), Value::String(schema.ty.to_owned()));
    if !schema.description.is_empty() {
        out.insert(
            "description".to_owned(),
            Value::String(schema.description.to_owned()),
        );
    }
    Value::Object(out)
}

fn binding_value_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "type": { "const": "binding" }
        },
        "required": ["type"],
    })
}

fn expression_value_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "type": { "const": "expression" },
            "code": { "type": "string" }
        },
        "required": ["type", "code"],
    })
}

fn legacy_prop_error(
    owner: &str,
    props: &std::collections::BTreeMap<String, Value>,
    schemas: &IndexMap<&str, &TemplateInput>,
) -> Option<Error> {
    for (key, value) in props {
        let Some(schema) = schemas.get(key.as_str()) else {
            return Some(Error::UnknownProp {
                owner: owner.to_owned(),
                prop: key.clone(),
            });
        };
        if !is_dynamic_value(value) && !value_matches_type(value, schema.ty) {
            return Some(Error::InvalidPropType {
                owner: owner.to_owned(),
                prop: key.clone(),
                expected: schema.ty.to_owned(),
            });
        }
    }
    None
}

fn validate_meta_targets<'a>(
    owner: &str,
    meta_input_paths: Option<&IndexMap<&'static str, Vec<&'static str>>>,
    has_jsx: bool,
    stat_ids: impl Iterator<Item = &'a str>,
) -> Result<()> {
    let Some(input_paths) = meta_input_paths else {
        return Ok(());
    };
    let stat_ids = stat_ids.collect::<indexmap::IndexSet<_>>();
    for target in input_paths.keys() {
        if *target == "$jsx" {
            if has_jsx {
                continue;
            }
        } else if stat_ids.contains(target) {
            continue;
        }
        return Err(Error::TemplateMetaTargetNotFound {
            owner: owner.to_owned(),
            target: (*target).to_owned(),
        });
    }
    Ok(())
}

fn render_validation_template(
    owner: &str,
    part: &str,
    template: &str,
    input_paths: &[&str],
    replacement: impl Fn(&str) -> &'static str,
) -> Result<String> {
    let declared = input_paths
        .iter()
        .copied()
        .collect::<indexmap::IndexSet<_>>();
    for placeholder in template_placeholders(template) {
        if !declared.contains(placeholder.as_str()) {
            return Err(Error::TemplatePlaceholderNotDeclared {
                owner: owner.to_owned(),
                part: part.to_owned(),
                placeholder,
            });
        }
    }

    let mut out = template.to_owned();
    out = out.replace("<__ENGINE_CHILDREN__ />", "<React.Fragment />");
    out = out.replace("<__ENGINE_CHILDREN__/>", "<React.Fragment />");
    out = out.replace("__ENGINE_CHILDREN__", "<React.Fragment />");
    for input in input_paths {
        out = out.replace(&format!("%%{input}%%"), replacement(input));
    }
    Ok(out)
}

fn template_placeholders(template: &str) -> indexmap::IndexSet<String> {
    let mut out = indexmap::IndexSet::new();
    let mut rest = template;
    while let Some(start) = rest.find("%%") {
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("%%") else {
            break;
        };
        let name = after_start[..end].trim();
        if !name.is_empty() {
            out.insert(name.to_owned());
        }
        rest = &after_start[end + 2..];
    }
    out
}

fn node_validation_replacement(_input: &str) -> &'static str {
    "undefined"
}

fn data_source_validation_replacement(
    input: &str,
    identifier_inputs: &indexmap::IndexSet<&str>,
) -> &'static str {
    if identifier_inputs.contains(input) || is_identifier_input(input) {
        "__forgeTemplateIdent"
    } else {
        "undefined"
    }
}

fn node_source_owner(id: &str) -> String {
    format!("node source {id}")
}

fn data_source_source_owner(id: &str) -> String {
    format!("data source source {id}")
}

fn node_instance_owner(node_id: &str, source_id: &str) -> String {
    format!("node {node_id} ({source_id})")
}

fn data_source_instance_owner(data_source_id: &str, source_id: &str) -> String {
    format!("data source {data_source_id} ({source_id})")
}

fn is_dynamic_value(value: &Value) -> bool {
    value
        .as_object()
        .and_then(|obj| obj.get("type"))
        .and_then(Value::as_str)
        .is_some_and(|ty| ty == "binding" || ty == "expression")
}

fn value_matches_type(value: &Value, expected: &str) -> bool {
    if value.is_null() {
        return true;
    }
    match expected {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        _ => true,
    }
}

fn order_stat_sources<'a>(owner: &str, stats: &'a [StatSource]) -> Result<Vec<&'a StatSource>> {
    let by_id = stats
        .iter()
        .enumerate()
        .map(|(index, stat)| (stat.id, index))
        .collect::<IndexMap<_, _>>();
    let mut ordered = Vec::new();
    let mut visiting = indexmap::IndexSet::<&str>::new();
    let mut visited = indexmap::IndexSet::<&str>::new();

    for stat in stats {
        visit_stat_source(
            owner,
            stat.id,
            stats,
            &by_id,
            &mut visiting,
            &mut visited,
            &mut ordered,
        )?;
    }

    Ok(ordered)
}

fn visit_stat_source<'a>(
    owner: &str,
    stat_id: &str,
    stats: &'a [StatSource],
    by_id: &IndexMap<&str, usize>,
    visiting: &mut indexmap::IndexSet<&'a str>,
    visited: &mut indexmap::IndexSet<&'a str>,
    ordered: &mut Vec<&'a StatSource>,
) -> Result<()> {
    if visited.contains(stat_id) {
        return Ok(());
    }
    if visiting.contains(stat_id) {
        return Err(Error::StatDependencyCycle {
            owner: owner.to_owned(),
            stat: stat_id.to_owned(),
        });
    }
    let index = by_id
        .get(stat_id)
        .copied()
        .ok_or_else(|| Error::StatDependencyNotFound {
            owner: owner.to_owned(),
            stat: stat_id.to_owned(),
            dependency: stat_id.to_owned(),
        })?;
    let stat = &stats[index];
    visiting.insert(stat.id);
    for dependency in &stat.depends {
        if !by_id.contains_key(dependency) {
            return Err(Error::StatDependencyNotFound {
                owner: owner.to_owned(),
                stat: stat.id.to_owned(),
                dependency: (*dependency).to_owned(),
            });
        }
        visit_stat_source(owner, dependency, stats, by_id, visiting, visited, ordered)?;
    }
    visiting.shift_remove(stat.id);
    visited.insert(stat.id);
    ordered.push(stat);
    Ok(())
}

#[derive(Default)]
pub struct Registry {
    nodes: IndexMap<&'static str, Box<dyn NodeDefinition>>,
    data_sources: IndexMap<&'static str, Box<dyn DataSourceDefinition>>,
}

impl Registry {
    pub fn register_node(&mut self, definition: impl NodeDefinition + 'static) {
        self.nodes.insert(definition.id(), Box::new(definition));
    }

    pub fn register_data_source(&mut self, definition: impl DataSourceDefinition + 'static) {
        self.data_sources
            .insert(definition.id(), Box::new(definition));
    }

    pub fn node(&self, id: &str) -> Result<&dyn NodeDefinition> {
        self.nodes
            .get(id)
            .map(|item| item.as_ref())
            .ok_or_else(|| Error::NodeNotFound { id: id.to_owned() })
    }

    pub fn data_source(&self, id: &str) -> Result<&dyn DataSourceDefinition> {
        self.data_sources
            .get(id)
            .map(|item| item.as_ref())
            .ok_or_else(|| Error::DataSourceNotFound { id: id.to_owned() })
    }

    pub fn validate_templates(&self, backend: &dyn JsCodeBackend) -> Result<()> {
        for definition in self.nodes.values() {
            definition.validate_templates(backend)?;
        }
        for definition in self.data_sources.values() {
            definition.validate_templates(backend)?;
        }
        Ok(())
    }
}
