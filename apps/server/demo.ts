import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { ExtensionManifest, VirtualFile } from "@frontend-forge/forge-core";
import { ForgeCore } from "@frontend-forge/forge-core";
import {
  CodeExporter,
  ComponentGenerator,
  ProjectGenerator,
} from "@frontend-forge/forge-core/advanced";
import {
  BUILD_TIMEOUT_MS,
  CHILD_MAX_OLD_SPACE_MB,
  DEFAULT_EXTERNALS,
} from "./src/config.ts";
import { CrdTableNode, IframeNode } from "./src/preview/nodeDef.ts";
import {
  CrdColumnsDataSource,
  CrdPageStateDataSource,
  WorkspaceCrdPageStateDataSource,
} from "./src/preview/dataSourceDef.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const vendorNodeModules = path.resolve(here, "vendor", "node_modules");

const exporter = new CodeExporter({
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
  build: {
    moduleName: "qweqwe",
    systemjs: true,
    target: "kubesphere-extension",
  },
  displayName: "qweqwe",
  locales: [],
  menus: [
    {
      icon: "BoxDuotone",
      name: "frontendintegrations/qweqwe/qweqwe-cluster",
      order: 999,
      parent: "cluster",
      title: "qweqw",
    },
    {
      icon: "GridDuotone",
      name: "frontendintegrations/qweqwe/qweqwe-cluster/frontendintegrations",
      order: 999,
      parent: "cluster.frontendintegrations/qweqwe/qweqwe-cluster",
      title: "qweq",
    },
  ],
  name: "qweqwe",
  pages: [
    {
      componentsTree: {
        context: {},
        dataSources: [
          {
            config: {
              COLUMNS_CONFIG: [
                {
                  enableSorting: true,
                  key: "name",
                  render: {
                    path: "metadata.name",
                    payload: {},
                    type: "text",
                  },
                  title: "NAME",
                },
                {
                  enableHiding: true,
                  enableSorting: true,
                  key: "updateTime",
                  render: {
                    path: "metadata.creationTimestamp",
                    payload: {
                      format: "local-datetime",
                    },
                    type: "time",
                  },
                  title: "CREATION_TIME",
                },
              ],
              HOOK_NAME: "useCrdColumns",
            },
            id: "columns",
            type: "crd-columns",
          },
          {
            args: [
              {
                bind: "columns",
                source: "columns",
                type: "binding",
              },
            ],
            config: {
              CRD_CONFIG: {
                apiVersion: "v1alpha1",
                group: "frontend-forge.kubesphere.io",
                kapi: true,
                kind: "FrontendIntegration",
                plural: "frontendintegrations",
              },
              HOOK_NAME: "useCrdPageState",
              PAGE_ID: "qweqwe-cluster-qweqwe-cluster_frontendintegrations",
              SCOPE: "cluster",
            },
            id: "pageState",
            type: "crd-page-state",
          },
        ],
        meta: {
          id: "qweqwe-cluster-qweqwe-cluster_frontendintegrations",
          name: "qweqwe-cluster-qweqwe-cluster_frontendintegrations",
          path: "/qweqwe-cluster-qweqwe-cluster_frontendintegrations",
          title: "qweq",
        },
        root: {
          id: "qweqwe-cluster-qweqwe-cluster_frontendintegrations-root",
          meta: {
            scope: true,
            title: "CrdTable",
          },
          props: {
            COLUMNS: {
              bind: "columns",
              source: "columns",
              type: "binding",
            },
            CREATE: {
              bind: "create",
              source: "pageState",
              type: "binding",
            },
            CREATE_INITIAL_VALUE: {
              apiVersion: "frontend-forge.kubesphere.io/v1alpha1",
              kind: "FrontendIntegration",
            },
            DATA: {
              bind: "data",
              source: "pageState",
              type: "binding",
            },
            DEL: {
              bind: "del",
              source: "pageState",
              type: "binding",
            },
            IS_LOADING: {
              bind: "loading",
              defaultValue: false,
              source: "pageState",
              type: "binding",
            },
            PAGE_CONTEXT: {
              bind: "pageContext",
              source: "pageState",
              type: "binding",
            },
            PARAMS: {
              bind: "params",
              source: "pageState",
              type: "binding",
            },
            REFETCH: {
              bind: "refetch",
              source: "pageState",
              type: "binding",
            },
            TABLE_KEY: "qweqwe-cluster-qweqwe-cluster_frontendintegrations",
            TITLE: "qweq",
            TOOLBAR_LEFT: {
              bind: "toolbarLeft",
              source: "pageState",
              type: "binding",
            },
            UPDATE: {
              bind: "update",
              source: "pageState",
              type: "binding",
            },
          },
          type: "CrdTable",
        },
      },
      entryComponent: "qweqwe-cluster-qweqwe-cluster_frontendintegrations",
      id: "qweqwe-cluster-qweqwe-cluster_frontendintegrations",
    },
  ],
  routes: [
    {
      pageId: "qweqwe-cluster-qweqwe-cluster_frontendintegrations",
      path: "/clusters/:cluster/frontendintegrations/qweqwe/qweqwe-cluster/frontendintegrations",
    },
  ],
  version: "1.0",
};

function writeFiles(outputDir: string, files: VirtualFile[]) {
  for (const file of files) {
    const fullPath = path.join(outputDir, file.path);
    fs.mkdirSync(path.dirname(fullPath), { recursive: true });
    fs.writeFileSync(fullPath, file.content, "utf8");
  }
}

const builtFiles = await forge.buildProject(manifest, { build: true });
const outputDir = path.resolve(process.cwd(), ".tmp", "qweqwe-demo-build");
writeFiles(outputDir, builtFiles);

const indexJs = builtFiles.find((file) => file.path === "index.js");
if (!indexJs) {
  throw new Error("index.js was not generated");
}

const summary = {
  outputDir,
  files: builtFiles.map((file) => ({
    path: file.path,
    bytes: Buffer.byteLength(file.content, "utf8"),
  })),
  indexJsBytes: Buffer.byteLength(indexJs.content, "utf8"),
  indexJsPreview: indexJs.content.slice(0, 240),
};

const summaryPath = path.join(outputDir, "summary.json");
fs.writeFileSync(summaryPath, JSON.stringify(summary, null, 2), "utf8");

console.log(JSON.stringify(summary, null, 2));
