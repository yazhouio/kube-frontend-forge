import { DataTable } from "@kubed/components";

import type {
  ColumnDef,
  Table as ReactTable,
  RowModel,
  TableMeta,
} from "@tanstack/react-table";
import React, { useCallback, useImperativeHandle } from "react";
import type { ForwardedRef, ReactElement, RefAttributes } from "react";
import { usePageStore } from "../../hooks/useTanStackPageStore";
import { usePageTable } from "../../hooks/useTanStackPageTable";

type TablePayload<TData> = {
  data: TData[];
  total: number;
  status?: number;
  ok?: boolean;
};

type TableMetaWithName<TData> = TableMeta<TData> & {
  tableName: string;
};

export type BaseTableHandle<TData> = {
  getSelectedRowModel: () => RowModel<TData>;
};

export type TableProps<TData extends { uid: string }> = {
  columns: ColumnDef<TData>[];
  data?: TablePayload<TData>;
  isLoading?: boolean;
  tableMeta: TableMetaWithName<TData>;
};

const defaultValue = [];

const Table = <TData extends { uid: string }>(
  props: TableProps<TData>,
  ref: ForwardedRef<BaseTableHandle<TData>>,
) => {
  const { columns, data, isLoading, tableMeta } = props;

  const page = usePageStore<TData>({
    pageId: tableMeta.tableName,
    columns,
  });


  const table = usePageTable<TData>({
    data: data?.data ?? defaultValue,
    columns,
    page,
    tableMeta,
    tableOptions: {
      getRowId: useCallback((row) => row.uid, []),
      loading: isLoading,
      rowCount: data?.total || 0,
    },
  });

  useImperativeHandle(
    ref,
    () => ({
      getSelectedRowModel: table.getSelectedRowModel,
      resetRowSelection: table.resetRowSelection,
    }),
    [table],
  );

  return <DataTable.DataTable table={table} />;
};

type BaseTableComponent = <TData extends { uid: string }>(
  props: TableProps<TData> & RefAttributes<BaseTableHandle<TData>>,
) => ReactElement | null;

export const BaseTable = React.forwardRef(Table) as BaseTableComponent;

export * from "./TableTd";
