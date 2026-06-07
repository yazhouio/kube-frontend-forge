use std::collections::BTreeMap;

use indexmap::{IndexMap, IndexSet};

use crate::action_graph::ActionGraphPlan;
use crate::builtins::default_registry;
use crate::code_backend::{JsCodeBackend, SwcCodeBackend};
use crate::error::{Error, Result};
use crate::model::{ComponentNode, DataSourceNode, PageConfig};
use crate::names::{NameAllocator, sanitize_ident};
use crate::registry::{Registry, RenderContext, RenderedStat};
use crate::value::{
    BindingContext, BindingUse, DataSourceBindingInfo, DataSourceCallMode, collect_binding_uses,
    expr_to_code_with_context, pascal_case, resolve_binding_output_var_name,
    value_to_expr_code_with_context,
};

pub struct ComponentGenerator<B = SwcCodeBackend> {
    registry: Registry,
    backend: B,
}

impl Default for ComponentGenerator<SwcCodeBackend> {
    fn default() -> Self {
        Self {
            registry: default_registry(),
            backend: SwcCodeBackend,
        }
    }
}

impl ComponentGenerator<SwcCodeBackend> {
    pub fn new(registry: Registry) -> Self {
        Self {
            registry,
            backend: SwcCodeBackend,
        }
    }
}

impl<B: JsCodeBackend> ComponentGenerator<B> {
    pub fn with_backend(registry: Registry, backend: B) -> Self {
        Self { registry, backend }
    }

    pub fn generate_page_code(&self, page: &PageConfig) -> Result<String> {
        let action_graphs = ActionGraphPlan::new(&page.action_graphs)?;
        let mut bindings = self.build_binding_context(page)?;
        action_graphs.add_to_binding_context(&mut bindings);
        let mut ctx = RenderContext {
            bindings,
            ..RenderContext::default()
        };
        let mut component_names = NameAllocator::default();

        let data_sources_by_id = page
            .data_sources
            .iter()
            .map(|data_source| (data_source.id.as_str(), data_source))
            .collect::<BTreeMap<_, _>>();

        let root_preferred_name = page
            .root
            .meta
            .as_ref()
            .and_then(|meta| meta.title.as_deref())
            .or(page.meta.title.as_deref())
            .unwrap_or("Page");
        let root_name = self.render_boundary_node(
            &page.root,
            Some(root_preferred_name),
            &mut ctx,
            &mut component_names,
            &data_sources_by_id,
            &action_graphs,
        )?;

        self.backend.emit_module(
            &ctx.imports.emit_sources(),
            &ctx.module_items,
            Some(&root_name),
        )
    }

    fn render_boundary_node(
        &self,
        node: &ComponentNode,
        preferred_name: Option<&str>,
        ctx: &mut RenderContext,
        component_names: &mut NameAllocator,
        data_sources_by_id: &BTreeMap<&str, &DataSourceNode>,
        action_graphs: &ActionGraphPlan,
    ) -> Result<String> {
        let fragment = self.render_fragment(
            node,
            ctx,
            component_names,
            data_sources_by_id,
            action_graphs,
        )?;
        let mut binding_uses = fragment.binding_uses.clone();
        binding_uses.extend(action_graphs.binding_uses_for_graphs(&fragment.action_graph_ids));
        let data_source_stats =
            self.render_used_data_sources(ctx, &binding_uses, data_sources_by_id)?;
        let component_name = component_names.allocate_component_name(
            preferred_name,
            node.meta.as_ref().and_then(|meta| meta.title.as_deref()),
            &node.id,
            &node.ty,
        )?;
        let mut stats = self.boundary_prelude_stats(
            ctx,
            &binding_uses,
            action_graphs.graphs_use_runtime(&fragment.action_graph_ids),
        );
        stats.extend(action_graphs.render_boundary_stats(
            &fragment.action_graph_ids,
            ctx,
            data_sources_by_id,
        )?);
        let mut local_render_stats = data_source_stats;
        local_render_stats.extend(fragment.stats);
        let (local_stats, jsx_replacements) =
            self.render_local_stats(&local_render_stats, ctx, &binding_uses)?;
        let jsx = self
            .backend
            .rename_expr_idents(&fragment.jsx, &jsx_replacements)?;
        stats.extend(local_stats);
        let function_code = format!(
            "function {component_name}(props) {{\n{}\nreturn {};\n}}",
            stats.join("\n"),
            jsx
        );
        ctx.module_items.push(function_code);
        Ok(component_name)
    }

    fn render_fragment(
        &self,
        node: &ComponentNode,
        ctx: &mut RenderContext,
        component_names: &mut NameAllocator,
        data_sources_by_id: &BTreeMap<&str, &DataSourceNode>,
        action_graphs: &ActionGraphPlan,
    ) -> Result<RenderedFragment> {
        let mut stats = Vec::<RenderedStat>::new();
        let mut binding_uses = Vec::<BindingUse>::new();
        let mut action_graph_ids = IndexSet::<String>::new();
        let mut children = Vec::<ChildRender>::new();

        for child in &node.children {
            if child.meta.as_ref().is_some_and(|meta| meta.scope) {
                let child_name = self.render_boundary_node(
                    child,
                    None,
                    ctx,
                    component_names,
                    data_sources_by_id,
                    action_graphs,
                )?;
                let (jsx, attr_binding_uses) = self.scoped_child_jsx(child, &child_name, ctx)?;
                children.push(ChildRender::Scoped {
                    jsx,
                    binding_uses: attr_binding_uses,
                });
            } else {
                let child_fragment = self.render_fragment(
                    child,
                    ctx,
                    component_names,
                    data_sources_by_id,
                    action_graphs,
                )?;
                children.push(ChildRender::Inline(child_fragment));
            }
        }

        let mut render_node = node.clone();
        for handler in action_graphs.handlers_for_node(&node.id) {
            render_node
                .props
                .entry(handler.prop_name.clone())
                .or_insert_with(|| handler.value.clone());
            action_graph_ids.insert(handler.graph_id.clone());
        }

        let probe_child_jsx = children.iter().map(ChildRender::jsx).collect::<Vec<_>>();
        let rendered_probe =
            self.registry
                .node(&node.ty)?
                .render(&render_node, probe_child_jsx, ctx)?;
        let mut used_names = IndexSet::<String>::new();
        add_stat_outputs(&rendered_probe.stats, &mut used_names);

        let mut child_jsx = Vec::<String>::new();
        for child in children {
            match child {
                ChildRender::Scoped {
                    jsx,
                    binding_uses: child_binding_uses,
                } => {
                    binding_uses.extend(child_binding_uses);
                    child_jsx.push(jsx);
                }
                ChildRender::Inline(mut child_fragment) => {
                    rename_inline_fragment_outputs(
                        &mut child_fragment,
                        &mut used_names,
                        &self.backend,
                    )?;
                    stats.extend(child_fragment.stats);
                    binding_uses.extend(child_fragment.binding_uses);
                    action_graph_ids.extend(child_fragment.action_graph_ids);
                    child_jsx.push(child_fragment.jsx);
                }
            }
        }

        let rendered = self
            .registry
            .node(&node.ty)?
            .render(&render_node, child_jsx, ctx)?;
        let mut own_stats = rendered.stats;
        own_stats.extend(stats);
        binding_uses.extend(rendered.binding_uses);
        for binding_use in &binding_uses {
            if let BindingUse::ActionGraph { source } = binding_use {
                action_graph_ids.insert(source.clone());
            }
        }
        Ok(RenderedFragment {
            jsx: rendered.jsx,
            stats: own_stats,
            binding_uses,
            action_graph_ids,
        })
    }

    fn render_used_data_sources(
        &self,
        ctx: &mut RenderContext,
        binding_uses: &[BindingUse],
        data_sources_by_id: &BTreeMap<&str, &DataSourceNode>,
    ) -> Result<Vec<RenderedStat>> {
        let source_ids = self
            .collect_required_outputs(&ctx.bindings, binding_uses)
            .keys()
            .cloned()
            .collect::<Vec<_>>();

        let mut stats = Vec::new();
        for source_id in source_ids {
            if ctx.rendered_data_sources.contains(&source_id) {
                continue;
            }
            let data_source = data_sources_by_id.get(source_id.as_str()).ok_or_else(|| {
                Error::BindingSourceNotFound {
                    id: source_id.clone(),
                }
            })?;
            stats.extend(
                self.registry
                    .data_source(&data_source.ty)?
                    .render(data_source, ctx)
                    .map_err(|source| Error::RenderDataSource {
                        id: data_source.id.clone(),
                        source: Box::new(source),
                    })?,
            );
            ctx.rendered_data_sources.insert(source_id);
        }

        Ok(stats)
    }

    fn render_local_stats(
        &self,
        stats: &[RenderedStat],
        ctx: &mut RenderContext,
        binding_uses: &[BindingUse],
    ) -> Result<(Vec<String>, BTreeMap<String, String>)> {
        let mut names = NameAllocator::default();
        names.reserve("props");
        names.reserve("__runtime__");
        for name in self.data_source_binding_var_names(ctx, binding_uses) {
            names.reserve(name);
        }

        let mut local_stats = Vec::new();
        let mut module_replacements = BTreeMap::<String, String>::new();
        let mut local_replacements = BTreeMap::<String, String>::new();

        for stat in stats {
            let mut replacements = module_replacements.clone();
            replacements.extend(local_replacements.clone());

            if stat.scope.is_module_scope() {
                for output in &stat.outputs {
                    let allocated = ctx.module_names.allocate(output, output)?;
                    names.reserve(allocated.clone());
                    if allocated != *output {
                        replacements.insert(output.clone(), allocated.clone());
                        module_replacements.insert(output.clone(), allocated);
                    }
                }
                let code = self
                    .backend
                    .rename_module_item_idents(&stat.code, &replacements)?;
                if stat.scope == crate::registry::StatementScope::ModuleImport {
                    let split = self.backend.split_imports(&code)?;
                    for import in split.imports {
                        ctx.imports.add_source(&import);
                    }
                    ctx.module_items.extend(split.rest);
                } else {
                    ctx.module_items.push(code);
                }
                continue;
            }

            for output in &stat.outputs {
                let allocated = names.allocate(output, output)?;
                if allocated != *output {
                    replacements.insert(output.clone(), allocated.clone());
                    local_replacements.insert(output.clone(), allocated);
                }
            }
            local_stats.push(
                self.backend
                    .rename_module_item_idents(&stat.code, &replacements)?,
            );
        }

        let mut jsx_replacements = module_replacements;
        jsx_replacements.extend(local_replacements);
        Ok((local_stats, jsx_replacements))
    }

    fn build_binding_context(&self, page: &PageConfig) -> Result<BindingContext> {
        let mut ctx = BindingContext::default();
        for data_source in &page.data_sources {
            let definition = self.registry.data_source(&data_source.ty)?;
            let base_name = crate::value::camel_case(&data_source.id);
            let hook_name = data_source
                .config
                .get("HOOK_NAME")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("use{}", pascal_case(&data_source.id, "DataSource")));
            let fetcher_name = data_source
                .config
                .get("FETCHER_NAME")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("fetch{}", pascal_case(&data_source.id, "DataSource")));
            ctx.data_sources.insert(
                data_source.id.clone(),
                DataSourceBindingInfo {
                    id: data_source.id.clone(),
                    hook_name,
                    fetcher_name,
                    base_name,
                    output_names: definition
                        .outputs()
                        .iter()
                        .map(|output| (*output).to_owned())
                        .collect(),
                    call_mode: definition.call_mode(),
                    args: Vec::new(),
                    arg_binding_uses: Vec::new(),
                },
            );
        }
        for data_source in &page.data_sources {
            let args = data_source
                .args
                .iter()
                .map(|arg| expr_to_code_with_context(arg, &ctx))
                .collect::<Result<Vec<_>>>()?;
            let arg_binding_uses = data_source
                .args
                .iter()
                .flat_map(|arg| collect_binding_uses(arg, &ctx))
                .collect::<Vec<_>>();
            if let Some(info) = ctx.data_sources.get_mut(&data_source.id) {
                info.args = args;
                info.arg_binding_uses = arg_binding_uses;
            }
        }
        Ok(ctx)
    }

    fn data_source_binding_stats(
        &self,
        ctx: &RenderContext,
        binding_uses: &[BindingUse],
    ) -> Vec<String> {
        let by_source = self.collect_required_outputs(&ctx.bindings, binding_uses);

        by_source
            .into_iter()
            .filter_map(|(source, outputs)| {
                let info = ctx.bindings.data_sources.get(&source)?;
                if info.call_mode != DataSourceCallMode::Hook {
                    return None;
                }
                let destructured = outputs
                    .iter()
                    .map(|output| {
                        format!(
                            "{output}: {}",
                            resolve_binding_output_var_name(info, output)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let args = info.args.join(", ");
                Some(format!(
                    "const {{ {destructured} }} = {}({args});",
                    info.hook_name
                ))
            })
            .collect()
    }

    fn data_source_binding_var_names(
        &self,
        ctx: &RenderContext,
        binding_uses: &[BindingUse],
    ) -> Vec<String> {
        self.collect_required_outputs(&ctx.bindings, binding_uses)
            .into_iter()
            .filter_map(|(source, outputs)| {
                let info = ctx.bindings.data_sources.get(&source)?;
                if info.call_mode != DataSourceCallMode::Hook {
                    return None;
                }
                Some(
                    outputs
                        .iter()
                        .map(|output| resolve_binding_output_var_name(info, output))
                        .collect::<Vec<_>>(),
                )
            })
            .flatten()
            .collect()
    }

    fn boundary_prelude_stats(
        &self,
        ctx: &mut RenderContext,
        binding_uses: &[BindingUse],
        force_runtime: bool,
    ) -> Vec<String> {
        let mut stats = Vec::new();
        if force_runtime || self.binding_uses_runtime(&ctx.bindings, binding_uses) {
            self.ensure_runtime_import(ctx);
            stats.push("const __runtime__ = useRuntimeContext();".to_owned());
        }
        stats.extend(self.data_source_binding_stats(ctx, binding_uses));
        stats
    }

    fn ensure_runtime_import(&self, ctx: &mut RenderContext) {
        if !ctx
            .imports
            .has_named("@frontend-forge/forge-components", "useRuntimeContext")
        {
            ctx.imports
                .add_named("@frontend-forge/forge-components", "useRuntimeContext");
        }
    }

    fn scoped_child_jsx(
        &self,
        child: &ComponentNode,
        child_name: &str,
        ctx: &RenderContext,
    ) -> Result<(String, Vec<BindingUse>)> {
        let mut attrs = Vec::new();
        let mut binding_uses = Vec::new();
        for name in self.registry.node(&child.ty)?.runtime_prop_names() {
            if !is_js_ident(name) {
                continue;
            }
            if let Some(value) = child.props.get(name) {
                let code =
                    value_to_expr_code_with_context(Some(value), "undefined", &ctx.bindings)?;
                attrs.push(format!("{name}={{{code}}}"));
                binding_uses.extend(collect_binding_uses(value, &ctx.bindings));
            }
        }
        if attrs.is_empty() {
            Ok((format!("<{child_name} />"), binding_uses))
        } else {
            Ok((
                format!("<{child_name} {} />", attrs.join(" ")),
                binding_uses,
            ))
        }
    }

    fn collect_required_outputs(
        &self,
        bindings: &BindingContext,
        binding_uses: &[BindingUse],
    ) -> IndexMap<String, IndexSet<String>> {
        let mut by_source = IndexMap::<String, IndexSet<String>>::new();
        let mut visiting = IndexSet::<String>::new();
        self.collect_required_outputs_inner(bindings, binding_uses, &mut by_source, &mut visiting);
        by_source
    }

    fn binding_uses_runtime(&self, bindings: &BindingContext, binding_uses: &[BindingUse]) -> bool {
        let mut visiting = IndexSet::<String>::new();
        self.binding_uses_runtime_inner(bindings, binding_uses, &mut visiting)
    }

    fn binding_uses_runtime_inner(
        &self,
        bindings: &BindingContext,
        binding_uses: &[BindingUse],
        visiting: &mut IndexSet<String>,
    ) -> bool {
        for binding_use in binding_uses {
            match binding_use {
                BindingUse::Runtime => return true,
                BindingUse::DataSource { source, .. } => {
                    if visiting.contains(source) {
                        continue;
                    }
                    if let Some(info) = bindings.data_sources.get(source) {
                        visiting.insert(source.clone());
                        let has_runtime = self.binding_uses_runtime_inner(
                            bindings,
                            &info.arg_binding_uses,
                            visiting,
                        );
                        visiting.shift_remove(source);
                        if has_runtime {
                            return true;
                        }
                    }
                }
                BindingUse::ActionGraph { .. } => {}
            }
        }
        false
    }

    fn collect_required_outputs_inner(
        &self,
        bindings: &BindingContext,
        binding_uses: &[BindingUse],
        by_source: &mut IndexMap<String, IndexSet<String>>,
        visiting: &mut IndexSet<String>,
    ) {
        for binding_use in binding_uses {
            if let BindingUse::DataSource { source, output } = binding_use {
                if visiting.contains(source) {
                    continue;
                }
                if let Some(info) = bindings.data_sources.get(source) {
                    visiting.insert(source.clone());
                    self.collect_required_outputs_inner(
                        bindings,
                        &info.arg_binding_uses,
                        by_source,
                        visiting,
                    );
                    visiting.shift_remove(source);
                }
                by_source
                    .entry(source.clone())
                    .or_default()
                    .insert(output.clone());
            }
        }
    }
}

struct RenderedFragment {
    jsx: String,
    stats: Vec<RenderedStat>,
    binding_uses: Vec<BindingUse>,
    action_graph_ids: IndexSet<String>,
}

enum ChildRender {
    Scoped {
        jsx: String,
        binding_uses: Vec<BindingUse>,
    },
    Inline(RenderedFragment),
}

impl ChildRender {
    fn jsx(&self) -> String {
        match self {
            Self::Scoped { jsx, .. } => jsx.clone(),
            Self::Inline(fragment) => fragment.jsx.clone(),
        }
    }
}

fn add_stat_outputs(stats: &[RenderedStat], used_names: &mut IndexSet<String>) {
    for stat in stats {
        for output in &stat.outputs {
            used_names.insert(output.clone());
        }
    }
}

fn rename_inline_fragment_outputs(
    fragment: &mut RenderedFragment,
    used_names: &mut IndexSet<String>,
    backend: &impl JsCodeBackend,
) -> Result<()> {
    let mut replacements = BTreeMap::<String, String>::new();
    for stat in &fragment.stats {
        if stat.scope.is_module_scope() {
            continue;
        }
        for output in &stat.outputs {
            if replacements.contains_key(output) {
                continue;
            }
            if used_names.contains(output) {
                replacements.insert(output.clone(), allocate_unique_name(output, used_names)?);
            } else {
                used_names.insert(output.clone());
            }
        }
    }

    if replacements.is_empty() {
        return Ok(());
    }

    for stat in &mut fragment.stats {
        stat.code = backend.rename_module_item_idents(&stat.code, &replacements)?;
        for output in &mut stat.outputs {
            if let Some(replacement) = replacements.get(output) {
                *output = replacement.clone();
            }
        }
    }
    fragment.jsx = backend.rename_expr_idents(&fragment.jsx, &replacements)?;
    Ok(())
}

fn allocate_unique_name(preferred: &str, used_names: &mut IndexSet<String>) -> Result<String> {
    let base = sanitize_ident(preferred)?;
    let mut name = base.clone();
    let mut index = 2;
    while used_names.contains(&name) {
        name = format!("{base}{index}");
        index += 1;
    }
    used_names.insert(name.clone());
    Ok(name)
}

fn is_js_ident(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first == '$' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_backend::OxcCodeBackend;
    use crate::model::unwrap_page_schema;
    use crate::registry::{
        DataSourceSource, DataSourceSourceGenerateCode, DataSourceSourceSchema, NodeSource,
        NodeSourceGenerateCode, NodeSourceMeta, Registry, StatSource, StatementScope,
        TemplateOutput,
    };

    #[test]
    fn generates_basic_page() {
        let raw = serde_json::json!({
          "pageSchema": {
            "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
            "root": {
              "id": "root",
              "meta": { "scope": true, "title": "SamplePage" },
              "type": "Layout",
              "props": { "TEXT": "Hello" },
              "children": [
                { "id": "child", "type": "Text", "props": { "TEXT": "World", "DEFAULT_VALUE": 1 } }
              ]
            },
            "context": {}
          }
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::default()
            .generate_page_code(&page)
            .unwrap();
        assert!(code.contains("import { useState } from \"react\";"));
        assert!(code.contains("function SamplePage"));
        assert!(code.contains("const [text, setText] = useState(1);"));
        assert!(code.contains("<div>{\"World\"}</div>"));
        assert!(code.contains("<div>{\"Hello\"}</div>"));
        assert!(code.contains("export default SamplePage"));
        assert!(!code.contains("function TextChild"));
    }

    #[test]
    fn generates_basic_page_with_oxc_backend() {
        let raw = serde_json::json!({
          "pageSchema": {
            "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
            "root": {
              "id": "root",
              "meta": { "scope": true, "title": "SamplePage" },
              "type": "Layout",
              "props": { "TEXT": "Hello" },
              "children": [
                { "id": "child", "type": "Text", "props": { "TEXT": "World", "DEFAULT_VALUE": 1 } }
              ]
            },
            "context": {}
          }
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code =
            ComponentGenerator::with_backend(crate::builtins::default_registry(), OxcCodeBackend)
                .generate_page_code(&page)
                .unwrap();
        assert!(code.contains("import { useState } from \"react\""));
        assert!(code.contains("function SamplePage"));
        assert!(code.contains("const [text, setText] = useState(1)"));
        assert!(code.contains("<div>{\"World\"}</div>"));
        assert!(code.contains("export default SamplePage"));
    }

    #[test]
    fn allocates_unique_local_stat_outputs() {
        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Layout",
            "props": { "TEXT": "Hello" },
            "children": [
              { "id": "child-1", "type": "Text", "props": { "TEXT": "One", "DEFAULT_VALUE": 1 } },
              { "id": "child-2", "type": "Text", "props": { "TEXT": "Two", "DEFAULT_VALUE": 2 } }
            ]
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::default()
            .generate_page_code(&page)
            .unwrap();
        assert!(code.contains("const [text, setText] = useState(1);"));
        assert!(code.contains("const [text2, setText2] = useState(2);"));
    }

    #[test]
    fn renames_inline_child_jsx_when_stat_outputs_collide() {
        let mut registry = crate::builtins::default_registry();
        registry.register_node(NodeSource::new(
            "LabelStat",
            NodeSourceGenerateCode {
                stats: vec![StatSource {
                    id: "label",
                    scope: StatementScope::FunctionBody,
                    code: "const label = %%VALUE%%;",
                    output: vec!["label"],
                    depends: vec![],
                }],
                jsx: Some("<span>{label}</span>"),
                meta: Some(NodeSourceMeta {
                    input_paths: [("label", vec!["VALUE"])].into_iter().collect(),
                    runtime_deps: Vec::new(),
                }),
                ..NodeSourceGenerateCode::default()
            },
        ));

        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Layout",
            "props": { "TEXT": "Root" },
            "children": [
              { "id": "first", "type": "LabelStat", "props": { "VALUE": "One" } },
              { "id": "second", "type": "LabelStat", "props": { "VALUE": "Two" } }
            ]
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap();

        assert!(code.contains(r#"const label = "One";"#));
        assert!(code.contains(r#"const label2 = "Two";"#));
        assert!(code.contains("<span>{label}</span><span>{label2}</span>"));
    }

    #[test]
    fn orders_node_stats_by_dependencies() {
        let mut registry = Registry::default();
        registry.register_node(NodeSource::new(
            "Dependent",
            NodeSourceGenerateCode {
                stats: vec![
                    StatSource {
                        id: "b",
                        scope: StatementScope::FunctionBody,
                        code: "const b = a + 1;",
                        output: vec!["b"],
                        depends: vec!["a"],
                    },
                    StatSource {
                        id: "a",
                        scope: StatementScope::FunctionBody,
                        code: "const a = 1;",
                        output: vec!["a"],
                        depends: vec![],
                    },
                ],
                jsx: Some("<div>{%%TEXT%%}</div>"),
                meta: Some(NodeSourceMeta {
                    input_paths: [("$jsx", vec!["TEXT"])].into_iter().collect(),
                    runtime_deps: Vec::new(),
                }),
                ..NodeSourceGenerateCode::default()
            },
        ));
        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Dependent",
            "props": { "TEXT": "Hello" }
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap();
        assert!(code.find("const a = 1;").unwrap() < code.find("const b = a + 1;").unwrap());
    }

    #[test]
    fn renders_module_scoped_node_stats_outside_component() {
        let mut registry = Registry::default();
        registry.register_node(NodeSource::new(
            "ScopedStats",
            NodeSourceGenerateCode {
                stats: vec![
                    StatSource {
                        id: "helper",
                        scope: StatementScope::ModuleDecl,
                        code: "const helper = () => \"Hello\";",
                        output: vec!["helper"],
                        depends: vec![],
                    },
                    StatSource {
                        id: "value",
                        scope: StatementScope::FunctionBody,
                        code: "const value = helper();",
                        output: vec!["value"],
                        depends: vec!["helper"],
                    },
                ],
                jsx: Some("<div>{value}</div>"),
                ..NodeSourceGenerateCode::default()
            },
        ));
        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "ScopedStats",
            "props": {}
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap();
        let helper_index = code.find("const helper =").unwrap();
        let function_index = code.find("function SamplePage").unwrap();
        let value_index = code.find("const value = helper();").unwrap();

        assert!(helper_index < function_index);
        assert!(function_index < value_index);
    }

    #[test]
    fn extracts_module_import_stats_into_import_registry() {
        let mut registry = Registry::default();
        registry.register_node(NodeSource::new(
            "ImportStats",
            NodeSourceGenerateCode {
                imports: vec![r#"import { useMemo } from "react";"#],
                stats: vec![StatSource {
                    id: "imports",
                    scope: StatementScope::ModuleImport,
                    code: r#"import { useState } from "react"; const moduleValue = 1;"#,
                    output: vec!["moduleValue"],
                    depends: vec![],
                }],
                jsx: Some("<div>{moduleValue}</div>"),
                ..NodeSourceGenerateCode::default()
            },
        ));
        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "ImportStats",
            "props": {}
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap();

        assert!(code.contains(r#"import { useMemo, useState } from "react";"#));
        assert_eq!(code.matches(r#"from "react""#).count(), 1);
        assert!(code.find("import {").unwrap() < code.find("const moduleValue = 1;").unwrap());
        assert!(
            code.find("const moduleValue = 1;").unwrap()
                < code.find("function SamplePage").unwrap()
        );
    }

    #[test]
    fn reports_missing_stat_dependencies() {
        let mut registry = Registry::default();
        registry.register_node(NodeSource::new(
            "Broken",
            NodeSourceGenerateCode {
                stats: vec![StatSource {
                    id: "b",
                    scope: StatementScope::FunctionBody,
                    code: "const b = a + 1;",
                    output: vec!["b"],
                    depends: vec!["a"],
                }],
                jsx: Some("<div />"),
                ..NodeSourceGenerateCode::default()
            },
        ));
        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Broken",
            "props": {}
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let err = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap_err();
        assert!(matches!(err, Error::StatDependencyNotFound { .. }));
    }

    #[test]
    fn resolves_data_source_binding_outputs() {
        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "dataSources": [
            {
              "id": "cols",
              "type": "crd-columns",
              "config": {
                "COLUMNS_CONFIG": [{ "key": "name", "title": "Name" }]
              }
            },
            {
              "id": "unused",
              "type": "crd-columns",
              "config": {
                "COLUMNS_CONFIG": []
              }
            }
          ],
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Layout",
            "props": { "TEXT": "Hello" },
            "children": [
              {
                "id": "child",
                "type": "Text",
                "props": {
                  "TEXT": {
                    "type": "binding",
                    "source": "cols",
                    "path": "columns.0.title",
                    "defaultValue": "Unknown"
                  },
                  "DEFAULT_VALUE": 1
                }
              }
            ]
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::default()
            .generate_page_code(&page)
            .unwrap();
        assert!(code.contains("const useCols = ()=>"));
        assert!(code.contains("const { columns: colsColumns } = useCols();"));
        assert!(!code.contains("useUnused"));
        assert!(code.contains(r#"(colsColumns?.[0]?.title ?? "Unknown")"#));
    }

    #[test]
    fn renders_data_source_hook_name_as_identifier() {
        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "dataSources": [
            {
              "id": "cols",
              "type": "crd-columns",
              "config": {
                "HOOK_NAME": "useCustomColumns",
                "COLUMNS_CONFIG": [{ "key": "name", "title": "Name" }]
              }
            }
          ],
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Layout",
            "props": { "TEXT": "Hello" },
            "children": [
              {
                "id": "child",
                "type": "Text",
                "props": {
                  "TEXT": {
                    "type": "binding",
                    "source": "cols",
                    "path": "columns.0.title"
                  },
                  "DEFAULT_VALUE": 1
                }
              }
            ]
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::default()
            .generate_page_code(&page)
            .unwrap();
        assert!(code.contains("const useCustomColumns = ()=>"));
        assert!(code.contains("const { columns: colsColumns } = useCustomColumns();"));
        assert!(!code.contains("const \"useCustomColumns\""));
    }

    #[test]
    fn renders_data_source_scopes_in_target_boundary() {
        let mut registry = crate::builtins::default_registry();
        registry.register_data_source(
            DataSourceSource::new(
                "scoped-source",
                DataSourceSourceGenerateCode {
                    imports: vec![r#"import { useMemo } from "react";"#],
                    stats: vec![
                        StatSource {
                            id: "imports",
                            scope: StatementScope::ModuleImport,
                            code: r#"import { useState } from "react"; const dsModuleValue = 1;"#,
                            output: vec!["dsModuleValue"],
                            depends: vec![],
                        },
                        StatSource {
                            id: "local",
                            scope: StatementScope::FunctionBody,
                            code: "const dsLocal = dsModuleValue;",
                            output: vec!["dsLocal"],
                            depends: vec!["imports"],
                        },
                    ],
                    ..DataSourceSourceGenerateCode::default()
                },
            )
            .with_schema(DataSourceSourceSchema {
                template_inputs: Default::default(),
                outputs: [("data", TemplateOutput { ty: "any" })]
                    .into_iter()
                    .collect(),
            }),
        );
        registry.register_node(NodeSource::new(
            "Label",
            NodeSourceGenerateCode {
                jsx: Some("<div>{%%VALUE%%}</div>"),
                meta: Some(NodeSourceMeta {
                    input_paths: [("$jsx", vec!["VALUE"])].into_iter().collect(),
                    runtime_deps: Vec::new(),
                }),
                ..NodeSourceGenerateCode::default()
            },
        ));

        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "dataSources": [
            {
              "id": "scoped",
              "type": "scoped-source",
              "config": {}
            }
          ],
          "root": {
            "id": "label",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Label",
            "props": {
              "VALUE": { "type": "binding", "source": "scoped", "path": "data" }
            }
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap();
        let import_index = code
            .find(r#"import { useMemo, useState } from "react";"#)
            .unwrap();
        let module_index = code.find("const dsModuleValue = 1;").unwrap();
        let function_index = code.find("function SamplePage").unwrap();
        let local_index = code.find("const dsLocal = dsModuleValue;").unwrap();

        assert!(import_index < module_index);
        assert!(module_index < function_index);
        assert!(function_index < local_index);
        assert!(code.contains("const { data: scopedData } = useScoped();"));
    }

    #[test]
    fn passes_data_source_args_and_orders_dependencies() {
        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "dataSources": [
            {
              "id": "rows",
              "type": "crd-page-state",
              "config": {
                "CRD_CONFIG": { "plural": "widgets", "group": "example.io" }
              }
            },
            {
              "id": "cols",
              "type": "crd-columns",
              "config": {
                "COLUMNS_CONFIG": [{ "key": "name", "title": "Name" }]
              },
              "args": [
                {
                  "type": "binding",
                  "source": "rows",
                  "path": "data"
                }
              ]
            }
          ],
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Layout",
            "props": { "TEXT": "Hello" },
            "children": [
              {
                "id": "child",
                "type": "Text",
                "props": {
                  "TEXT": {
                    "type": "binding",
                    "source": "cols",
                    "path": "columns.0.title",
                    "defaultValue": "Unknown"
                  },
                  "DEFAULT_VALUE": 1
                }
              }
            ]
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::default()
            .generate_page_code(&page)
            .unwrap();
        let rows_pos = code.find("useRows();").unwrap();
        let cols_pos = code.find("useCols((rowsData ?? undefined));").unwrap();
        assert!(rows_pos < cols_pos);
        assert!(code.contains("const useStore = getCrdStore({"));
        assert!(code.contains(r#"const pageId = "rows";"#));
        assert!(code.contains(r#"const scope = "cluster";"#));
        assert!(code.contains("const { data: rowsData } = useRows();"));
        assert!(
            code.contains("const { columns: colsColumns } = useCols((rowsData ?? undefined));")
        );
    }

    #[test]
    fn allocates_unique_module_stat_outputs_for_data_sources() {
        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "dataSources": [
            {
              "id": "rowsA",
              "type": "crd-page-state",
              "config": {
                "CRD_CONFIG": { "plural": "widgets", "group": "example.io" }
              }
            },
            {
              "id": "rowsB",
              "type": "crd-page-state",
              "config": {
                "CRD_CONFIG": { "plural": "gadgets", "group": "example.io" }
              }
            }
          ],
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Layout",
            "props": { "TEXT": "Hello" },
            "children": [
              {
                "id": "child-a",
                "type": "Text",
                "props": {
                  "TEXT": {
                    "type": "binding",
                    "source": "rowsA",
                    "path": "data"
                  },
                  "DEFAULT_VALUE": 1
                }
              },
              {
                "id": "child-b",
                "type": "Text",
                "props": {
                  "TEXT": {
                    "type": "binding",
                    "source": "rowsB",
                    "path": "data"
                  },
                  "DEFAULT_VALUE": 2
                }
              }
            ]
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::default()
            .generate_page_code(&page)
            .unwrap();
        assert!(code.contains("const useStore = getCrdStore({"));
        assert!(code.contains("const useStore2 = getCrdStore({"));
        assert!(code.contains("const store = useStore({"));
        assert!(code.contains("const store = useStore2({"));
    }

    #[test]
    fn injects_action_graph_handlers_and_dispatch_stats() {
        let mut registry = Registry::default();
        registry.register_node(NodeSource::new(
            "Button",
            NodeSourceGenerateCode {
                jsx: Some("<button onClick={%%ON_CLICK%%}>{%%LABEL%%}</button>"),
                meta: Some(NodeSourceMeta {
                    input_paths: [("$jsx", vec!["ON_CLICK", "LABEL"])].into_iter().collect(),
                    runtime_deps: Vec::new(),
                }),
                ..NodeSourceGenerateCode::default()
            },
        ));

        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "root": {
            "id": "button-submit",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Button",
            "props": { "LABEL": "Submit" }
          },
          "actionGraphs": [
            {
              "id": "formGraph",
              "context": { "name": "" },
              "actions": {
                "SUBMIT": {
                  "on": "button-submit.click",
                  "do": [
                    { "type": "assign", "to": "context.name", "value": "$event.value" },
                    { "type": "reset", "path": "context.name" }
                  ]
                }
              }
            }
          ],
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap();
        assert!(code.contains(r#"import { create } from "zustand";"#));
        assert!(code.contains(r#"import { get, set } from "es-toolkit/compat";"#));
        assert!(code.contains("const useFormGraphStore = create"));
        assert!(code.contains(r#"dispatchActionFormGraph("SUBMIT""#));
        assert!(code.contains("const dispatchActionFormGraph = (actionId, event)=>"));
        assert!(code.contains("<button onClick={(event)=>"));
    }

    #[test]
    fn action_graph_context_binding_renders_context_value() {
        let mut registry = Registry::default();
        registry.register_node(NodeSource::new(
            "Label",
            NodeSourceGenerateCode {
                jsx: Some("<span>{%%VALUE%%}</span>"),
                meta: Some(NodeSourceMeta {
                    input_paths: [("$jsx", vec!["VALUE"])].into_iter().collect(),
                    runtime_deps: Vec::new(),
                }),
                ..NodeSourceGenerateCode::default()
            },
        ));

        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "root": {
            "id": "label-name",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Label",
            "props": {
              "VALUE": {
                "type": "binding",
                "target": "context",
                "source": "formGraph",
                "path": "name",
                "defaultValue": "Unknown"
              }
            }
          },
          "actionGraphs": [
            {
              "id": "formGraph",
              "context": { "name": "Ada" },
              "actions": {}
            }
          ],
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap();
        assert!(code.contains("const useFormGraphStore = create"));
        assert!(code.contains("const actionFormGraphContext = useFormGraphStore"));
        assert!(code.contains(r#"(actionFormGraphContext?.name ?? "Unknown")"#));
    }

    #[test]
    fn action_graph_handlers_do_not_override_explicit_node_props() {
        let mut registry = Registry::default();
        registry.register_node(NodeSource::new(
            "Button",
            NodeSourceGenerateCode {
                jsx: Some("<button onClick={%%ON_CLICK%%}>{%%LABEL%%}</button>"),
                meta: Some(NodeSourceMeta {
                    input_paths: [("$jsx", vec!["ON_CLICK", "LABEL"])].into_iter().collect(),
                    runtime_deps: Vec::new(),
                }),
                ..NodeSourceGenerateCode::default()
            },
        ));

        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "root": {
            "id": "button-submit",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Button",
            "props": {
              "LABEL": "Submit",
              "ON_CLICK": {
                "type": "expression",
                "code": "() => explicitSubmit()"
              }
            }
          },
          "actionGraphs": [
            {
              "id": "formGraph",
              "context": {},
              "actions": {
                "SUBMIT": {
                  "on": "button-submit.click",
                  "do": []
                }
              }
            }
          ],
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap();
        assert!(code.contains("<button onClick={()=>explicitSubmit()}"));
        assert!(!code.contains(r#"dispatchActionFormGraph("SUBMIT", { event })"#));
        assert!(code.contains("const dispatchActionFormGraph = (actionId, event)=>"));
    }

    #[test]
    fn reports_ambiguous_binding_source_without_target() {
        let mut registry = crate::builtins::default_registry();
        registry.register_node(NodeSource::new(
            "Label",
            NodeSourceGenerateCode {
                jsx: Some("<span>{%%VALUE%%}</span>"),
                meta: Some(NodeSourceMeta {
                    input_paths: [("$jsx", vec!["VALUE"])].into_iter().collect(),
                    runtime_deps: Vec::new(),
                }),
                ..NodeSourceGenerateCode::default()
            },
        ));

        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "dataSources": [
            {
              "id": "shared",
              "type": "static",
              "config": { "DATA": { "name": "Ada" } }
            }
          ],
          "root": {
            "id": "label-name",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Label",
            "props": {
              "VALUE": {
                "type": "binding",
                "source": "shared",
                "path": "name"
              }
            }
          },
          "actionGraphs": [
            {
              "id": "shared",
              "context": { "name": "Ada" },
              "actions": {}
            }
          ],
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let err = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap_err();
        assert!(matches!(err, Error::AmbiguousBindingSource { .. }));
    }

    #[test]
    fn validates_node_props_against_source_schema() {
        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Text",
            "props": { "TEXT": "Hello", "DEFAULT_VALUE": "bad" }
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let err = ComponentGenerator::default()
            .generate_page_code(&page)
            .unwrap_err();
        assert!(matches!(err, Error::InvalidPropType { .. }));
    }

    #[test]
    fn reports_unknown_node_props_for_declared_schema() {
        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Layout",
            "props": { "TEXT": "Hello", "EXTRA": true }
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let err = ComponentGenerator::default()
            .generate_page_code(&page)
            .unwrap_err();
        assert!(matches!(err, Error::UnknownProp { .. }));
    }

    #[test]
    fn passes_runtime_props_to_scoped_children() {
        let mut registry = crate::builtins::default_registry();
        registry.register_node(
            NodeSource::new(
                "PropCard",
                NodeSourceGenerateCode {
                    imports: vec![r#"import * as React from "react""#],
                    jsx: Some(
                        "<article><h3>{props.TITLE}</h3><strong>{props.COUNT}</strong></article>",
                    ),
                    ..NodeSourceGenerateCode::default()
                },
            )
            .with_schema(crate::registry::NodeSourceSchema {
                template_inputs: Default::default(),
                runtime_props: [
                    (
                        "TITLE",
                        crate::registry::TemplateInput {
                            ty: "string",
                            description: "Card title",
                        },
                    ),
                    (
                        "COUNT",
                        crate::registry::TemplateInput {
                            ty: "number",
                            description: "Card count",
                        },
                    ),
                ]
                .into_iter()
                .collect(),
            }),
        );

        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Layout",
            "props": { "TEXT": "Root" },
            "children": [
              {
                "id": "card",
                "meta": { "scope": true, "title": "CardPanel" },
                "type": "PropCard",
                "props": { "TITLE": "Hello", "COUNT": 3 }
              }
            ]
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap();
        assert!(code.contains(r#"<CardPanel TITLE={"Hello"} COUNT={3}/>"#));
        assert!(code.contains("function CardPanel(props)"));
        assert!(code.contains("<h3>{props.TITLE}</h3>"));
    }

    #[test]
    fn runtime_prop_bindings_are_resolved_in_parent_boundary() {
        let mut registry = crate::builtins::default_registry();
        registry.register_node(
            NodeSource::new(
                "PropCard",
                NodeSourceGenerateCode {
                    imports: vec![r#"import * as React from "react""#],
                    jsx: Some("<article><h3>{props.TITLE}</h3></article>"),
                    ..NodeSourceGenerateCode::default()
                },
            )
            .with_schema(crate::registry::NodeSourceSchema {
                template_inputs: Default::default(),
                runtime_props: [(
                    "TITLE",
                    crate::registry::TemplateInput {
                        ty: "string",
                        description: "Card title",
                    },
                )]
                .into_iter()
                .collect(),
            }),
        );

        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "dataSources": [
            {
              "id": "draft",
              "type": "static",
              "config": { "DATA": { "title": "Hello" } }
            }
          ],
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Layout",
            "props": { "TEXT": "Root" },
            "children": [
              {
                "id": "card",
                "meta": { "scope": true, "title": "CardPanel" },
                "type": "PropCard",
                "props": {
                  "TITLE": {
                    "type": "binding",
                    "source": "draft",
                    "path": "data.title"
                  }
                }
              }
            ]
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap();
        assert!(code.contains("const { data: draftData } = useDraft();"));
        assert!(code.contains("<CardPanel TITLE={(draftData?.title ?? undefined)}/>"));
    }

    #[test]
    fn node_runtime_deps_inject_runtime_context() {
        let mut registry = Registry::default();
        registry.register_node(NodeSource::new(
            "RuntimeLabel",
            NodeSourceGenerateCode {
                jsx: Some("<span>{__runtime__?.route?.params?.name}</span>"),
                meta: Some(NodeSourceMeta {
                    input_paths: Default::default(),
                    runtime_deps: vec!["runtime"],
                }),
                ..NodeSourceGenerateCode::default()
            },
        ));

        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "root": {
            "id": "runtime-label",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "RuntimeLabel",
            "props": {}
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap();
        assert!(
            code.contains(
                r#"import { useRuntimeContext } from "@frontend-forge/forge-components";"#
            )
        );
        assert!(code.contains("const __runtime__ = useRuntimeContext();"));
        assert!(code.contains("<span>{__runtime__?.route?.params?.name}</span>"));
    }

    #[test]
    fn action_graph_call_data_source_requires_mutate_binding() {
        let mut registry = crate::builtins::default_registry();
        registry.register_node(NodeSource::new(
            "Button",
            NodeSourceGenerateCode {
                jsx: Some("<button onClick={%%ON_CLICK%%}>{%%LABEL%%}</button>"),
                meta: Some(NodeSourceMeta {
                    input_paths: [("$jsx", vec!["ON_CLICK", "LABEL"])].into_iter().collect(),
                    runtime_deps: Vec::new(),
                }),
                ..NodeSourceGenerateCode::default()
            },
        ));

        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "dataSources": [
            {
              "id": "rows",
              "type": "crd-page-state",
              "config": {
                "CRD_CONFIG": { "plural": "widgets", "group": "example.io" }
              }
            }
          ],
          "root": {
            "id": "button-submit",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Button",
            "props": { "LABEL": "Submit" }
          },
          "actionGraphs": [
            {
              "id": "formGraph",
              "context": { "name": "Ada" },
              "actions": {
                "SUBMIT": {
                  "on": "button-submit.click",
                  "do": [
                    { "type": "callDataSource", "id": "rows", "args": ["context.name"] }
                  ]
                }
              }
            }
          ],
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap();
        assert!(code.contains("const { mutate: rowsMutate } = useRows();"));
        assert!(code.contains(r#""rows": rowsMutate"#));
        assert!(code.contains(r#"result = callActionFormGraphDataSource("rows""#));
    }

    #[test]
    fn action_graph_call_rest_data_source_uses_request_mode() {
        let mut registry = crate::builtins::default_registry();
        registry.register_node(NodeSource::new(
            "Button",
            NodeSourceGenerateCode {
                jsx: Some("<button onClick={%%ON_CLICK%%}>{%%LABEL%%}</button>"),
                meta: Some(NodeSourceMeta {
                    input_paths: [("$jsx", vec!["ON_CLICK", "LABEL"])].into_iter().collect(),
                    runtime_deps: Vec::new(),
                }),
                ..NodeSourceGenerateCode::default()
            },
        ));

        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "dataSources": [
            {
              "id": "create-user",
              "type": "rest",
              "config": {
                "URL": "/api/users",
                "METHOD": "POST",
                "DEFAULT_VALUE": null
              },
              "autoLoad": false
            }
          ],
          "root": {
            "id": "button-submit",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Button",
            "props": { "LABEL": "Submit" }
          },
          "actionGraphs": [
            {
              "id": "formGraph",
              "context": { "name": "Ada" },
              "actions": {
                "SUBMIT": {
                  "on": "button-submit.click",
                  "do": [
                    { "type": "callDataSource", "id": "create-user", "args": ["context.name"] }
                  ]
                }
              }
            }
          ],
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap();
        assert!(code.contains(r#"import useSWR from "swr";"#));
        assert!(code.contains("const fetchCreateUser = (url)=>"));
        assert!(code.contains("const useCreateUser = (options = {})=>"));
        assert!(code.contains("false ? \"/api/users\" : null"));
        assert!(code.contains("fetchCreateUser,"));
        assert!(code.contains("const { mutate: createUserMutate } = useCreateUser();"));
        assert!(code.contains(r#""create-user": "request""#));
        assert!(code.contains(r#"const url = "/api/users";"#));
        assert!(code.contains(r#"const method = ("POST" || "GET").toUpperCase();"#));
        assert!(code.contains("return mutate(effect, {"));
    }

    #[test]
    fn action_graph_call_static_data_source_uses_set_mode() {
        let mut registry = crate::builtins::default_registry();
        registry.register_node(NodeSource::new(
            "Button",
            NodeSourceGenerateCode {
                jsx: Some("<button onClick={%%ON_CLICK%%}>{%%LABEL%%}</button>"),
                meta: Some(NodeSourceMeta {
                    input_paths: [("$jsx", vec!["ON_CLICK", "LABEL"])].into_iter().collect(),
                    runtime_deps: Vec::new(),
                }),
                ..NodeSourceGenerateCode::default()
            },
        ));

        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "dataSources": [
            {
              "id": "draft",
              "type": "static",
              "config": {
                "DATA": { "name": "" }
              }
            }
          ],
          "root": {
            "id": "button-save",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Button",
            "props": { "LABEL": "Save" }
          },
          "actionGraphs": [
            {
              "id": "formGraph",
              "context": { "name": "Ada" },
              "actions": {
                "SAVE": {
                  "on": "button-save.click",
                  "do": [
                    { "type": "callDataSource", "id": "draft", "args": ["context.name"] }
                  ]
                }
              }
            }
          ],
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap();
        assert!(code.contains("const useDraft = ()=>"));
        assert!(code.contains("const { mutate: draftMutate } = useDraft();"));
        assert!(code.contains(r#""draft": "set""#));
        assert!(code.contains(r#""draft": (payload, env)=>payload"#));
        assert!(code.contains(r#"if (mode === "set" && mutate)"#));
    }

    #[test]
    fn action_graph_navigation_injects_runtime_context() {
        let mut registry = Registry::default();
        registry.register_node(NodeSource::new(
            "Button",
            NodeSourceGenerateCode {
                jsx: Some("<button onClick={%%ON_CLICK%%}>{%%LABEL%%}</button>"),
                meta: Some(NodeSourceMeta {
                    input_paths: [("$jsx", vec!["ON_CLICK", "LABEL"])].into_iter().collect(),
                    runtime_deps: Vec::new(),
                }),
                ..NodeSourceGenerateCode::default()
            },
        ));

        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "root": {
            "id": "button-detail",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Button",
            "props": { "LABEL": "Detail" }
          },
          "actionGraphs": [
            {
              "id": "navGraph",
              "context": { "detailPath": "/detail" },
              "actions": {
                "OPEN": {
                  "on": "button-detail.click",
                  "do": [
                    { "type": "navigate", "path": "context.detailPath" }
                  ]
                }
              }
            }
          ],
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::new(registry)
            .generate_page_code(&page)
            .unwrap();
        assert!(
            code.contains(
                r#"import { useRuntimeContext } from "@frontend-forge/forge-components";"#
            )
        );
        assert!(code.contains("const __runtime__ = useRuntimeContext();"));
        assert!(code.contains("__runtime__.navigation.navigate"));
    }

    #[test]
    fn injects_runtime_context_for_runtime_bindings() {
        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Layout",
            "props": { "TEXT": "Hello" },
            "children": [
              {
                "id": "child",
                "type": "Text",
                "props": {
                  "TEXT": {
                    "type": "binding",
                    "target": "runtime",
                    "path": "route.params.name",
                    "defaultValue": "Unknown"
                  },
                  "DEFAULT_VALUE": 1
                }
              }
            ]
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::default()
            .generate_page_code(&page)
            .unwrap();
        assert!(
            code.contains(
                r#"import { useRuntimeContext } from "@frontend-forge/forge-components";"#
            )
        );
        assert!(code.contains("const __runtime__ = useRuntimeContext();"));
        assert!(code.contains(r#"(__runtime__?.route?.params?.name ?? "Unknown")"#));
    }

    #[test]
    fn orders_runtime_before_data_source_args_that_use_runtime() {
        let raw = serde_json::json!({
          "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
          "dataSources": [
            {
              "id": "cols",
              "type": "crd-columns",
              "config": {
                "COLUMNS_CONFIG": [{ "key": "name", "title": "Name" }]
              },
              "args": [
                {
                  "type": "binding",
                  "target": "runtime",
                  "path": "route.params.namespace"
                }
              ]
            }
          ],
          "root": {
            "id": "root",
            "meta": { "scope": true, "title": "SamplePage" },
            "type": "Layout",
            "props": { "TEXT": "Hello" },
            "children": [
              {
                "id": "child",
                "type": "Text",
                "props": {
                  "TEXT": {
                    "type": "binding",
                    "source": "cols",
                    "path": "columns.0.title",
                    "defaultValue": "Unknown"
                  },
                  "DEFAULT_VALUE": 1
                }
              }
            ]
          },
          "context": {}
        });
        let page = unwrap_page_schema(raw).unwrap();
        let code = ComponentGenerator::default()
            .generate_page_code(&page)
            .unwrap();

        let runtime_pos = code
            .find("const __runtime__ = useRuntimeContext();")
            .unwrap();
        let cols_pos = code
            .find("useCols((__runtime__?.route?.params?.namespace ?? undefined));")
            .unwrap();
        assert!(runtime_pos < cols_pos);
    }
}
