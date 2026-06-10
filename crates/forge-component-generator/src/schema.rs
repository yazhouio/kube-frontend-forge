use serde_json::{Map, Value, json};

use crate::registry::Registry;

pub fn component_tree_schema(registry: &Registry) -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://frontend-forge.dev/schemas/component-tree.schema.json",
        "title": "Frontend Forge Component Tree",
        "$ref": "#/$defs/componentTree",
        "$defs": component_tree_defs(registry),
    })
}

fn component_tree_defs(registry: &Registry) -> Value {
    let mut defs = Map::new();
    defs.insert("componentTree".to_owned(), component_tree_root_schema());
    defs.insert("pageMeta".to_owned(), page_meta_schema());
    defs.insert("componentMeta".to_owned(), component_meta_schema());
    defs.insert("componentNode".to_owned(), component_node_schema(registry));
    defs.insert(
        "dataSourceNode".to_owned(),
        data_source_node_schema(registry),
    );
    defs.insert("actionGraph".to_owned(), action_graph_schema());
    defs.insert("actionNode".to_owned(), action_node_schema());
    defs.insert("actionStep".to_owned(), action_step_schema());
    Value::Object(defs)
}

fn component_tree_root_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["meta", "root"],
        "properties": {
            "meta": { "$ref": "#/$defs/pageMeta" },
            "dataSources": {
                "type": "array",
                "items": { "$ref": "#/$defs/dataSourceNode" },
                "default": []
            },
            "actionGraphs": {
                "type": "array",
                "items": { "$ref": "#/$defs/actionGraph" },
                "default": []
            },
            "root": { "$ref": "#/$defs/componentNode" },
            "context": { "default": {} }
        }
    })
}

fn page_meta_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "name"],
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "name": { "type": "string", "minLength": 1 },
            "title": { "type": "string" },
            "path": { "type": "string" }
        }
    })
}

fn component_meta_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "scope": { "type": "boolean", "default": false },
            "title": { "type": "string" }
        }
    })
}

fn component_node_schema(registry: &Registry) -> Value {
    let branches = registry
        .node_definitions()
        .map(|definition| {
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["id", "type"],
                "properties": {
                    "id": { "type": "string", "minLength": 1 },
                    "type": { "const": definition.id() },
                    "props": definition.props_json_schema(),
                    "meta": { "$ref": "#/$defs/componentMeta" },
                    "children": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/componentNode" },
                        "default": []
                    }
                }
            })
        })
        .collect::<Vec<_>>();

    json!({
        "oneOf": branches,
    })
}

fn data_source_node_schema(registry: &Registry) -> Value {
    let branches = registry
        .data_source_definitions()
        .map(|definition| {
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["id", "type"],
                "properties": {
                    "id": { "type": "string", "minLength": 1 },
                    "type": { "const": definition.id() },
                    "config": definition.config_json_schema(),
                    "args": {
                        "type": "array",
                        "items": true,
                        "default": []
                    },
                    "autoLoad": { "type": "boolean" },
                    "polling": true
                },
                "x-outputs": definition.outputs()
            })
        })
        .collect::<Vec<_>>();

    json!({
        "oneOf": branches,
    })
}

fn action_graph_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["id"],
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "context": { "default": {} },
            "actions": {
                "type": "object",
                "additionalProperties": { "$ref": "#/$defs/actionNode" },
                "default": {}
            }
        }
    })
}

fn action_node_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["on"],
        "properties": {
            "on": { "type": "string", "minLength": 1 },
            "do": {
                "type": "array",
                "items": { "$ref": "#/$defs/actionStep" },
                "default": []
            }
        }
    })
}

fn action_step_schema() -> Value {
    json!({
        "oneOf": [
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["type", "to", "value"],
                "properties": {
                    "type": { "const": "assign" },
                    "to": { "type": "string", "minLength": 1 },
                    "value": { "type": "string" }
                }
            },
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["type", "id"],
                "properties": {
                    "type": { "const": "callDataSource" },
                    "id": { "type": "string", "minLength": 1 },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "default": []
                    }
                }
            },
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["type", "path"],
                "properties": {
                    "type": { "const": "reset" },
                    "path": { "type": "string", "minLength": 1 }
                }
            },
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["type", "path"],
                "properties": {
                    "type": { "const": "navigate" },
                    "path": { "type": "string", "minLength": 1 }
                }
            },
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["type"],
                "properties": {
                    "type": { "const": "goBack" }
                }
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use jsonschema::validator_for;
    use serde_json::json;

    use super::component_tree_schema;
    use crate::builtins::default_registry;

    #[test]
    fn component_tree_schema_accepts_registered_node_sources() {
        let schema = component_tree_schema(&default_registry());
        let validator = validator_for(&schema).unwrap();

        validator
            .validate(&json!({
                "meta": { "id": "sample", "name": "Sample" },
                "root": {
                    "id": "root",
                    "type": "Layout",
                    "props": { "TEXT": "Hello" },
                    "children": [
                        {
                            "id": "child",
                            "type": "Text",
                            "props": { "TEXT": "World", "DEFAULT_VALUE": 1 }
                        }
                    ]
                },
                "context": {}
            }))
            .unwrap();
    }

    #[test]
    fn component_tree_schema_rejects_unknown_node_props_when_schema_is_declared() {
        let schema = component_tree_schema(&default_registry());
        let validator = validator_for(&schema).unwrap();
        let error = validator
            .validate(&json!({
                "meta": { "id": "sample", "name": "Sample" },
                "root": {
                    "id": "root",
                    "type": "Layout",
                    "props": { "TEXT": "Hello", "UNKNOWN": true }
                },
                "context": {}
            }))
            .unwrap_err()
            .to_string();

        assert!(error.contains("not valid under any of the schemas listed in the 'oneOf' keyword"));
    }
}
