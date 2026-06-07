use std::collections::BTreeMap;

use serde_json::Value;

use crate::error::{Error, Result};

#[derive(Clone, Default)]
pub struct BindingContext {
    pub data_sources: BTreeMap<String, DataSourceBindingInfo>,
    pub action_graphs: BTreeMap<String, ActionGraphBindingInfo>,
}

#[derive(Clone)]
pub struct ActionGraphBindingInfo {
    pub id: String,
    pub context_name: String,
}

#[derive(Clone)]
pub struct DataSourceBindingInfo {
    pub id: String,
    pub hook_name: String,
    pub fetcher_name: String,
    pub base_name: String,
    pub output_names: Vec<String>,
    pub call_mode: DataSourceCallMode,
    pub args: Vec<String>,
    pub arg_binding_uses: Vec<BindingUse>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DataSourceCallMode {
    Hook,
    Value,
}

impl Default for DataSourceCallMode {
    fn default() -> Self {
        Self::Hook
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingUse {
    DataSource { source: String, output: String },
    ActionGraph { source: String },
    Runtime,
}

pub fn value_to_expr_code(value: Option<&Value>, fallback: &str) -> Result<String> {
    value_to_expr_code_with_context(value, fallback, &BindingContext::default())
}

pub fn value_to_expr_code_with_context(
    value: Option<&Value>,
    fallback: &str,
    ctx: &BindingContext,
) -> Result<String> {
    match value {
        Some(value) => Ok(expr_to_code_with_context(value, ctx)?),
        None => Ok(fallback.to_owned()),
    }
}

pub fn collect_binding_uses(value: &Value, ctx: &BindingContext) -> Vec<BindingUse> {
    let mut out = Vec::new();
    collect_binding_uses_inner(value, ctx, &mut out);
    out
}

fn collect_binding_uses_inner(value: &Value, ctx: &BindingContext, out: &mut Vec<BindingUse>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_binding_uses_inner(item, ctx, out);
            }
        }
        Value::Object(obj) => {
            if obj.get("type").and_then(Value::as_str) == Some("binding") {
                if let Some(item) = binding_use(value, ctx) {
                    out.push(item);
                }
                return;
            }
            for item in obj.values() {
                collect_binding_uses_inner(item, ctx, out);
            }
        }
        _ => {}
    }
}

fn binding_use(value: &Value, ctx: &BindingContext) -> Option<BindingUse> {
    let obj = value.as_object()?;
    let target = obj
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or("dataSource");
    if target == "runtime" {
        return Some(BindingUse::Runtime);
    }
    if target == "context" {
        return obj
            .get("source")
            .and_then(Value::as_str)
            .map(|source| BindingUse::ActionGraph {
                source: source.to_owned(),
            });
    }
    let source = obj.get("source").and_then(Value::as_str)?;
    if ctx.action_graphs.contains_key(source) && !ctx.data_sources.contains_key(source) {
        return Some(BindingUse::ActionGraph {
            source: source.to_owned(),
        });
    }
    let path = obj
        .get("path")
        .or_else(|| obj.get("bind"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let bind = obj.get("bind").and_then(Value::as_str);
    let output = resolve_data_source_output(source, path, bind, ctx);
    Some(BindingUse::DataSource {
        source: source.to_owned(),
        output,
    })
}

pub fn expr_to_code(value: &Value) -> Result<String> {
    expr_to_code_with_context(value, &BindingContext::default())
}

pub fn expr_to_code_with_context(value: &Value, ctx: &BindingContext) -> Result<String> {
    if let Some(obj) = value.as_object() {
        if obj.get("type").and_then(Value::as_str) == Some("expression") {
            return obj
                .get("code")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(Error::ExpressionCodeRequired);
        }

        if obj.get("type").and_then(Value::as_str) == Some("binding") {
            return binding_to_code(value, ctx);
        }
    }

    serde_json::to_string(value).map_err(|source| Error::JsonValue { source })
}

fn binding_to_code(value: &Value, ctx: &BindingContext) -> Result<String> {
    let obj = value.as_object().ok_or(Error::BindingObjectRequired)?;
    let target = obj
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or("dataSource");
    let source = obj.get("source").and_then(Value::as_str).unwrap_or("");
    let path = obj
        .get("path")
        .or_else(|| obj.get("bind"))
        .and_then(Value::as_str)
        .unwrap_or("");

    let fallback = obj
        .get("defaultValue")
        .map(serde_json::to_string)
        .transpose()
        .map_err(|source| Error::JsonValue { source })?
        .unwrap_or_else(|| "undefined".to_owned());

    if obj.get("target").and_then(Value::as_str).is_none()
        && ctx.data_sources.contains_key(source)
        && ctx.action_graphs.contains_key(source)
    {
        return Err(Error::AmbiguousBindingSource {
            source_id: source.to_owned(),
        });
    }

    let code = match target {
        "runtime" => {
            if path.is_empty() {
                "__runtime__".to_owned()
            } else {
                path_access("__runtime__", &split_path(path))
            }
        }
        "context" => action_graph_binding_code(source, path, ctx)?,
        _ => {
            if source.is_empty() {
                "undefined".to_owned()
            } else if ctx.action_graphs.contains_key(source)
                && !ctx.data_sources.contains_key(source)
            {
                action_graph_binding_code(source, path, ctx)?
            } else {
                data_source_binding_code(source, path, obj.get("bind").and_then(Value::as_str), ctx)
            }
        }
    };

    Ok(format!("({code} ?? {fallback})"))
}

fn action_graph_binding_code(source: &str, path: &str, ctx: &BindingContext) -> Result<String> {
    let info = ctx
        .action_graphs
        .get(source)
        .ok_or_else(|| Error::ActionGraphNotFound {
            id: source.to_owned(),
        })?;
    if path.is_empty() {
        Ok(info.context_name.clone())
    } else {
        Ok(path_access(&info.context_name, &split_path(path)))
    }
}

fn data_source_binding_code(
    source: &str,
    path: &str,
    bind: Option<&str>,
    ctx: &BindingContext,
) -> String {
    let Some(info) = ctx.data_sources.get(source) else {
        return if path.is_empty() {
            camel_case(source)
        } else {
            path_access(&camel_case(source), &split_path(path))
        };
    };

    let mut path_parts = split_path(path);
    let output_name = resolve_data_source_output(source, path, bind, ctx);
    if bind.filter(|value| !value.is_empty()).is_none()
        && !path_parts.is_empty()
        && output_defined(&info.output_names, &path_parts[0])
    {
        path_parts.remove(0);
    }

    let base = resolve_binding_output_var_name(info, &output_name);
    if path_parts.is_empty() {
        base
    } else {
        path_access(&base, &path_parts)
    }
}

fn resolve_data_source_output(
    source: &str,
    path: &str,
    bind: Option<&str>,
    ctx: &BindingContext,
) -> String {
    let Some(info) = ctx.data_sources.get(source) else {
        return bind
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                split_path(path)
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "data".to_owned())
            });
    };
    let path_parts = split_path(path);
    if let Some(bind) = bind.filter(|value| !value.is_empty()) {
        bind.to_owned()
    } else if path_parts.is_empty() {
        default_output(&info.output_names).unwrap_or_else(|| "data".to_owned())
    } else if output_defined(&info.output_names, &path_parts[0]) {
        path_parts[0].clone()
    } else {
        default_output(&info.output_names).unwrap_or_else(|| "data".to_owned())
    }
}

pub fn resolve_binding_output_var_name(info: &DataSourceBindingInfo, output_name: &str) -> String {
    if info.call_mode == DataSourceCallMode::Value {
        return info.hook_name.clone();
    }
    match output_name {
        "data" => format!("{}Data", info.base_name),
        "error" => format!("{}Error", info.base_name),
        "isLoading" => format!("{}Loading", info.base_name),
        "mutate" => format!("{}Mutate", info.base_name),
        _ => format!("{}{}", info.base_name, pascal_case(output_name, "Output")),
    }
}

fn default_output(outputs: &[String]) -> Option<String> {
    if outputs.iter().any(|output| output == "data") {
        return Some("data".to_owned());
    }
    if outputs.len() == 1 {
        return outputs.first().cloned();
    }
    if outputs.is_empty() {
        return Some("data".to_owned());
    }
    None
}

fn output_defined(outputs: &[String], output: &str) -> bool {
    outputs.is_empty() || outputs.iter().any(|item| item == output)
}

pub fn camel_case(input: &str) -> String {
    let pascal = pascal_case(input, "Value");
    let mut chars = pascal.chars();
    let Some(first) = chars.next() else {
        return "value".to_owned();
    };
    format!(
        "{}{}",
        first.to_ascii_lowercase(),
        chars.collect::<String>()
    )
}

pub fn pascal_case(input: &str, fallback: &str) -> String {
    let parts = input
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return fallback.to_owned();
    }

    let mut out = String::new();
    for part in parts {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(chars);
        }
    }
    if out.is_empty() {
        fallback.to_owned()
    } else {
        out
    }
}

fn split_path(path: &str) -> Vec<String> {
    path.split('.')
        .filter(|segment| !segment.is_empty())
        .map(str::to_owned)
        .collect()
}

fn path_access(base: &str, parts: &[String]) -> String {
    let mut out = base.to_owned();
    for part in parts {
        if is_identifier(part) {
            out.push_str("?.");
            out.push_str(part);
        } else if part.chars().all(|ch| ch.is_ascii_digit()) {
            out.push_str("?.[");
            out.push_str(part);
            out.push(']');
        } else {
            out.push_str("?.[");
            out.push_str(&serde_json::to_string(part).unwrap_or_else(|_| "\"\"".to_owned()));
            out.push(']');
        }
    }
    out
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first == '$' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}
