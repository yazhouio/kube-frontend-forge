import { defineWorkspaceTableScene } from "../src/preview/defineWorkspaceTableScene.js";
import { WorkspaceTableScene } from "./workspaceTableScene.js";

// export const workspaceTablePageConfig =
//   defineWorkspaceTableScene(WorkspaceTableScene);

const config = {
  version: "1.0",
  name: "servicemonitors1",
  displayName: "servicemonitors1",
  routes: [
    {
      path: "/clusters/:cluster/frontendintegrations/asdfas",
      pageId: "servicemonitors1-cluster",
    },
    {
      path: "/workspaces/:workspace/frontendintegrations/asdfas",
      pageId: "servicemonitors1-workspace",
    },
  ],
  menus: [
    {
      parent: "cluster",
      name: "servicemonitors1",
      title: "servicemonitors1",
      icon: "GridDuotone",
      order: 999,
    },
    {
      parent: "workspace",
      name: "servicemonitors1",
      title: "servicemonitors1",
      icon: "GridDuotone",
      order: 999,
    },
  ],
  locales: [],
  pages: [
    {
      id: "servicemonitors1-cluster",
      entryComponent: "servicemonitors1-cluster",
      componentsTree: {
        meta: {
          id: "servicemonitors1-cluster",
          name: "servicemonitors1-cluster",
          title: "servicemonitors1",
          path: "/servicemonitors1-cluster",
        },
        context: {},
        dataSources: [
          {
            id: "columns",
            type: "crd-columns",
            config: {
              COLUMNS_CONFIG: [
                {
                  key: "name",
                  title: "名称",
                  render: { type: "text", path: "metadata.name", payload: {} },
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
                    payload: { format: "local-datetime" },
                  },
                },
              ],
              HOOK_NAME: "useCrdColumns",
            },
          },
          {
            id: "pageState",
            type: "crd-page-state",
            args: [{ type: "binding", source: "columns", bind: "columns" }],
            config: {
              PAGE_ID: "servicemonitors1-cluster",
              CRD_CONFIG: {
                apiVersion: "v1",
                plural: "servicemonitors",
                group: "monitoring.coreos.com",
                kapi: true,
              },
              SCOPE: "namespace",
              HOOK_NAME: "useCrdPageState",
            },
          },
        ],
        root: {
          id: "servicemonitors1-cluster-root",
          type: "CrdTable",
          props: {
            TABLE_KEY: "servicemonitors1-cluster",
            TITLE: "servicemonitors1",
            PARAMS: { type: "binding", source: "pageState", bind: "params" },
            REFETCH: { type: "binding", source: "pageState", bind: "refetch" },
            TOOLBAR_LEFT: {
              type: "binding",
              source: "pageState",
              bind: "toolbarLeft",
            },
            PAGE_CONTEXT: {
              type: "binding",
              source: "pageState",
              bind: "pageContext",
            },
            COLUMNS: { type: "binding", source: "columns", bind: "columns" },
            DATA: { type: "binding", source: "pageState", bind: "data" },
            NOT_FOUND_EMPTY_PROPS: {
              titleI18nKey: "PROJECT_GENERATOR.CRD_NOT_FOUND_TITLE",
              descriptionI18nKey:
                "PROJECT_GENERATOR.CRD_NOT_FOUND_DESCRIPTION",
              command:
                "kubectl label crd servicemonitors.monitoring.coreos.com kubesphere.io/resource-served=true",
            },
            IS_LOADING: {
              type: "binding",
              source: "pageState",
              bind: "loading",
              defaultValue: false,
            },
            UPDATE: { type: "binding", source: "pageState", bind: "update" },
            DEL: { type: "binding", source: "pageState", bind: "del" },
            CREATE: { type: "binding", source: "pageState", bind: "create" },
          },
          meta: { title: "CrdTable", scope: true },
        },
      },
    },
    {
      id: "servicemonitors1-workspace",
      entryComponent: "servicemonitors1-workspace",
      componentsTree: {
        meta: {
          id: "servicemonitors1-workspace",
          name: "servicemonitors1-workspace",
          title: "servicemonitors1",
          path: "/servicemonitors1-workspace",
        },
        context: {},
        dataSources: [
          {
            id: "columns",
            type: "crd-columns",
            config: {
              COLUMNS_CONFIG: [
                {
                  key: "name",
                  title: "名称",
                  render: { type: "text", path: "metadata.name", payload: {} },
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
                    payload: { format: "local-datetime" },
                  },
                },
              ],
              HOOK_NAME: "useCrdColumns",
            },
          },
          {
            id: "pageState",
            type: "workspace-crd-page-state",
            args: [{ type: "binding", source: "columns", bind: "columns" }],
            config: {
              PAGE_ID: "servicemonitors1-workspace",
              CRD_CONFIG: {
                apiVersion: "v1",
                plural: "servicemonitors",
                group: "monitoring.coreos.com",
                kapi: true,
              },
              HOOK_NAME: "useCrdPageState",
            },
          },
        ],
        root: {
          id: "servicemonitors1-workspace-root",
          type: "CrdTable",
          props: {
            TABLE_KEY: "servicemonitors1-workspace",
            TITLE: "servicemonitors1",
            PARAMS: { type: "binding", source: "pageState", bind: "params" },
            REFETCH: { type: "binding", source: "pageState", bind: "refetch" },
            TOOLBAR_LEFT: {
              type: "binding",
              source: "pageState",
              bind: "toolbarLeft",
            },
            PAGE_CONTEXT: {
              type: "binding",
              source: "pageState",
              bind: "pageContext",
            },
            COLUMNS: { type: "binding", source: "columns", bind: "columns" },
            DATA: { type: "binding", source: "pageState", bind: "data" },
            NOT_FOUND_EMPTY_PROPS: {
              titleI18nKey: "PROJECT_GENERATOR.CRD_NOT_FOUND_TITLE",
              descriptionI18nKey:
                "PROJECT_GENERATOR.CRD_NOT_FOUND_DESCRIPTION",
              command:
                "kubectl label crd servicemonitors.monitoring.coreos.com kubesphere.io/resource-served=true",
            },
            IS_LOADING: {
              type: "binding",
              source: "pageState",
              bind: "loading",
              defaultValue: false,
            },
            UPDATE: { type: "binding", source: "pageState", bind: "update" },
            DEL: { type: "binding", source: "pageState", bind: "del" },
            CREATE: { type: "binding", source: "pageState", bind: "create" },
          },
          meta: { title: "CrdTable", scope: true },
        },
      },
    },
  ],
  build: {
    target: "kubesphere-extension",
    moduleName: "servicemonitors1",
    systemjs: true,
  },
};

export const workspaceTablePageConfig = config.pages[1].componentsTree;
