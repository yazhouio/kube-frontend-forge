use forge_component_generator::{
    ComponentGenerator, JsCodeBackend, OxcCodeBackend,
    builtins::default_registry,
    registry::{
        DataSourceSource, DataSourceSourceGenerateCode, DataSourceSourceSchema, NodeSource,
        NodeSourceGenerateCode, NodeSourceMeta, Registry, StatSource, StatementScope,
        TemplateOutput,
    },
    unwrap_page_schema,
    value::DataSourceActionMode,
};
use serde_json::Value;

fn compact_code(value: &str) -> String {
    value.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn assert_code_contains(case_name: &str, code: &str, expected: &str) {
    if code.contains(expected) {
        return;
    }
    assert!(
        compact_code(code).contains(&compact_code(expected)),
        "{case_name} expected generated code to contain `{expected}`\n\n{code}"
    );
}

fn assert_clean_generated_code(case_name: &str, code: &str) {
    assert_code_contains(case_name, code, "export default");
    for marker in ["%%", "__ENGINE_CHILDREN__"] {
        assert!(
            !code.contains(marker),
            "{case_name} left template marker `{marker}` in generated code\n\n{code}"
        );
    }
    OxcCodeBackend
        .validate_module_items(code)
        .unwrap_or_else(|err| panic!("{case_name} generated invalid TSX: {err}\n\n{code}"));
}

fn generate_default_case(case_name: &str, tree: Value) -> String {
    let page = unwrap_page_schema(tree)
        .unwrap_or_else(|err| panic!("{case_name} failed to parse componentsTree: {err}"));
    let code = ComponentGenerator::default()
        .generate_page_code(&page)
        .unwrap_or_else(|err| panic!("{case_name} failed to generate code: {err}"));
    assert_clean_generated_code(case_name, &code);
    code
}

fn generate_with_registry(case_name: &str, registry: Registry, tree: Value) -> String {
    let page = unwrap_page_schema(tree)
        .unwrap_or_else(|err| panic!("{case_name} failed to parse componentsTree: {err}"));
    let code = ComponentGenerator::new(registry)
        .generate_page_code(&page)
        .unwrap_or_else(|err| panic!("{case_name} failed to generate code: {err}"));
    assert_clean_generated_code(case_name, &code);
    code
}

#[test]
fn built_in_components_tree_matrix_generates_valid_tsx() {
    struct Case {
        name: &'static str,
        tree: Value,
        required: &'static [&'static str],
    }

    let cases = vec![
        Case {
            name: "layout_only",
            tree: serde_json::json!({
              "meta": { "id": "layout-only", "name": "LayoutOnly", "path": "/layout-only" },
              "root": {
                "id": "root",
                "meta": { "scope": true, "title": "LayoutOnlyPage" },
                "type": "Layout",
                "props": { "TEXT": "Root" }
              },
              "context": {}
            }),
            required: &["function LayoutOnlyPage", "<div>{\"Root\"}</div>"],
        },
        Case {
            name: "deep_inline_layout_and_text",
            tree: serde_json::json!({
              "meta": { "id": "deep-inline", "name": "DeepInline", "path": "/deep-inline" },
              "root": {
                "id": "root",
                "meta": { "scope": true, "title": "DeepInlinePage" },
                "type": "Layout",
                "props": { "TEXT": "Root" },
                "children": [
                  {
                    "id": "section",
                    "type": "Layout",
                    "props": { "TEXT": "Section" },
                    "children": [
                      {
                        "id": "leaf",
                        "type": "Text",
                        "props": { "TEXT": "Leaf", "DEFAULT_VALUE": 1 }
                      }
                    ]
                  }
                ]
              },
              "context": {}
            }),
            required: &[
                "function DeepInlinePage",
                "<div>{\"Leaf\"}</div>",
                "<div>{\"Section\"}</div>",
                "<div>{\"Root\"}</div>",
            ],
        },
        Case {
            name: "scoped_text_child",
            tree: serde_json::json!({
              "meta": { "id": "scoped-text", "name": "ScopedText", "path": "/scoped-text" },
              "root": {
                "id": "root",
                "meta": { "scope": true, "title": "ScopedTextPage" },
                "type": "Layout",
                "props": { "TEXT": "Root" },
                "children": [
                  {
                    "id": "card",
                    "meta": { "scope": true, "title": "CardText" },
                    "type": "Text",
                    "props": { "TEXT": "Scoped", "DEFAULT_VALUE": 2 }
                  }
                ]
              },
              "context": {}
            }),
            required: &[
                "function ScopedTextPage",
                "function CardText",
                "const [text, setText] = useState(2)",
                "<CardText />",
            ],
        },
        Case {
            name: "iframe_child",
            tree: serde_json::json!({
              "meta": { "id": "iframe-page", "name": "IframePage", "path": "/iframe" },
              "root": {
                "id": "root",
                "meta": { "scope": true, "title": "IframePage" },
                "type": "Layout",
                "props": { "TEXT": "Root" },
                "children": [
                  {
                    "id": "frame",
                    "type": "Iframe",
                    "props": { "FRAME_URL": "https://example.com/embed" }
                  }
                ]
              },
              "context": {}
            }),
            required: &[
                r#"import { BaseIframe } from "@frontend-forge/forge-components""#,
                r#"<BaseIframe src={"https://example.com/embed"} />"#,
            ],
        },
        Case {
            name: "rest_binding_to_text",
            tree: serde_json::json!({
              "meta": { "id": "rest-binding", "name": "RestBinding", "path": "/rest-binding" },
              "dataSources": [
                {
                  "id": "user",
                  "type": "rest",
                  "config": {
                    "URL": "/api/user",
                    "DEFAULT_VALUE": { "name": "Ada" }
                  }
                }
              ],
              "root": {
                "id": "root",
                "meta": { "scope": true, "title": "RestBindingPage" },
                "type": "Layout",
                "props": { "TEXT": "Root" },
                "children": [
                  {
                    "id": "name",
                    "type": "Text",
                    "props": {
                      "TEXT": {
                        "type": "binding",
                        "source": "user",
                        "path": "data.name",
                        "defaultValue": "Unknown"
                      },
                      "DEFAULT_VALUE": 0
                    }
                  }
                ]
              },
              "context": {}
            }),
            required: &[
                r#"import useSWR from "swr""#,
                r#"const { data: userData } = useUser();"#,
                r#"userData?.name ?? "Unknown""#,
            ],
        },
        Case {
            name: "cluster_crd_table",
            tree: crd_table_case("cluster-widgets", "crd-page-state"),
            required: &[
                "function WidgetTable",
                "const useStore = getCrdStore",
                "const { columns: columnsColumns } = useColumns();",
                "<PageTable",
            ],
        },
        Case {
            name: "workspace_crd_table",
            tree: crd_table_case("workspace-widgets", "workspace-crd-page-state"),
            required: &[
                "function WidgetTable",
                "useWorkspaceTableActions",
                "useWorkspaceProjectSelect",
                "<PageTable",
            ],
        },
    ];

    for case in cases {
        let code = generate_default_case(case.name, case.tree);
        for expected in case.required {
            assert_code_contains(case.name, &code, expected);
        }
    }
}

#[test]
fn full_example_manifest_components_trees_generate_valid_tsx() {
    let manifest: Value = serde_json::from_str(include_str!("../../../examples/full.json"))
        .expect("examples/full.json should be valid JSON");
    let pages = manifest
        .get("pages")
        .and_then(Value::as_array)
        .expect("examples/full.json should contain pages");
    assert!(!pages.is_empty(), "examples/full.json should contain pages");

    for page in pages {
        let case_name = page
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("<unknown-page>");
        let tree = page
            .get("componentsTree")
            .cloned()
            .unwrap_or_else(|| panic!("{case_name} should contain componentsTree"));
        let code = generate_default_case(case_name, tree);
        assert_code_contains(case_name, &code, "<PageTable");
        assert_code_contains(case_name, &code, "const useStore = getCrdStore");
    }
}

#[test]
fn custom_action_components_tree_generates_valid_tsx() {
    let mut registry = default_registry();
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
    registry.register_data_source(
        DataSourceSource::new(
            "custom-request",
            DataSourceSourceGenerateCode {
                stats: vec![StatSource {
                    id: "hookDecl",
                    scope: StatementScope::ModuleDecl,
                    code: r#"const %%HOOK_NAME%% = () => ({ data: null, error: null, isLoading: false, mutate: (effect) => typeof effect === "function" ? effect() : effect });"#,
                    output: vec!["HOOK_NAME"],
                    depends: vec![],
                }],
                meta: Some(NodeSourceMeta {
                    input_paths: [("hookDecl", vec!["HOOK_NAME"])].into_iter().collect(),
                    runtime_deps: Vec::new(),
                }),
                action_mode: DataSourceActionMode::Request,
                ..DataSourceSourceGenerateCode::default()
            },
        )
        .with_schema(DataSourceSourceSchema {
            template_inputs: Default::default(),
            outputs: [
                ("data", TemplateOutput { ty: "object" }),
                ("mutate", TemplateOutput { ty: "object" }),
            ]
            .into_iter()
            .collect(),
        }),
    );

    let tree = serde_json::json!({
      "meta": { "id": "action-page", "name": "ActionPage", "path": "/action" },
      "dataSources": [
        {
          "id": "custom",
          "type": "custom-request",
          "config": {
            "URL": "/api/custom",
            "METHOD": "POST"
          }
        }
      ],
      "root": {
        "id": "button-submit",
        "meta": { "scope": true, "title": "ActionPage" },
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
                { "type": "callDataSource", "id": "custom", "args": ["context.name"] }
              ]
            }
          }
        }
      ],
      "context": {}
    });

    let code = generate_with_registry("custom_action", registry, tree);
    assert_code_contains(
        "custom_action",
        &code,
        r#"dispatchActionFormGraph("SUBMIT""#,
    );
    assert_code_contains("custom_action", &code, r#""custom": "request""#);
    assert_code_contains("custom_action", &code, r#"const url = "/api/custom";"#);
    assert_code_contains("custom_action", &code, r#"<button onClick={(event) =>"#);
}

fn crd_table_case(id: &'static str, page_state_type: &'static str) -> Value {
    serde_json::json!({
      "meta": { "id": id, "name": id, "path": format!("/{id}") },
      "dataSources": [
        {
          "id": "columns",
          "type": "crd-columns",
          "config": {
            "COLUMNS_CONFIG": [
              {
                "key": "name",
                "title": "NAME",
                "render": { "type": "text", "path": "metadata.name" }
              }
            ]
          }
        },
        {
          "id": "pageState",
          "type": page_state_type,
          "args": [
            { "type": "binding", "source": "columns", "bind": "columns" }
          ],
          "config": {
            "PAGE_ID": id,
            "CRD_CONFIG": {
              "apiVersion": "v1",
              "group": "example.io",
              "kind": "Widget",
              "plural": "widgets"
            }
          }
        }
      ],
      "root": {
        "id": "table",
        "meta": { "scope": true, "title": "WidgetTable" },
        "type": "CrdTable",
        "props": {
          "TABLE_KEY": id,
          "TITLE": "Widgets",
          "AUTH_KEY": "widgets",
          "PARAMS": { "type": "binding", "source": "pageState", "bind": "params" },
          "CREATE_INITIAL_VALUE": {
            "apiVersion": "example.io/v1",
            "kind": "Widget",
            "metadata": { "name": "" },
            "spec": {}
          },
          "REFETCH": { "type": "binding", "source": "pageState", "bind": "refetch" },
          "TOOLBAR_LEFT": { "type": "binding", "source": "pageState", "bind": "toolbarLeft" },
          "PAGE_CONTEXT": { "type": "binding", "source": "pageState", "bind": "pageContext" },
          "COLUMNS": { "type": "binding", "source": "columns", "bind": "columns" },
          "DATA": { "type": "binding", "source": "pageState", "bind": "data" },
          "IS_LOADING": {
            "type": "binding",
            "source": "pageState",
            "bind": "loading",
            "defaultValue": false
          },
          "UPDATE": { "type": "binding", "source": "pageState", "bind": "update" },
          "DEL": { "type": "binding", "source": "pageState", "bind": "del" },
          "CREATE": { "type": "binding", "source": "pageState", "bind": "create" }
        }
      },
      "context": {}
    })
}
