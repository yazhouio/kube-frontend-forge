use serde_json::{Map, Value, json};

pub fn manifest_schema(component_tree_schema: Value) -> Value {
    let mut defs = component_defs(component_tree_schema);
    defs.insert("manifest".to_owned(), manifest_definition());
    defs.insert("route".to_owned(), route_definition());
    defs.insert("menu".to_owned(), menu_definition());
    defs.insert("locale".to_owned(), locale_definition());
    defs.insert("page".to_owned(), page_definition());
    defs.insert("build".to_owned(), build_definition());

    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://frontend-forge.dev/schemas/manifest.schema.json",
        "title": "Frontend Forge Manifest",
        "oneOf": [
            { "$ref": "#/$defs/manifest" },
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["manifest"],
                "properties": {
                    "manifest": { "$ref": "#/$defs/manifest" }
                }
            }
        ],
        "$defs": defs
    })
}

fn component_defs(component_tree_schema: Value) -> Map<String, Value> {
    component_tree_schema
        .get("$defs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn manifest_definition() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["version", "name", "routes", "menus", "locales", "pages"],
        "properties": {
            "version": { "const": "1.0" },
            "name": { "type": "string", "minLength": 1 },
            "displayName": { "type": "string" },
            "description": { "type": "string" },
            "routes": {
                "type": "array",
                "items": { "$ref": "#/$defs/route" }
            },
            "menus": {
                "type": "array",
                "items": { "$ref": "#/$defs/menu" }
            },
            "locales": {
                "type": "array",
                "items": { "$ref": "#/$defs/locale" }
            },
            "pages": {
                "type": "array",
                "items": { "$ref": "#/$defs/page" }
            },
            "build": { "$ref": "#/$defs/build" }
        }
    })
}

fn route_definition() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["path", "pageId"],
        "properties": {
            "path": { "type": "string", "minLength": 1 },
            "pageId": { "type": "string", "minLength": 1 }
        }
    })
}

fn menu_definition() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["parent", "name", "title"],
        "properties": {
            "parent": { "type": "string", "minLength": 1 },
            "name": { "type": "string", "minLength": 1 },
            "title": { "type": "string", "minLength": 1 },
            "icon": { "type": "string" },
            "order": { "type": "number" },
            "clusterModule": { "type": "string" },
            "skipWorkspaceAuth": { "type": "boolean" }
        }
    })
}

fn locale_definition() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["lang", "messages"],
        "properties": {
            "lang": { "type": "string", "minLength": 1 },
            "messages": {
                "type": "object",
                "additionalProperties": { "type": "string" }
            }
        }
    })
}

fn page_definition() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "entryComponent", "componentsTree"],
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "entryComponent": { "type": "string", "minLength": 1 },
            "componentsTree": { "$ref": "#/$defs/componentTree" }
        }
    })
}

fn build_definition() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["target"],
        "properties": {
            "target": { "const": "kubesphere-extension" },
            "moduleName": { "type": "string" },
            "namespace": { "type": "string" },
            "cluster": { "type": "string" },
            "systemjs": { "type": "boolean" },
            "format": { "enum": ["esm", "systemjs"] }
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::manifest_schema;

    #[test]
    fn manifest_schema_embeds_component_tree_defs() {
        let schema = manifest_schema(json!({
            "$defs": {
                "componentTree": {
                    "type": "object",
                    "required": ["root"],
                    "properties": { "root": true }
                }
            }
        }));

        assert_eq!(
            schema
                .pointer("/$defs/page/properties/componentsTree/$ref")
                .and_then(|value| value.as_str()),
            Some("#/$defs/componentTree")
        );
        assert!(schema.pointer("/$defs/componentTree").is_some());
    }
}
