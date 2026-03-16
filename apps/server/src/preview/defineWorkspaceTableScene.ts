import type { PageConfig } from "@frontend-forge/forge-core/advanced";

type CrdTableScope = "namespace" | "cluster" | string;

type CrdConfig = {
  apiVersion: string;
  kind: string;
  plural: string;
  group: string;
  kapi?: boolean;
  [key: string]: unknown;
};

type CrdTableEmptyState = {
  titleI18nKey?: string;
  descriptionI18nKey?: string;
  command?: string;
};

type CrdTablePageInfo = {
  id: string;
  title: string;
  authKey: string;
  emptyState?: CrdTableEmptyState;
};

type CrdTableColumnRender = {
  type: "text" | "time" | "link";
  path: string;
  format?: "local-datetime" | "utc";
  pattern?: string;
  link?: string;
  payload?: Record<string, unknown>;
};

type CrdTableColumn = {
  key: string;
  title: string;
  render: CrdTableColumnRender;
  enableHiding?: boolean;
  enableSorting?: boolean;
  [key: string]: unknown;
};

export type WorkspaceTableSceneConfig = {
  meta: {
    id: string;
    name: string;
    title?: string;
    path: string;
  };
  crd: CrdConfig;
  scope: CrdTableScope;
  page: CrdTablePageInfo;
  columns: CrdTableColumn[];
  storeOptions?: Record<string, unknown>;
};

const DEFAULT_EMPTY_TITLE_I18N_KEY =
  "PROJECT_GENERATOR.CRD_NOT_FOUND_TITLE";
const DEFAULT_EMPTY_DESCRIPTION_I18N_KEY =
  "PROJECT_GENERATOR.CRD_NOT_FOUND_DESCRIPTION";

const buildCreateInitialValue = (crd: CrdConfig) => {
  const apiVersion =
    typeof crd.apiVersion === "string" && crd.apiVersion.includes("/")
      ? crd.apiVersion
      : crd.group
        ? `${crd.group}/${crd.apiVersion}`
        : crd.apiVersion;
  return {
    apiVersion,
    kind: crd.kind,
  };
};

const buildEmptyCommand = (crd: CrdConfig) =>
  `kubectl label crd ${crd.plural}.${crd.group} kubesphere.io/resource-served=true`;

const buildColumnRender = (render: CrdTableColumnRender) => {
  const payload = { ...(render.payload ?? {}) };

  if (render.type === "time") {
    if (render.format) {
      payload.format = render.format;
    }
    if (render.pattern) {
      payload.pattern = render.pattern;
    }
  }

  if (render.type === "link" && render.link) {
    payload.link = render.link;
  }

  return {
    type: render.type,
    path: render.path,
    payload,
  };
};

export const defineWorkspaceTableScene = (
  scene: WorkspaceTableSceneConfig,
): PageConfig => {
  const columnsConfig = scene.columns.map((column) => {
    const { key, title, render, ...rest } = column;
    return {
      key,
      title,
      render: buildColumnRender(render),
      ...rest,
    };
  });
  const pageStateArgs: Record<string, unknown>[] = [
    {
      type: "binding",
      source: "columns",
      bind: "columns",
    },
  ];
  if (scene.storeOptions !== undefined) {
    pageStateArgs.push(scene.storeOptions);
  }

  return {
    meta: {
      id: scene.page.id,
      name: scene.page.id,
      title: scene.page.title,
      path: `/${scene.page.id}`,
    },
    context: {},
    dataSources: [
      {
        id: "columns",
        type: "crd-columns",
        config: {
          COLUMNS_CONFIG: columnsConfig,
          HOOK_NAME: "useCrdColumns",
        },
      },
      {
        id: "pageState",
        type: "workspace-crd-page-state",
        args: pageStateArgs,
        config: {
          PAGE_ID: scene.page.id,
          CRD_CONFIG: scene.crd,
          HOOK_NAME: "useCrdPageState",
        },
      },
    ],
    root: {
      id: `${scene.page.id}-root`,
      type: "CrdTable",
      props: {
        TABLE_KEY: scene.page.id,
        TITLE: scene.page.title,
        AUTH_KEY: scene.page.authKey,
        PARAMS: {
          type: "binding",
          source: "pageState",
          bind: "params",
        },
        REFETCH: {
          type: "binding",
          source: "pageState",
          bind: "refetch",
        },
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
        COLUMNS: {
          type: "binding",
          source: "columns",
          bind: "columns",
        },
        DATA: {
          type: "binding",
          source: "pageState",
          bind: "data",
        },
        IS_LOADING: {
          type: "binding",
          source: "pageState",
          bind: "loading",
          defaultValue: false,
        },
        UPDATE: {
          type: "binding",
          source: "pageState",
          bind: "update",
        },
        DEL: {
          type: "binding",
          source: "pageState",
          bind: "del",
        },
        CREATE: {
          type: "binding",
          source: "pageState",
          bind: "create",
        },
        NOT_FOUND_EMPTY_PROPS: {
          titleI18nKey:
            scene.page.emptyState?.titleI18nKey ??
            DEFAULT_EMPTY_TITLE_I18N_KEY,
          descriptionI18nKey:
            scene.page.emptyState?.descriptionI18nKey ??
            DEFAULT_EMPTY_DESCRIPTION_I18N_KEY,
          command: scene.page.emptyState?.command ?? buildEmptyCommand(scene.crd),
        },
        CREATE_INITIAL_VALUE: buildCreateInitialValue(scene.crd),
      },
      meta: {
        title: "CrdTable",
        scope: true,
      },
    },
  };
};
