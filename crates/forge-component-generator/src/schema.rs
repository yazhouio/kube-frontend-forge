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

pub fn node_source_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://frontend-forge.dev/schemas/node-source.schema.json",
        "title": "Frontend Forge NodeSource",
        "$ref": "#/$defs/nodeSource",
        "$defs": source_defs_with("nodeSource", node_source_definition()),
    })
}

pub fn data_source_source_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://frontend-forge.dev/schemas/data-source-source.schema.json",
        "title": "Frontend Forge DataSourceSource",
        "$ref": "#/$defs/dataSourceSource",
        "$defs": source_defs_with("dataSourceSource", data_source_source_definition()),
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

fn source_defs_with(root_name: &str, root_schema: Value) -> Value {
    let mut defs = Map::new();
    defs.insert(root_name.to_owned(), root_schema);
    defs.insert("templateInput".to_owned(), template_input_definition());
    defs.insert("templateOutput".to_owned(), template_output_definition());
    defs.insert("statSource".to_owned(), stat_source_definition());
    defs.insert("statementScope".to_owned(), statement_scope_definition());
    defs.insert("nodeSourceMeta".to_owned(), node_source_meta_definition());
    defs.insert("templateDefault".to_owned(), template_default_definition());
    Value::Object(defs)
}

fn node_source_definition() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "generateCode"],
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "templateInputs": {
                        "type": "object",
                        "additionalProperties": { "$ref": "#/$defs/templateInput" },
                        "default": {}
                    },
                    "runtimeProps": {
                        "type": "object",
                        "additionalProperties": { "$ref": "#/$defs/templateInput" },
                        "default": {}
                    }
                },
                "default": {}
            },
            "generateCode": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "imports": {
                        "type": "array",
                        "items": { "type": "string" },
                        "default": []
                    },
                    "stats": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/statSource" },
                        "default": []
                    },
                    "jsx": { "type": "string" },
                    "meta": { "$ref": "#/$defs/nodeSourceMeta" }
                }
            }
        }
    })
}

fn data_source_source_definition() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "generateCode"],
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "templateInputs": {
                        "type": "object",
                        "additionalProperties": { "$ref": "#/$defs/templateInput" },
                        "default": {}
                    },
                    "outputs": {
                        "type": "object",
                        "additionalProperties": { "$ref": "#/$defs/templateOutput" },
                        "default": {}
                    }
                },
                "default": {}
            },
            "generateCode": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "imports": {
                        "type": "array",
                        "items": { "type": "string" },
                        "default": []
                    },
                    "stats": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/statSource" },
                        "default": []
                    },
                    "meta": { "$ref": "#/$defs/nodeSourceMeta" },
                    "callMode": {
                        "type": "string",
                        "enum": ["hook", "value"],
                        "default": "hook"
                    },
                    "actionMode": {
                        "type": "string",
                        "enum": ["set", "request", "mutate"],
                        "default": "mutate"
                    },
                    "defaults": {
                        "type": "object",
                        "additionalProperties": { "$ref": "#/$defs/templateDefault" },
                        "default": {}
                    }
                }
            }
        }
    })
}

fn template_input_definition() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["type"],
        "properties": {
            "type": {
                "type": "string",
                "enum": ["array", "boolean", "integer", "number", "object", "string"]
            },
            "description": { "type": "string", "default": "" }
        }
    })
}

fn template_output_definition() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["type"],
        "properties": {
            "type": { "type": "string", "minLength": 1 }
        }
    })
}

fn stat_source_definition() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "scope", "code"],
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "scope": { "$ref": "#/$defs/statementScope" },
            "code": { "type": "string" },
            "output": {
                "type": "array",
                "items": { "type": "string", "minLength": 1 },
                "default": []
            },
            "depends": {
                "type": "array",
                "items": { "type": "string", "minLength": 1 },
                "default": []
            }
        }
    })
}

fn statement_scope_definition() -> Value {
    json!({
        "type": "string",
        "enum": [
            "moduleImport",
            "moduleDecl",
            "moduleInit",
            "functionDecl",
            "functionBody",
            "block",
            "controlFlow",
            "jsx"
        ]
    })
}

fn node_source_meta_definition() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "inputPaths": {
                "type": "object",
                "additionalProperties": {
                    "type": "array",
                    "items": { "type": "string", "minLength": 1 }
                },
                "default": {}
            },
            "runtimeDeps": {
                "type": "array",
                "items": { "type": "string", "minLength": 1 },
                "default": []
            }
        }
    })
}

fn template_default_definition() -> Value {
    json!({
        "oneOf": [
            { "type": "string" },
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["type"],
                "properties": {
                    "type": { "const": "dataSourceIdJson" }
                }
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use jsonschema::validator_for;
    use serde_json::json;

    use super::{component_tree_schema, data_source_source_schema, node_source_schema};
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

    #[test]
    fn node_source_schema_accepts_compatible_source_shape() {
        let schema = node_source_schema();
        let validator = validator_for(&schema).unwrap();

        validator
            .validate(&json!({
                "id": "Layout",
                "schema": {
                    "templateInputs": {
                        "TEXT": {
                            "type": "string",
                            "description": "Layout text"
                        }
                    }
                },
                "generateCode": {
                    "imports": ["import * as React from \"react\""],
                    "jsx": "<div>{%%TEXT%%}</div>",
                    "stats": [],
                    "meta": {
                        "inputPaths": {
                            "$jsx": ["TEXT"]
                        }
                    }
                }
            }))
            .unwrap();
    }

    #[test]
    fn data_source_source_schema_accepts_registered_source_shape() {
        let schema = data_source_source_schema();
        let validator = validator_for(&schema).unwrap();

        validator
            .validate(&json!({
                "id": "rest",
                "schema": {
                    "templateInputs": {
                        "URL": {
                            "type": "string",
                            "description": "Request URL"
                        }
                    },
                    "outputs": {
                        "data": { "type": "object" }
                    }
                },
                "generateCode": {
                    "imports": ["import useSWR from \"swr\""],
                    "stats": [
                        {
                            "id": "hookDecl",
                            "scope": "moduleDecl",
                            "code": "const %%HOOK_NAME%% = () => null;",
                            "output": ["HOOK_NAME"],
                            "depends": []
                        }
                    ],
                    "meta": {
                        "inputPaths": {
                            "hookDecl": ["HOOK_NAME"]
                        }
                    },
                    "actionMode": "request",
                    "defaults": {
                        "URL": "undefined"
                    }
                }
            }))
            .unwrap();
    }
}
