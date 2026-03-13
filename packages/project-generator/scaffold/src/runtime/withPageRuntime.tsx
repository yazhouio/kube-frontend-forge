import * as React from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";
import {
  RuntimeProvider,
  type RuntimeContextInfo,
} from "@frontend-forge/forge-components";
import {
  getActions,
  getLocalTime,
  useBatchActions,
  useItemActions,
  useTableActions,
  useWorkspaceProjectSelect,
  buildCreateActionGuard,
} from "@ks-console/shared";
import { RuntimePageInfo } from "./types";
import { usePageRuntimeRouter } from "./routerHook";

const pageContext = {
  useWorkspaceTableActions: (config) =>
    useTableActions({
      ...config,
      actions: config.actions.map((item) => {
        return {
          ...item,
          ...buildCreateActionGuard({
            params: config.params,
          }),
          action: item.action,
        };
      }),
    }),
  useTableActions: useTableActions,
  useBatchActions: useBatchActions,
  useItemActions: useItemActions,
  getActions: getActions,
  getLocalTime: getLocalTime,
  useWorkspaceProjectSelect,
  t: window.t ?? ((d) => d),
};

export function withPageRuntime<P>(
  Page: React.ComponentType<P>,
  page: RuntimePageInfo,
): React.FC<P> {
  return function RuntimeWrappedPage(props: P) {
    const route = usePageRuntimeRouter();

    const runtime = React.useMemo<RuntimeContextInfo>(
      () => ({
        ...route,
        page: {
          id: page.id,
        },
        capabilities: pageContext,
      }),
      [route, page],
    );

    return (
      <RuntimeProvider value={runtime}>
        <Page {...props} />
      </RuntimeProvider>
    );
  };
}
