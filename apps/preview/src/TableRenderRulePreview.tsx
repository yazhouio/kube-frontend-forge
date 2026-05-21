import {
  BaseTable,
  RuntimeProvider,
  TableTd,
} from "@frontend-forge/forge-components";
import { Card } from "@kubed/components";
import { getLocalTime } from "@ks-console/shared";
import * as React from "react";
import { useMemo } from "react";
import { usePageRuntimeRouter } from "./routerHook";

const columnsConfig = [
  {
    key: "name",
    title: "Legacy text",
    render: {
      type: "text",
      path: "metadata.name",
      payload: {},
    },
  },
  {
    key: "link",
    title: "Link",
    path: "metadata.name",
    valueType: "string",
    displayType: "link",
    payload: {
      to: "/resources/{metadata.namespace}/{metadata.name}",
    },
  },
  {
    key: "created",
    title: "Date",
    path: "metadata.creationTimestamp",
    valueType: "date",
    displayType: "date",
    payload: {
      format: "local-datetime",
    },
  },
  {
    key: "phase",
    title: "Enum",
    path: "status.phase",
    valueType: "enum",
    displayType: "enum",
    payload: {
      map: {
        Running: "Running",
        Failed: "Failed",
      },
    },
  },
  {
    key: "count",
    title: "Number",
    path: "metrics.count",
    valueType: "number",
    displayType: "number",
  },
  {
    key: "enabled",
    title: "Boolean",
    path: "spec.enabled",
    valueType: "boolean",
    displayType: "boolean",
    payload: {
      trueText: "Enabled",
      falseText: "Disabled",
    },
  },
  {
    key: "labels",
    title: "Object",
    path: "metadata.labels",
    valueType: "object",
    displayType: "text",
  },
  {
    key: "tags",
    title: "List",
    path: "spec.tags",
    valueType: "array",
    displayType: "list",
    payload: {
      separator: " / ",
    },
  },
  {
    key: "missing",
    title: "Empty",
    path: "spec.missing",
    valueType: "string",
    displayType: "text",
    emptyText: "N/A",
  },
];

const tableData = [
  {
    uid: "render-rules-1",
    metadata: {
      name: "frontend-alpha",
      namespace: "system",
      creationTimestamp: "2025-01-01T08:00:00Z",
      labels: {
        app: "frontend",
        tier: "web",
      },
    },
    status: {
      phase: "Running",
    },
    metrics: {
      count: 0,
    },
    spec: {
      enabled: false,
      tags: ["alpha", "stable"],
    },
  },
  {
    uid: "render-rules-2",
    metadata: {
      name: "frontend-beta",
      namespace: "dev",
      creationTimestamp: "2025-02-03T12:30:00Z",
      labels: {
        app: "frontend",
        tier: "api",
      },
    },
    status: {
      phase: "Failed",
    },
    metrics: {
      count: 1200,
    },
    spec: {
      enabled: true,
      tags: ["beta", "canary"],
    },
  },
];

const tableMeta = {
  tableName: "table-render-rules-preview",
  getProps: {
    table: () => ({
      stickyHeader: true,
      tableWrapperClassName: "table",
    }),
    filters: () => ({
      simpleMode: false,
      suggestions: [],
    }),
  },
};

const pageContext = {
  getLocalTime,
};

function TableRenderRulePreviewItem() {
  const columns = useMemo(
    () =>
      columnsConfig.map((column) => {
        const {
          key,
          title,
          render,
          path,
          valueType,
          displayType,
          payload,
          emptyText,
          ...rest
        } = column;
        const renderConfig =
          render ?? { path, valueType, displayType, payload, emptyText };

        return {
          accessorKey: key,
          header: title,
          cell: (info) => (
            <TableTd meta={renderConfig} original={info.row.original} />
          ),
          ...rest,
        };
      }),
    [],
  );

  return (
    <Card padding={0}>
      <BaseTable
        columns={columns}
        data={{
          data: tableData,
          total: tableData.length,
        }}
        tableMeta={tableMeta}
      />
    </Card>
  );
}

export function TableRenderRulePreview() {
  const route = usePageRuntimeRouter();

  return (
    <RuntimeProvider
      value={{
        ...route,
        page: {
          id: "table-render-rules-preview",
        },
        capabilities: pageContext,
      }}
    >
      <TableRenderRulePreviewItem />
    </RuntimeProvider>
  );
}
