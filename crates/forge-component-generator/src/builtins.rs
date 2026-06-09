use crate::registry::{
    DataSourceSource, DataSourceSourceGenerateCode, DataSourceSourceSchema, NodeSource,
    NodeSourceGenerateCode, NodeSourceMeta, NodeSourceSchema, Registry, StatSource, StatementScope,
    TemplateDefault, TemplateInput, TemplateOutput,
};

pub fn default_registry() -> Registry {
    let mut registry = Registry::default();
    registry.register_node(layout_node());
    registry.register_node(text_node());
    registry.register_node(iframe_node());
    registry.register_node(crd_table_node());
    registry.register_data_source(static_data_source());
    registry.register_data_source(rest_data_source());
    registry.register_data_source(crd_columns_data_source());
    registry.register_data_source(crd_page_state_data_source());
    registry.register_data_source(workspace_crd_page_state_data_source());
    registry
}

fn layout_node() -> NodeSource {
    NodeSource::new(
        "Layout",
        NodeSourceGenerateCode {
            imports: vec![r#"import * as React from "react""#],
            jsx: Some(
                r#"<div className='layout'><__ENGINE_CHILDREN__ />
    <div>{%%TEXT%%}</div></div>"#,
            ),
            stats: vec![],
            meta: Some(meta_input_paths(vec![("$jsx", vec!["TEXT"])])),
        },
    )
    .with_schema(schema(vec![(
        "TEXT",
        TemplateInput {
            ty: "string",
            description: "Layout text",
        },
    )]))
}

fn text_node() -> NodeSource {
    NodeSource::new(
        "Text",
        NodeSourceGenerateCode {
            imports: vec![
                r#"import * as React from "react""#,
                r#"import { useState } from "react""#,
            ],
            jsx: Some(r#"<div>{%%TEXT%%}</div>"#),
            stats: vec![StatSource {
                id: "textState",
                scope: StatementScope::FunctionBody,
                code: "const [text, setText] = useState(%%DEFAULT_VALUE%%);",
                output: vec!["text", "setText"],
                depends: vec![],
            }],
            meta: Some(meta_input_paths(vec![
                ("$jsx", vec!["TEXT"]),
                ("textState", vec!["DEFAULT_VALUE"]),
            ])),
        },
    )
    .with_schema(schema(vec![
        (
            "TEXT",
            TemplateInput {
                ty: "string",
                description: "Text content",
            },
        ),
        (
            "DEFAULT_VALUE",
            TemplateInput {
                ty: "number",
                description: "Default value",
            },
        ),
    ]))
}

fn iframe_node() -> NodeSource {
    NodeSource::new(
        "Iframe",
        NodeSourceGenerateCode {
            imports: vec![
                r#"import * as React from "react""#,
                r#"import { BaseIframe } from "@frontend-forge/forge-components""#,
            ],
            jsx: Some(r#"<BaseIframe src={%%FRAME_URL%%} />"#),
            stats: vec![],
            meta: Some(meta_input_paths(vec![("$jsx", vec!["FRAME_URL"])])),
        },
    )
    .with_schema(schema(vec![(
        "FRAME_URL",
        TemplateInput {
            ty: "string",
            description: "Iframe src url",
        },
    )]))
}

fn crd_table_node() -> NodeSource {
    NodeSource::new(
        "CrdTable",
        NodeSourceGenerateCode {
            imports: vec![
                r#"import * as React from "react""#,
                r#"import { CRDTable404Fallback, PageTable } from "@frontend-forge/forge-components""#,
            ],
            jsx: Some(
                r#"<PageTable
  tableKey={%%TABLE_KEY%%}
  title={t(%%TITLE%%)}
  authKey={%%AUTH_KEY%%}
  params={%%PARAMS%%}
  createInitialValue={%%CREATE_INITIAL_VALUE%%}
  refetch={%%REFETCH%%}
  toolbarLeft={%%TOOLBAR_LEFT%%}
  pageContext={%%PAGE_CONTEXT%%}
  columns={%%COLUMNS%%}
  data={%%DATA%%}
  isLoading={%%IS_LOADING%%}
  fallbacks={[
    {
      ...CRDTable404Fallback,
      props: {
        ...(CRDTable404Fallback.props || {}),
        command:
          typeof __crdTableFallbackCommand === "string"
            ? __crdTableFallbackCommand
            : undefined,
        ...(%%NOT_FOUND_EMPTY_PROPS%% || {}),
      },
    },
  ]}
  update={%%UPDATE%%}
  del={%%DEL%%}
  create={%%CREATE%%}
/>"#,
            ),
            stats: vec![],
            meta: Some(meta_input_paths(vec![(
                "$jsx",
                vec![
                    "TABLE_KEY",
                    "TITLE",
                    "AUTH_KEY",
                    "PARAMS",
                    "REFETCH",
                    "TOOLBAR_LEFT",
                    "PAGE_CONTEXT",
                    "COLUMNS",
                    "DATA",
                    "NOT_FOUND_EMPTY_PROPS",
                    "IS_LOADING",
                    "UPDATE",
                    "DEL",
                    "CREATE",
                    "CREATE_INITIAL_VALUE",
                ],
            )])),
        },
    )
}

fn schema(inputs: Vec<(&'static str, TemplateInput)>) -> NodeSourceSchema {
    NodeSourceSchema {
        template_inputs: inputs.into_iter().collect(),
        runtime_props: Default::default(),
    }
}

fn meta_input_paths(items: Vec<(&'static str, Vec<&'static str>)>) -> NodeSourceMeta {
    NodeSourceMeta {
        input_paths: items.into_iter().collect(),
        runtime_deps: Vec::new(),
    }
}

fn data_source_schema(
    inputs: Vec<(&'static str, TemplateInput)>,
    outputs: Vec<(&'static str, TemplateOutput)>,
) -> DataSourceSourceSchema {
    DataSourceSourceSchema {
        template_inputs: inputs.into_iter().collect(),
        outputs: outputs.into_iter().collect(),
    }
}

fn standard_data_source_outputs() -> Vec<(&'static str, TemplateOutput)> {
    vec![
        ("data", TemplateOutput { ty: "object" }),
        ("error", TemplateOutput { ty: "object" }),
        ("isLoading", TemplateOutput { ty: "boolean" }),
        ("mutate", TemplateOutput { ty: "object" }),
    ]
}

fn static_data_source() -> DataSourceSource {
    DataSourceSource::new(
        "static",
        DataSourceSourceGenerateCode {
            imports: vec![r#"import { useState } from "react""#],
            stats: vec![StatSource {
                id: "hookDecl",
                scope: StatementScope::ModuleDecl,
                code: r#"const %%HOOK_NAME%% = () => {
  const [data, setData] = useState(%%DATA%%);
  return { data, error: null, isLoading: false, mutate: setData };
};"#,
                output: vec!["HOOK_NAME"],
                depends: vec![],
            }],
            meta: Some(meta_input_paths(vec![(
                "hookDecl",
                vec!["DATA", "HOOK_NAME"],
            )])),
            defaults: [("DATA", TemplateDefault::Expr("null"))]
                .into_iter()
                .collect(),
            ..DataSourceSourceGenerateCode::default()
        },
    )
    .with_schema(data_source_schema(
        vec![
            (
                "DATA",
                TemplateInput {
                    ty: "object",
                    description: "Static payload",
                },
            ),
            (
                "HOOK_NAME",
                TemplateInput {
                    ty: "string",
                    description: "Hook name",
                },
            ),
        ],
        standard_data_source_outputs(),
    ))
}

fn rest_data_source() -> DataSourceSource {
    DataSourceSource::new(
        "rest",
        DataSourceSourceGenerateCode {
            imports: vec![r#"import useSWR from "swr""#],
            stats: vec![
                StatSource {
                    id: "fetcherDecl",
                    scope: StatementScope::ModuleDecl,
                    code: "const %%FETCHER_NAME%% = (url) => fetch(url).then((res) => res.json());",
                    output: vec!["FETCHER_NAME"],
                    depends: vec![],
                },
                StatSource {
                    id: "hookDecl",
                    scope: StatementScope::ModuleDecl,
                    code: r#"const %%HOOK_NAME%% = (options = {}) =>
  useSWR(
    %%AUTO_LOAD%% ? %%URL%% : null,
    %%FETCHER_NAME%%,
    { fallbackData: %%DEFAULT_VALUE%%, ...options }
  );"#,
                    output: vec!["HOOK_NAME"],
                    depends: vec!["fetcherDecl"],
                },
            ],
            meta: Some(meta_input_paths(vec![
                ("fetcherDecl", vec!["FETCHER_NAME"]),
                (
                    "hookDecl",
                    vec![
                        "AUTO_LOAD",
                        "URL",
                        "DEFAULT_VALUE",
                        "HOOK_NAME",
                        "FETCHER_NAME",
                    ],
                ),
            ])),
            defaults: [
                ("AUTO_LOAD", TemplateDefault::Expr("true")),
                ("DEFAULT_VALUE", TemplateDefault::Expr("null")),
                ("URL", TemplateDefault::Expr("undefined")),
            ]
            .into_iter()
            .collect(),
            ..DataSourceSourceGenerateCode::default()
        },
    )
    .with_schema(data_source_schema(
        vec![
            (
                "URL",
                TemplateInput {
                    ty: "string",
                    description: "Request URL",
                },
            ),
            (
                "METHOD",
                TemplateInput {
                    ty: "string",
                    description: "Request method",
                },
            ),
            (
                "HEADERS",
                TemplateInput {
                    ty: "object",
                    description: "Request headers",
                },
            ),
            (
                "DEFAULT_VALUE",
                TemplateInput {
                    ty: "object",
                    description: "Default response value",
                },
            ),
            (
                "AUTO_LOAD",
                TemplateInput {
                    ty: "boolean",
                    description: "Auto load on mount",
                },
            ),
            (
                "HOOK_NAME",
                TemplateInput {
                    ty: "string",
                    description: "Hook name",
                },
            ),
            (
                "FETCHER_NAME",
                TemplateInput {
                    ty: "string",
                    description: "Fetcher name",
                },
            ),
        ],
        standard_data_source_outputs(),
    ))
}

fn crd_columns_data_source() -> DataSourceSource {
    DataSourceSource::new(
        "crd-columns",
        DataSourceSourceGenerateCode {
            imports: vec![
                r#"import { useMemo } from "react""#,
                r#"import { TableTd, useRuntimeContext } from "@frontend-forge/forge-components""#,
            ],
            stats: vec![
                StatSource {
                    id: "columnsConfigDecl",
                    scope: StatementScope::ModuleDecl,
                    code: "const columnsConfig = %%COLUMNS_CONFIG%%;",
                    output: vec!["columnsConfig"],
                    depends: vec![],
                },
                StatSource {
                    id: "hookDecl",
                    scope: StatementScope::ModuleDecl,
                    code: r#"const %%HOOK_NAME%% = () => {
  const runtime = useRuntimeContext();
  const cap = runtime?.capabilities || {};
  const t = cap.t ?? ((d) => d);
  const columns = useMemo(
    () =>
      columnsConfig.map((column) => {
        const { key, title, render, path, valueType, displayType, payload, emptyText, ...rest } = column;
        const renderConfig = render ?? { path, valueType, displayType, payload, emptyText };
        return {
          accessorKey: key,
          header: t(title),
          cell: (info) => <TableTd meta={renderConfig} original={info.row.original} />,
          ...rest,
        };
      }),
    [columnsConfig],
  );
  return { columns };
};"#,
                    output: vec!["HOOK_NAME"],
                    depends: vec![],
                },
            ],
            meta: Some(meta_input_paths(vec![
                ("columnsConfigDecl", vec!["COLUMNS_CONFIG"]),
                ("hookDecl", vec!["HOOK_NAME"]),
            ])),
            ..DataSourceSourceGenerateCode::default()
        },
    )
    .with_schema(data_source_schema(
        vec![
            (
                "COLUMNS_CONFIG",
                TemplateInput {
                    ty: "array",
                    description: "Columns config",
                },
            ),
            (
                "HOOK_NAME",
                TemplateInput {
                    ty: "string",
                    description: "Hook name",
                },
            ),
        ],
        vec![("columns", TemplateOutput { ty: "array" })],
    ))
}

fn crd_page_state_data_source() -> DataSourceSource {
    crd_page_state_data_source_with_hook(
        "crd-page-state",
        r#"const %%HOOK_NAME%% = (columns, storeOptions = undefined) => {
  const pageId = %%PAGE_ID%%;
  const page = usePageStore({ pageId, columns });
  const runtime = useRuntimeContext();
  const params = runtime?.route?.params || {};
  const pageContext = runtime?.capabilities || {};
  const storeQuery = useMemo(() => buildSearchObject(page, true), [page]);
  const scope = %%SCOPE%%;
  const { render: renderProjectSelect, params: { namespace: selectNamespace } } = useProjectSelect(
    { cluster: params.cluster },
    { enabled: scope === "namespace" },
  );
  const namespace = scope === "namespace" ? selectNamespace : undefined;
  const toolbarLeft = () => scope === "namespace" ? renderProjectSelect() : null;
  const store = useStore({ params: { ...params, namespace }, query: storeQuery }, storeOptions ?? {});
  return {
    params,
    toolbarLeft,
    pageContext,
    data: store.data,
    loading: Boolean(store.isLoading || store.isValidating),
    refetch: store.mutate,
    update: store.update,
    del: store.batchDelete,
    create: store.create,
  };
};"#,
    )
}

fn workspace_crd_page_state_data_source() -> DataSourceSource {
    crd_page_state_data_source_with_hook(
        "workspace-crd-page-state",
        r#"const %%HOOK_NAME%% = (columns, storeOptions = undefined) => {
  const pageId = %%PAGE_ID%%;
  const page = usePageStore({ pageId, columns });
  const runtime = useRuntimeContext();
  const params = runtime?.route?.params || {};
  const pageContext = {...runtime?.capabilities, useTableActions: runtime?.capabilities?.useWorkspaceTableActions};
  const storeQuery = useMemo(() => buildSearchObject(page, true), [page]);

  const useWorkspaceProjectSelectHook = useMemo(
    () =>
      pageContext?.useWorkspaceProjectSelect ||
      (() => ({ render: null, params: {} })),
    [pageContext],
  );
  const {
    render: renderProjectSelect,
    params: { cluster, namespace },
  } = useWorkspaceProjectSelectHook({
    workspace: params.workspace,
    showAll: false,
  });

  const resolvedOptions =
    storeOptions && Object.prototype.hasOwnProperty.call(storeOptions, "enabled")
      ? storeOptions
      : {
          ...(storeOptions || {}),
          enabled: Boolean(namespace),
        };

  const store = useStore(
    {
      params: { ...params, namespace, cluster },
      query: storeQuery,
    },
    resolvedOptions,
  );

  return {
    params: { ...params, namespace, cluster },
    toolbarLeft: renderProjectSelect,
    pageContext,
    data: store.data,
    loading: Boolean(store.isLoading || store.isValidating),
    refetch: store.mutate,
    update: store.update,
    del: store.batchDelete,
    create: store.create,
  };
};"#,
    )
}

fn crd_page_state_data_source_with_hook(
    id: &'static str,
    hook_code: &'static str,
) -> DataSourceSource {
    DataSourceSource::new(
        id,
        DataSourceSourceGenerateCode {
            imports: vec![
                r#"import { useMemo } from "react""#,
                r#"import { buildSearchObject, getCrdStore, usePageStore, useProjectSelect, useRuntimeContext } from "@frontend-forge/forge-components""#,
            ],
            stats: vec![
                StatSource {
                    id: "storeDecl",
                    scope: StatementScope::ModuleDecl,
                    code: "const useStore = getCrdStore(%%CRD_CONFIG%%);",
                    output: vec!["useStore"],
                    depends: vec![],
                },
                StatSource {
                    id: "fallbackCommandDecl",
                    scope: StatementScope::ModuleDecl,
                    code: r#"const __crdTableFallbackCommand = "kubectl label crd " + (%%CRD_CONFIG%%).plural + "." + (%%CRD_CONFIG%%).group + " kubesphere.io/resource-served=true";"#,
                    output: vec!["__crdTableFallbackCommand"],
                    depends: vec!["storeDecl"],
                },
                StatSource {
                    id: "hookDecl",
                    scope: StatementScope::ModuleDecl,
                    code: hook_code,
                    output: vec!["HOOK_NAME"],
                    depends: vec!["storeDecl"],
                },
            ],
            meta: Some(meta_input_paths(vec![
                ("storeDecl", vec!["CRD_CONFIG"]),
                ("fallbackCommandDecl", vec!["CRD_CONFIG"]),
                ("hookDecl", vec!["HOOK_NAME", "PAGE_ID", "SCOPE"]),
            ])),
            defaults: [
                ("CRD_CONFIG", TemplateDefault::Expr("{}")),
                ("PAGE_ID", TemplateDefault::DataSourceIdJson),
                ("SCOPE", TemplateDefault::Expr(r#""cluster""#)),
            ]
            .into_iter()
            .collect(),
            ..DataSourceSourceGenerateCode::default()
        },
    )
    .with_schema(data_source_schema(
        vec![
            (
                "PAGE_ID",
                TemplateInput {
                    ty: "string",
                    description: "Page id",
                },
            ),
            (
                "CRD_CONFIG",
                TemplateInput {
                    ty: "object",
                    description: "CRD store config",
                },
            ),
            (
                "SCOPE",
                TemplateInput {
                    ty: "string",
                    description: "Scope name",
                },
            ),
            (
                "HOOK_NAME",
                TemplateInput {
                    ty: "string",
                    description: "Hook name",
                },
            ),
        ],
        crd_page_state_outputs(),
    ))
}

fn crd_page_state_outputs() -> Vec<(&'static str, TemplateOutput)> {
    vec![
        ("params", TemplateOutput { ty: "object" }),
        ("refetch", TemplateOutput { ty: "object" }),
        ("toolbarLeft", TemplateOutput { ty: "object" }),
        ("pageContext", TemplateOutput { ty: "object" }),
        ("data", TemplateOutput { ty: "object" }),
        ("loading", TemplateOutput { ty: "boolean" }),
        ("update", TemplateOutput { ty: "object" }),
        ("del", TemplateOutput { ty: "object" }),
        ("create", TemplateOutput { ty: "object" }),
    ]
}
