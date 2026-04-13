import { ExtensionManifest, ForgeCore } from "@frontend-forge/forge-core";
import {
  CodeExporter,
  ComponentGenerator,
  ProjectGenerator,
} from "@frontend-forge/forge-core/advanced";
import PQueue from "p-queue";
import path from "path";
import { fileURLToPath } from "url";
import { getCache, setCache } from "../../cache.js";
import {
  BUILD_TIMEOUT_MS,
  CHILD_MAX_OLD_SPACE_MB,
  CONCURRENCY,
  DEFAULT_EXTERNALS,
} from "../../config.js";
import {
  CrdColumnsDataSource,
  CrdPageStateDataSource,
  WorkspaceCrdPageStateDataSource,
} from "../dataSourceDef.js";
import { CrdTableNode, IframeNode } from "../nodeDef.js";

const queue = new PQueue({ concurrency: CONCURRENCY });
const here = path.dirname(fileURLToPath(import.meta.url));
const vendorNodeModules = path.resolve(here, "..", "..", "vendor", "node_modules");

const exporter = new CodeExporter({
  cache: {
    get: (key) => getCache(key),
    set: (key, value) => setCache(key, value),
  },
  schedule: (fn) => queue.add(() => fn()),
  buildTimeoutMs: BUILD_TIMEOUT_MS,
  childMaxOldSpaceMb: CHILD_MAX_OLD_SPACE_MB,
  defaultExternals: DEFAULT_EXTERNALS,
  defaultEntry: "src/index.tsx",
  vendorNodeModules,
});

const componentGenerator = new ComponentGenerator();
componentGenerator.registerNode(IframeNode);
componentGenerator.registerNode(CrdTableNode);
componentGenerator.registerDataSource(CrdColumnsDataSource);
componentGenerator.registerDataSource(CrdPageStateDataSource);
componentGenerator.registerDataSource(WorkspaceCrdPageStateDataSource);

const forge = new ForgeCore({
  componentGenerator,
  projectGenerator: new ProjectGenerator(),
  codeExporter: exporter,
});

const manifest: ExtensionManifest = {
  version: "1.0",
  name: "servicemonitors1",
  displayName: "servicemonitors1",
  routes: [
    {
      path: "/clusters/:cluster/frontendintegrations/servicemonitors1/asdfas",
      pageId: "servicemonitors1-cluster",
    },
    {
      path: "/workspaces/:workspace/frontendintegrations/servicemonitors1/asdfas",
      pageId: "servicemonitors1-workspace",
    },
  ],
  menus: [
    {
      parent: "cluster",
      name: "frontendintegrations/servicemonitors1/asdfas",
      title: "servicemonitors1",
      icon: "GridDuotone",
      order: 999,
    },
    {
      parent: "workspace",
      name: "frontendintegrations/servicemonitors1/asdfas",
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
                  title: "Title",
                  render: { type: "text", path: "metadata.name", payload: {} },
                  enableSorting: true,
                },
                {
                  key: "namespace",
                  title: "PROJECT_PL",
                  render: {
                    type: "text",
                    path: "metadata.namespace",
                    payload: {},
                  },
                  enableHiding: true,
                },
                {
                  key: "created",
                  title: "CREATION_TIME",
                  render: {
                    type: "time",
                    path: "metadata.creationTimestamp",
                    payload: { format: "local-datetime" },
                  },
                  enableSorting: true,
                  enableHiding: true,
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
                kind: "ServiceMonitor",
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
            CREATE_INITIAL_VALUE: {
              apiVersion: "monitoring.coreos.com/v1",
              kind: "ServiceMonitor",
            },
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
                  title: "Title",
                  render: { type: "text", path: "metadata.name", payload: {} },
                  enableSorting: true,
                },
                {
                  key: "namespace",
                  title: "PROJECT_PL",
                  render: {
                    type: "text",
                    path: "metadata.namespace",
                    payload: {},
                  },
                  enableHiding: true,
                },
                {
                  key: "created",
                  title: "CREATION_TIME",
                  render: {
                    type: "time",
                    path: "metadata.creationTimestamp",
                    payload: { format: "local-datetime" },
                  },
                  enableSorting: true,
                  enableHiding: true,
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
                kind: "ServiceMonitor",
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
            CREATE_INITIAL_VALUE: {
              apiVersion: "monitoring.coreos.com/v1",
              kind: "ServiceMonitor",
            },
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
const result2 = await forge.buildProject(manifest);
const demoDir = path.resolve(process.cwd(), "demo");
forge.emitToFileSystem(result2, demoDir);
console.log(`Emitted ${result2.length} files to ${demoDir}`);
