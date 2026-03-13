import {
  StatementScope,
  type NodeDefinition,
} from "@frontend-forge/forge-core/advanced";

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
      'import { PageTable } from "@frontend-forge/forge-components"',
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
      'import { useRef, useState } from "react"',
      'import { Loading } from "@kubed/components"',
    ],
    stats: [
      {
        id: "loadingState",
        scope: StatementScope.FunctionBody,
        code: "const [loading, setLoading] = useState(new URL(%%FRAME_URL%%, window.location.href).origin === window.location.origin);",
        output: ["loading", "setLoading"],
        depends: [],
      },
      {
        id: "iframeRef",
        scope: StatementScope.FunctionBody,
        code: "const iframeRef = useRef(null);",
        output: ["iframeRef"],
        depends: [],
      },
      {
        id: "onIframeLoad",
        scope: StatementScope.FunctionBody,
        code: `const onIframeLoad = () => {
  const iframeDom = iframeRef.current?.contentWindow?.document;
  setLoading(false);
};`,
        output: ["onIframeLoad"],
        depends: ["iframeRef", "loadingState"],
      },
    ],
    jsx: `<>
  {loading && <Loading className="page-loading" />}
  <iframe
    ref={iframeRef}
    src={%%FRAME_URL%%}
    width="100%"
    height="100%"
    frameBorder="0"
    style={{
      height: "calc(100vh - 68px)",
      display: loading ? "none" : "block",
    }}
    onLoad={onIframeLoad}
  />
</>`,
    meta: {
      inputPaths: {
        $jsx: ["FRAME_URL"],
        loadingState: ["FRAME_URL"],
      },
    },
  },
};
