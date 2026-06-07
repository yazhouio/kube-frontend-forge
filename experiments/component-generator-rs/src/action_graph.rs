use std::collections::BTreeMap;

use indexmap::{IndexMap, IndexSet};
use serde_json::Value;

use crate::ast::parse_module_items;
use crate::error::{Error, Result};
use crate::model::{ActionGraphSchema, ActionStep, DataSourceNode};
use crate::registry::RenderContext;
use crate::value::{
    ActionGraphBindingInfo, BindingContext, BindingUse, camel_case, expr_to_code, pascal_case,
    resolve_binding_output_var_name,
};

#[derive(Default)]
pub struct ActionGraphPlan {
    graphs: IndexMap<String, ActionGraphSchema>,
    info: IndexMap<String, ActionGraphInfo>,
    handlers_by_node: BTreeMap<String, Vec<ActionHandler>>,
}

#[derive(Clone)]
pub struct ActionHandler {
    pub graph_id: String,
    pub prop_name: String,
    pub value: Value,
}

#[derive(Clone)]
struct ActionGraphInfo {
    base_name: String,
    store_name: String,
    context_name: String,
    set_context_name: String,
    dispatch_name: String,
    resolve_name: String,
    get_path_name: String,
    set_path_name: String,
    call_data_source_name: String,
}

impl ActionGraphPlan {
    pub fn new(graphs: &[ActionGraphSchema]) -> Result<Self> {
        let mut plan = Self::default();
        for graph in graphs {
            plan.graphs.insert(graph.id.clone(), graph.clone());
            plan.info
                .insert(graph.id.clone(), ActionGraphInfo::new(&graph.id));
        }

        for graph in graphs {
            for (action_id, action) in &graph.actions {
                let (node_id, event_name) = parse_trigger(&graph.id, &action.on)?;
                let info = plan
                    .info
                    .get(&graph.id)
                    .ok_or_else(|| Error::ActionGraphNotFound {
                        id: graph.id.clone(),
                    })?;
                plan.handlers_by_node
                    .entry(node_id)
                    .or_default()
                    .push(ActionHandler {
                        graph_id: graph.id.clone(),
                        prop_name: event_prop_name(&event_name),
                        value: expression_value(build_handler_expression(
                            &event_name,
                            &info.dispatch_name,
                            action_id,
                        )),
                    });
            }
        }

        Ok(plan)
    }

    pub fn handlers_for_node(&self, node_id: &str) -> &[ActionHandler] {
        self.handlers_by_node
            .get(node_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn add_to_binding_context(&self, ctx: &mut BindingContext) {
        for (id, info) in &self.info {
            ctx.action_graphs.insert(
                id.clone(),
                ActionGraphBindingInfo {
                    id: id.clone(),
                    context_name: info.context_name.clone(),
                },
            );
        }
    }

    pub fn binding_uses_for_graphs(&self, graph_ids: &IndexSet<String>) -> Vec<BindingUse> {
        let mut out = Vec::new();
        for graph_id in graph_ids {
            let Some(graph) = self.graphs.get(graph_id) else {
                continue;
            };
            for action in graph.actions.values() {
                for step in &action.steps {
                    if let ActionStep::CallDataSource { id, .. } = step {
                        out.push(BindingUse::DataSource {
                            source: id.clone(),
                            output: "mutate".to_owned(),
                        });
                    }
                }
            }
        }
        out
    }

    pub fn graphs_use_runtime(&self, graph_ids: &IndexSet<String>) -> bool {
        graph_ids.iter().any(|graph_id| {
            self.graphs.get(graph_id).is_some_and(|graph| {
                graph.actions.values().any(|action| {
                    action.steps.iter().any(|step| {
                        matches!(step, ActionStep::Navigate { .. } | ActionStep::GoBack)
                    })
                })
            })
        })
    }

    pub fn render_boundary_stats(
        &self,
        graph_ids: &IndexSet<String>,
        ctx: &mut RenderContext,
        data_sources_by_id: &BTreeMap<&str, &DataSourceNode>,
    ) -> Result<Vec<String>> {
        if graph_ids.is_empty() {
            return Ok(Vec::new());
        }

        ctx.imports.add_named("zustand", "create");
        ctx.imports.add_named("es-toolkit/compat", "get");
        ctx.imports.add_named("es-toolkit/compat", "set");

        let mut stats = Vec::new();
        for graph_id in graph_ids {
            let graph = self
                .graphs
                .get(graph_id)
                .ok_or_else(|| Error::ActionGraphNotFound {
                    id: graph_id.clone(),
                })?;
            let info = self
                .info
                .get(graph_id)
                .ok_or_else(|| Error::ActionGraphNotFound {
                    id: graph_id.clone(),
                })?;
            if !ctx.rendered_action_graph_stores.contains(graph_id) {
                let store_code = build_store_code(graph, info)?;
                ctx.module_items.extend(parse_module_items(&store_code)?);
                ctx.rendered_action_graph_stores.insert(graph_id.clone());
            }
            stats.extend(build_function_stats(
                graph,
                info,
                &ctx.bindings,
                data_sources_by_id,
            )?);
        }
        Ok(stats)
    }
}

impl ActionGraphInfo {
    fn new(id: &str) -> Self {
        let base_name = camel_case(id);
        let pascal_name = pascal_case(id, "ActionGraph");
        Self {
            store_name: format!("use{pascal_name}Store"),
            context_name: format!("action{pascal_name}Context"),
            set_context_name: format!("setAction{pascal_name}Context"),
            dispatch_name: format!("dispatchAction{pascal_name}"),
            resolve_name: format!("resolveAction{pascal_name}"),
            get_path_name: format!("getAction{pascal_name}Path"),
            set_path_name: format!("setAction{pascal_name}Path"),
            call_data_source_name: format!("callAction{pascal_name}DataSource"),
            base_name,
        }
    }
}

fn parse_trigger(graph_id: &str, trigger: &str) -> Result<(String, String)> {
    let parts = trigger
        .split('.')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() < 2 {
        return Err(Error::InvalidActionTrigger {
            graph_id: graph_id.to_owned(),
            trigger: trigger.to_owned(),
        });
    }
    Ok((parts[0].to_owned(), parts[1..].join(".")))
}

fn event_prop_name(event_name: &str) -> String {
    let normalized = event_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("ON_{normalized}")
}

fn build_handler_expression(event_name: &str, dispatch_name: &str, action_id: &str) -> String {
    let payload = if event_name == "change" {
        "{ value: event && event.target ? event.target.value : undefined, event }"
    } else {
        "{ event }"
    };
    format!(r#"(event) => {{ {dispatch_name}("{action_id}", {payload}); }}"#)
}

fn expression_value(code: String) -> Value {
    serde_json::json!({
        "type": "expression",
        "code": code,
    })
}

fn build_store_code(graph: &ActionGraphSchema, info: &ActionGraphInfo) -> Result<String> {
    let context = expr_to_code(&graph.context)?;
    Ok(format!(
        r#"const {store} = create((set) => ({{
  context: {context},
  setContext: (next) =>
    set((prev) => ({{
      ...prev,
      context: {{
        ...(prev.context || {{}}),
        ...(next || {{}}),
      }},
    }})),
}}));"#,
        store = info.store_name
    ))
}

fn build_function_stats(
    graph: &ActionGraphSchema,
    info: &ActionGraphInfo,
    bindings: &BindingContext,
    data_sources_by_id: &BTreeMap<&str, &DataSourceNode>,
) -> Result<Vec<String>> {
    let mut stats = vec![
        format!(
            "const {context} = {store}((state) => state.context), {set_context} = {store}((state) => state.setContext);",
            context = info.context_name,
            set_context = info.set_context_name,
            store = info.store_name,
        ),
        format!(
            r#"const {get_path} = (target, path) => {{
  if (!path) {{
    return target;
  }}
  return get(target, path);
}};"#,
            get_path = info.get_path_name,
        ),
        format!(
            r#"const {resolve} = (value, event, context) => {{
  if (value && typeof value === "object") {{
    if (value.type === "event") {{
      return {get_path}(event, value.path);
    }}
    if (value.type === "context") {{
      return {get_path}(context, value.path);
    }}
  }}
  return value;
}};"#,
            resolve = info.resolve_name,
            get_path = info.get_path_name,
        ),
        format!(
            r#"const {set_path} = (target, path, value) => {{
  const cleaned = String(path || "");
  if (!cleaned) {{
    return value;
  }}
  const base = target ? {{ ...target }} : {{}};
  return set(base, cleaned, value);
}};"#,
            set_path = info.set_path_name,
        ),
    ];

    if graph_uses_data_source(graph) {
        stats.push(build_call_data_source_stat(
            graph,
            info,
            bindings,
            data_sources_by_id,
        )?);
    }
    stats.push(build_dispatch_stat(graph, info));
    Ok(stats)
}

fn graph_uses_data_source(graph: &ActionGraphSchema) -> bool {
    graph.actions.values().any(|action| {
        action
            .steps
            .iter()
            .any(|step| matches!(step, ActionStep::CallDataSource { .. }))
    })
}

fn build_call_data_source_stat(
    graph: &ActionGraphSchema,
    info: &ActionGraphInfo,
    bindings: &BindingContext,
    data_sources_by_id: &BTreeMap<&str, &DataSourceNode>,
) -> Result<String> {
    let mut handler_entries = IndexSet::<String>::new();
    let mut mode_entries = IndexSet::<String>::new();
    let mut mutate_entries = IndexSet::<String>::new();
    for action in graph.actions.values() {
        for step in &action.steps {
            if let ActionStep::CallDataSource { id, .. } = step {
                let data_source = data_sources_by_id
                    .get(id.as_str())
                    .ok_or_else(|| Error::BindingSourceNotFound { id: id.clone() })?;
                if let Some(binding) = bindings.data_sources.get(id) {
                    mutate_entries.insert(format!(
                        r#""{id}": {}"#,
                        resolve_binding_output_var_name(binding, "mutate")
                    ));
                }
                match data_source.ty.as_str() {
                    "static" => {
                        mode_entries.insert(format!(r#""{id}": "set""#));
                        handler_entries.insert(format!(r#""{id}": (payload, env) => payload"#));
                    }
                    "rest" => {
                        mode_entries.insert(format!(r#""{id}": "request""#));
                        handler_entries.insert(format!(
                            r#""{id}": (payload, env) => {{
  const url = {url};
  const method = ({method} || "GET").toUpperCase();
  const headers = {headers};
  const fetcher = env.fetch || fetch;
  const request = () => {{
    if (method !== "GET") {{
      return fetcher(url, {{
        method,
        headers: {{ "Content-Type": "application/json", ...(headers || {{}}) }},
        body: JSON.stringify(payload),
      }}).then((res) => res.json());
    }}
    return fetcher(url, {{ method, headers: headers || undefined }}).then((res) => res.json());
  }};
  return request();
}}"#,
                            id = id,
                            url = config_expr(data_source, "URL", "undefined")?,
                            method = config_expr(data_source, "METHOD", r#""GET""#)?,
                            headers = config_expr(data_source, "HEADERS", "undefined")?,
                        ));
                    }
                    _ => {
                        mode_entries.insert(format!(r#""{id}": "mutate""#));
                    }
                }
            }
        }
    }

    let handler_map = format!(
        "const {data_sources} = {{ {} }};",
        handler_entries.into_iter().collect::<Vec<_>>().join(", "),
        data_sources = info.data_source_map_name(),
    );
    let mode_map = format!(
        "const {base}DataSourceMode = {{ {} }};",
        mode_entries.into_iter().collect::<Vec<_>>().join(", "),
        base = info.base_name,
    );
    let mutate_map = format!(
        "const {base}DataSourceMutate = {{ {} }};",
        mutate_entries.into_iter().collect::<Vec<_>>().join(", "),
        base = info.base_name,
    );
    Ok(format!(
        r#"{handler_map}
{mode_map}
{mutate_map}
const {call_name} = (dataSourceId, args, event, context) => {{
  const resolvedArgs = (args || []).map((arg) => {resolve}(arg, event, context));
  const payload = resolvedArgs.length <= 1 ? resolvedArgs[0] : resolvedArgs;
  const handler = {data_sources}[dataSourceId];
  const mode = {base}DataSourceMode[dataSourceId];
  const mutate = {base}DataSourceMutate[dataSourceId];
  const env = {{ fetch, mutate }};
  if (mode === "set" && mutate) {{
    return mutate(payload);
  }}
  if (mode === "request" && handler) {{
    const effect = () => handler(payload, env);
    if (mutate) {{
      return mutate(effect, {{ revalidate: false }});
    }}
    return effect();
  }}
  if (mutate) {{
    return mutate(payload);
  }}
  if (handler) {{
    return handler(payload, env);
  }}
  return undefined;
}};"#,
        call_name = info.call_data_source_name,
        resolve = info.resolve_name,
        base = info.base_name,
        data_sources = info.data_source_map_name(),
    ))
}

impl ActionGraphInfo {
    fn data_source_map_name(&self) -> String {
        format!(
            "action{}DataSources",
            pascal_case(&self.base_name, "ActionGraph")
        )
    }
}

fn config_expr(data_source: &DataSourceNode, key: &str, fallback: &str) -> Result<String> {
    data_source
        .config
        .get(key)
        .map(expr_to_code)
        .transpose()
        .map(|value| value.unwrap_or_else(|| fallback.to_owned()))
}

fn build_dispatch_stat(graph: &ActionGraphSchema, info: &ActionGraphInfo) -> String {
    let mut cases = String::new();
    for (action_id, action) in &graph.actions {
        let mut statements = String::new();
        for step in &action.steps {
            statements.push_str(&step_statement(step, info));
        }
        cases.push_str(&format!(
            r#"case "{action_id}": {{
{statements}
  break;
}}
"#
        ));
    }
    format!(
        r#"const {dispatch} = (actionId, event) => {{
  let nextContext = {context};
  let changed = false;
  let result;
  switch (actionId) {{
{cases}    default:
      break;
  }}
  if (changed) {{
    {set_context}(nextContext);
  }}
  return result;
}};"#,
        dispatch = info.dispatch_name,
        context = info.context_name,
        set_context = info.set_context_name,
    )
}

fn step_statement(step: &ActionStep, info: &ActionGraphInfo) -> String {
    match step {
        ActionStep::Assign { to, value } => {
            format!(
                r#"  nextContext = {set_path}(nextContext, "{path}", {resolve}({value}, event, nextContext));
  changed = true;
"#,
                set_path = info.set_path_name,
                resolve = info.resolve_name,
                path = strip_context_prefix(to),
                value = action_value_expr(value),
            )
        }
        ActionStep::Reset { path } => {
            format!(
                r#"  nextContext = {set_path}(nextContext, "{path}", "");
  changed = true;
"#,
                set_path = info.set_path_name,
                path = strip_context_prefix(path),
            )
        }
        ActionStep::Navigate { path } => {
            format!(
                r#"  __runtime__.navigation.navigate({resolve}({path}, event, nextContext));
"#,
                resolve = info.resolve_name,
                path = action_value_expr(path),
            )
        }
        ActionStep::GoBack => "  __runtime__.navigation.goBack();\n".to_owned(),
        ActionStep::CallDataSource { id, args } => {
            let args_code = if args.is_empty() {
                "undefined".to_owned()
            } else {
                format!(
                    "[{}]",
                    args.iter()
                        .map(|arg| action_value_expr(arg))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            format!(
                r#"  result = {call_name}("{id}", {args_code}, event, nextContext);
"#,
                call_name = info.call_data_source_name,
            )
        }
    }
}

fn strip_context_prefix(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed == "context" {
        return String::new();
    }
    trimmed
        .strip_prefix("context.")
        .unwrap_or(trimmed)
        .to_owned()
}

fn action_value_expr(value: &str) -> String {
    if let Some(path) = value.strip_prefix("$event") {
        return format!(r#"{{ type: "event", path: "{}" }}"#, normalize_path(path));
    }
    if value == "context" || value.starts_with("context.") {
        return format!(
            r#"{{ type: "context", path: "{}" }}"#,
            normalize_path(value.strip_prefix("context").unwrap_or(""))
        );
    }
    serde_json::to_string(value).unwrap_or_else(|_| "undefined".to_owned())
}

fn normalize_path(path: &str) -> String {
    path.strip_prefix('.').unwrap_or(path).to_owned()
}
