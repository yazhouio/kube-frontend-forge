import { type NodeDefinition } from "@frontend-forge/forge-core/advanced";

export const CrdTableNode: NodeDefinition = {
  id: "CrdTable",
  schema: {
    templateInputs: {
      TABLE_KEY: {
        type: "string",
        description: "Table key",
      },
      TITLE: {
        type: "string",
        description: "Table title",
      },
      AUTH_KEY: {
        type: "string",
        description: "Auth key",
      },
      PARAMS: {
        type: "object",
        description: "Route params",
      },
      REFETCH: {
        type: "object",
        description: "Refetch handler",
      },
      TOOLBAR_LEFT: {
        type: "object",
        description: "Toolbar left renderer",
      },
      PAGE_CONTEXT: {
        type: "object",
        description: "Page context",
      },
      COLUMNS: {
        type: "array",
        description: "Table columns",
      },
      DATA: {
        type: "object",
        description: "Table data",
      },
      NOT_FOUND_EMPTY_PROPS: {
        type: "object",
        description: "404 empty fallback props",
      },
      IS_LOADING: {
        type: "boolean",
        description: "Loading state",
      },
      UPDATE: {
        type: "object",
        description: "Update handler",
      },
      DEL: {
        type: "object",
        description: "Delete handler",
      },
      CREATE: {
        type: "object",
        description: "Create handler",
      },
      CREATE_INITIAL_VALUE: {
        type: "object",
        description: "Create initial value",
      },
    },
  },
  generateCode: {
    imports: [
      'import * as React from "react"',
      'import { CRDTable404Fallback, PageTable } from "@frontend-forge/forge-components"',
    ],
    stats: [],
    jsx: `<PageTable
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
        ...(%%NOT_FOUND_EMPTY_PROPS%% || {}),
      },
    },
  ]}
  update={%%UPDATE%%}
  del={%%DEL%%}
  create={%%CREATE%%}
/>`,
    meta: {
      inputPaths: {
        $jsx: [
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
      },
    },
  },
};

export const IframeNode: NodeDefinition = {
  id: "Iframe",
  schema: {
    templateInputs: {
      FRAME_URL: {
        type: "string",
        description: "Iframe src url",
      },
    },
  },
  generateCode: {
    imports: [
      'import * as React from "react"',
      'import { BaseIframe } from "@frontend-forge/forge-components"',
    ],
    stats: [],
    jsx: `<BaseIframe src={%%FRAME_URL%%} />`,
    meta: {
      inputPaths: {
        $jsx: ["FRAME_URL"],
      },
    },
  },
};
