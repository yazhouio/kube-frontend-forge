import {
  CRDTable404Fallback,
  PageTable,
  TableTd,
  useRuntimeContext,
  buildSearchObject,
  getCrdStore,
  usePageStore,
  useProjectSelect,
} from "@frontend-forge/forge-components";
import * as React from "react";
import { useMemo } from "react";
const columnsConfig = [
  {
    key: "name",
    title: "名称",
    render: {
      type: "text",
      path: "metadata.name",
      payload: {},
    },
  },
  {
    key: "namespace",
    title: "PROJECT_PL",
    render: {
      type: "text",
      path: "metadata.namespace",
      payload: {},
    },
  },
  {
    key: "created",
    title: "创建时间",
    render: {
      type: "time",
      path: "metadata.creationTimestamp",
      payload: {
        format: "local-datetime",
      },
    },
  },
];
const useCrdColumns = () => {
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
  return {
    columns,
  };
};
const CRD_CONFIG = {
  apiVersion: "v1alpha1",
  plural: "frontendintegrations",
  group: "frontend-forge.kubesphere.io",
  kapi: true,
};
const EMPTY_COMMAND = `kubectl label crd ${CRD_CONFIG.plural}.${CRD_CONFIG.group} kubesphere.io/resource-served=true`;
const tableFallbacks = [
  {
    ...CRDTable404Fallback,
    props: {
      ...(CRDTable404Fallback.props || {}),
      command: EMPTY_COMMAND,
    },
  },
];
const useStore = getCrdStore(CRD_CONFIG);
const useCrdPageState = (columns, storeOptions = undefined) => {
  const pageId = "servicemonitors1-cluster";
  const page = usePageStore({
    pageId,
    columns,
  });
  const runtime = useRuntimeContext();
  const params = runtime?.route?.params || {};
  const pageContext = runtime?.capabilities || {};
  const storeQuery = useMemo(() => buildSearchObject(page, true), [page]);
  const scope = "namespace";
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
  const storeParams = {
    ...params,
    namespace,
  };
  const resolvedOptions =
    storeOptions &&
    Object.prototype.hasOwnProperty.call(storeOptions, "enabled")
      ? storeOptions
      : {
          ...(storeOptions || {}),
          enabled: scope !== "namespace" || Boolean(namespace),
        };
  const store = useStore(
    {
      params: storeParams,
      query: storeQuery,
    },
    resolvedOptions,
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
};
function CrdTable(props) {
  const title = "servicemonitors1";
  const { columns: columnsColumns } = useCrdColumns();
  const {
    params: pageStateParams,
    refetch: pageStateRefetch,
    toolbarLeft: pageStateToolbarLeft,
    pageContext: pageStatePageContext,
    data: pageStateData,
    loading: pageStateLoading,
    update: pageStateUpdate,
    del: pageStateDel,
    create: pageStateCreate,
  } = useCrdPageState(columnsColumns);
  return (
    <PageTable
      tableKey={"servicemonitors1-cluster"}
      title={title}
      // authKey={"servicemonitors"}
      params={pageStateParams}
      refetch={pageStateRefetch}
      toolbarLeft={pageStateToolbarLeft}
      pageContext={pageStatePageContext}
      columns={columnsColumns}
      data={pageStateData}
      isLoading={pageStateLoading ?? false}
      fallbacks={tableFallbacks}
      update={pageStateUpdate}
      del={pageStateDel}
      create={pageStateCreate}
    />
  );
}
export default CrdTable;
