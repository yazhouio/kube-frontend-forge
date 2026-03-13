import { StatementScope } from "@frontend-forge/forge-core/advanced";
import type { DataSourceDefinition } from "@frontend-forge/forge-core/advanced";

export const CrdColumnsDataSource: DataSourceDefinition = {
  id: "crd-columns",
  schema: {
    templateInputs: {
      COLUMNS_CONFIG: {
        type: "array",
        description: "Columns config",
      },
      HOOK_NAME: {
        type: "string",
        description: "Hook name",
      },
    },
    outputs: {
      columns: { type: "array" },
    },
  },
  generateCode: {
    imports: [
      'import * as React from "react"',
      'import { useMemo } from "react"',
      'import { TableTd, useRuntimeContext } from "@frontend-forge/forge-components"',
    ],
    stats: [
      {
        id: "columnsConfigDecl",
        scope: StatementScope.ModuleDecl,
        code: "const columnsConfig = %%COLUMNS_CONFIG%%;",
        output: ["columnsConfig"],
        depends: [],
      },
      {
        id: "hookDecl",
        scope: StatementScope.ModuleDecl,
        code: `const %%HOOK_NAME%% = () => {
  const runtime = useRuntimeContext();
  const cap = runtime?.capabilities || {};
  const t = cap.t ?? ((d) => d);
  const columns = useMemo(
    () =>
      columnsConfig.map((column) => {
        const { key, title, render, ...rest } = column;
        return {
          accessorKey: key,
          header: t(title),
          cell: (info) => (
            <TableTd meta={render} original={info.row.original} />
          ),
          ...rest,
        };
      }),
    [columnsConfig],
  );
  return { columns };
};`,
        output: ["HOOK_NAME"],
        depends: [],
      },
    ],
    meta: {
      inputPaths: {
        columnsConfigDecl: ["COLUMNS_CONFIG"],
        hookDecl: ["HOOK_NAME"],
      },
    },
  },
};

export const CrdPageStateDataSource: DataSourceDefinition = {
  id: "crd-page-state",
  schema: {
    templateInputs: {
      PAGE_ID: {
        type: "string",
        description: "Page id",
      },
      CRD_CONFIG: {
        type: "object",
        description: "CRD store config",
      },
      SCOPE: {
        type: "string",
        description: "Scope name",
      },
      HOOK_NAME: {
        type: "string",
        description: "Hook name",
      },
    },
    outputs: {
      params: { type: "object" },
      refetch: { type: "object" },
      toolbarLeft: { type: "object" },
      pageContext: { type: "object" },
      data: { type: "object" },
      loading: { type: "boolean" },
      update: { type: "object" },
      del: { type: "object" },
      create: { type: "object" },
    },
  },
  generateCode: {
    imports: [
      'import * as React from "react"',
      'import { useMemo } from "react"',
      'import { buildSearchObject, getCrdStore, usePageStore, useProjectSelect, useRuntimeContext } from "@frontend-forge/forge-components"',
    ],
    stats: [
      {
        id: "storeDecl",
        scope: StatementScope.ModuleDecl,
        code: "const useStore = getCrdStore(%%CRD_CONFIG%%);",
        output: ["useStore"],
        depends: [],
      },
      {
        id: "hookDecl",
        scope: StatementScope.ModuleDecl,
        code: `const %%HOOK_NAME%% = (columns, storeOptions = undefined) => {
  const pageId = %%PAGE_ID%%;
  const page = usePageStore({
    pageId,
    columns,
  });

  const runtime = useRuntimeContext();
  const params = runtime?.route?.params || {};
  const pageContext = runtime?.capabilities || {};
  const storeQuery = useMemo(() => buildSearchObject(page, true), [page]);

  const scope = %%SCOPE%%;
  const {
    render: renderProjectSelect,
    params: { namespace: selectNamespace },
  } = useProjectSelect(
    {
      cluster: params.cluster,
    },
    {
      enabled: scope === "namespace",
    },
  );
  const namespace = scope === "namespace" ? selectNamespace : undefined;
  const toolbarLeft = () => {
    if (scope === "namespace") {
      return renderProjectSelect();
    }
    return null;
  };

  const storeParams = { ...params, namespace };

  const store = useStore(
    {
      params: storeParams,
      query: storeQuery,
    },
    storeOptions,
  );

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
  };`,
        output: ["HOOK_NAME"],
        depends: [],
      },
    ],
    meta: {
      inputPaths: {
        storeDecl: ["CRD_CONFIG"],
        hookDecl: ["HOOK_NAME", "PAGE_ID", "SCOPE"],
      },
    },
  },
};

export const WorkspaceCrdPageStateDataSource: DataSourceDefinition = {
  id: "workspace-crd-page-state",
  schema: {
    templateInputs: {
      PAGE_ID: {
        type: "string",
        description: "Page id",
      },
      CRD_CONFIG: {
        type: "object",
        description: "CRD store config",
      },
      HOOK_NAME: {
        type: "string",
        description: "Hook name",
      },
    },
    outputs: {
      params: { type: "object" },
      refetch: { type: "object" },
      toolbarLeft: { type: "object" },
      pageContext: { type: "object" },
      data: { type: "object" },
      loading: { type: "boolean" },
      update: { type: "object" },
      del: { type: "object" },
      create: { type: "object" },
    },
  },
  generateCode: {
    imports: [
      'import * as React from "react"',
      'import { useMemo } from "react"',
      'import { buildSearchObject, getCrdStore, usePageStore, useRuntimeContext } from "@frontend-forge/forge-components"',
    ],
    stats: [
      {
        id: "storeDecl",
        scope: StatementScope.ModuleDecl,
        code: "const useStore = getCrdStore(%%CRD_CONFIG%%);",
        output: ["useStore"],
        depends: [],
      },
      {
        id: "hookDecl",
        scope: StatementScope.ModuleDecl,
        code: `const %%HOOK_NAME%% = (columns, storeOptions = undefined) => {
  const pageId = %%PAGE_ID%%;
  const page = usePageStore({
    pageId,
    columns,
  });

  const runtime = useRuntimeContext();
  const params = runtime?.route?.params || {};
  const pageContext = runtime?.capabilities || {};
  pageContext.useTableActions = pageContext.useWorkspaceTableActions;

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
};`,
        output: ["HOOK_NAME"],
        depends: [],
      },
    ],
    meta: {
      inputPaths: {
        storeDecl: ["CRD_CONFIG"],
        hookDecl: ["HOOK_NAME", "PAGE_ID"],
      },
    },
  },
};
