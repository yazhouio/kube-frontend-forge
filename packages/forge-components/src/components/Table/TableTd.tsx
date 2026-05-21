import { get, isObject } from "es-toolkit/compat";
import * as React from "react";
import { Link } from "react-router-dom";
import { useRuntimeContext } from "../../hooks";

import type { RowData } from "@tanstack/react-table";

export type TableCellDisplayType =
  | "text"
  | "date"
  | "enum"
  | "link"
  | "number"
  | "boolean"
  | "json"
  | "list";

export type TableCellValueType =
  | "string"
  | "text"
  | "date"
  | "enum"
  | "link"
  | "number"
  | "boolean"
  | "object"
  | "json"
  | "array"
  | "list";

export type TableCellRenderPayload = Record<string, any> & {
  link?: string;
  to?: string;
  href?: string;
  map?: Record<string, React.ReactNode>;
};

export interface TableCellRenderConfig {
  type?: string;
  path?: string;
  valueType?: TableCellValueType | string;
  displayType?: TableCellDisplayType | string;
  payload?: TableCellRenderPayload;
  emptyText?: React.ReactNode;
}

declare module "@tanstack/react-table" {
  interface ColumnMeta<TData extends RowData, TValue> {
    renderCell?: TableCellRenderConfig;
  }
}

export interface TableTdProps {
  meta?: TableCellRenderConfig;
  original: Record<string, unknown>;
}

const legacyDisplayTypeMap: Record<string, TableCellDisplayType> = {
  text: "text",
  link: "link",
  time: "date",
};

const valueTypeDisplayMap: Partial<
  Record<TableCellValueType, TableCellDisplayType>
> = {
  string: "text",
  text: "text",
  date: "date",
  enum: "enum",
  link: "link",
  number: "number",
  boolean: "boolean",
  object: "json",
  json: "json",
  array: "list",
  list: "list",
};

export function TableTd(props: TableTdProps) {
  const { meta, original } = props;
  const payload = meta?.payload ?? {};
  const emptyText = meta?.emptyText ?? payload.emptyText ?? "-";
  const value = getValueByPath(original, meta?.path);
  const displayType = resolveDisplayType(meta);

  if (isEmptyCellValue(value)) {
    return <div>{emptyText}</div>;
  }

  if (displayType === "date") {
    return <TableTdTime {...payload} value={value} emptyText={emptyText} />;
  }

  if (displayType === "enum") {
    return <TableTdEnum {...payload} value={value} />;
  }

  if (displayType === "link") {
    return (
      <TableTdLink
        {...payload}
        value={value}
        original={original}
        emptyText={emptyText}
      />
    );
  }

  if (displayType === "number") {
    return <TableTdNumber {...payload} value={value} />;
  }

  if (displayType === "boolean") {
    return <TableTdBoolean {...payload} value={value} />;
  }

  if (displayType === "json") {
    return <TableTdJson {...payload} value={value} />;
  }

  if (displayType === "list") {
    return <TableTdList {...payload} value={value} />;
  }

  return <TableTdText {...payload} value={value} />;
}

function resolveDisplayType(meta?: TableCellRenderConfig): TableCellDisplayType {
  if (isTableCellDisplayType(meta?.displayType)) {
    return meta.displayType;
  }

  if (meta?.type && legacyDisplayTypeMap[meta.type]) {
    return legacyDisplayTypeMap[meta.type];
  }

  const displayTypeByValue =
    valueTypeDisplayMap[meta?.valueType as TableCellValueType];

  if (displayTypeByValue) {
    return displayTypeByValue;
  }

  return "text";
}

function isTableCellDisplayType(
  value: string | undefined,
): value is TableCellDisplayType {
  return (
    value === "text" ||
    value === "date" ||
    value === "enum" ||
    value === "link" ||
    value === "number" ||
    value === "boolean" ||
    value === "json" ||
    value === "list"
  );
}

function getValueByPath(
  original: Record<string, unknown>,
  path: string | undefined,
) {
  if (!path) {
    return undefined;
  }

  return get(original, path);
}

function isEmptyCellValue(value: unknown): value is null | undefined | "" | [] {
  return (
    value === null ||
    value === undefined ||
    value === "" ||
    (Array.isArray(value) && value.length === 0)
  );
}

function safeStringify(value: unknown, space?: number) {
  try {
    return JSON.stringify(value, null, space);
  } catch {
    return String(value);
  }
}

function toDisplayText(value: unknown) {
  if (isEmptyCellValue(value)) {
    return "";
  }

  if (isObject(value)) {
    return safeStringify(value);
  }

  return String(value);
}

function renderDisplayValue(value: unknown) {
  if (React.isValidElement(value)) {
    return <div>{value}</div>;
  }

  return <div>{toDisplayText(value)}</div>;
}

export function TableTdText(props: { value: unknown }) {
  const { value } = props;
  return renderDisplayValue(value);
}

export function TableTdTime(props: {
  value: unknown;
  format?: "local-datetime" | "utc" | string;
  pattern?: string;
  emptyText?: React.ReactNode;
}) {
  const { value, format, pattern, emptyText = "-" } = props;
  const runtime = useRuntimeContext();
  const getLocalTime = runtime?.capabilities?.getLocalTime;

  if (isEmptyCellValue(value)) {
    return <div>{emptyText}</div>;
  }

  if (format === "local-datetime") {
    if (getLocalTime) {
      return (
        <div>
          {getLocalTime(value).format(pattern ?? "YYYY-MM-DD HH:mm:ss")}
        </div>
      );
    }
    return renderDisplayValue(value);
  }

  if (format === "utc") {
    const date = new Date(value as string | number | Date);

    if (Number.isNaN(date.getTime())) {
      return renderDisplayValue(value);
    }

    return <div>{date.toISOString()}</div>;
  }

  return renderDisplayValue(value);
}

export function TableTdEnum(props: {
  value: unknown;
  map?: Record<string, React.ReactNode>;
}) {
  const { value, map = {} } = props;
  const key = toDisplayText(value);
  const mappedValue = Object.prototype.hasOwnProperty.call(map, key)
    ? map[key]
    : value;

  return renderDisplayValue(mappedValue);
}

export function TableTdLink(props: {
  value: unknown;
  link?: string;
  to?: string;
  href?: string;
  original?: Record<string, unknown>;
  emptyText?: React.ReactNode;
}) {
  const { value, original = {}, emptyText = "-" } = props;
  const target = resolveLinkTarget(props, original, value);
  const displayValue = toDisplayText(value);

  if (isEmptyCellValue(displayValue)) {
    return <div>{emptyText}</div>;
  }

  if (!target) {
    return <div>{displayValue}</div>;
  }

  return <Link to={target}>{displayValue}</Link>;
}

export function TableTdNumber(
  props: { value: unknown; locale?: string | string[] } &
    Intl.NumberFormatOptions,
) {
  const { value, locale, ...formatOptions } = props;
  const numericValue = typeof value === "number" ? value : Number(value);

  if (Number.isNaN(numericValue)) {
    return renderDisplayValue(value);
  }

  try {
    return (
      <div>
        {new Intl.NumberFormat(locale, formatOptions).format(numericValue)}
      </div>
    );
  } catch {
    return <div>{String(numericValue)}</div>;
  }
}

export function TableTdBoolean(props: {
  value: unknown;
  trueText?: React.ReactNode;
  falseText?: React.ReactNode;
}) {
  const { value, trueText = "true", falseText = "false" } = props;
  const normalizedValue =
    typeof value === "string" ? value.toLowerCase() : value;

  if (
    normalizedValue === true ||
    normalizedValue === "true" ||
    normalizedValue === "1" ||
    value === 1
  ) {
    return renderDisplayValue(trueText);
  }

  if (
    normalizedValue === false ||
    normalizedValue === "false" ||
    normalizedValue === "0" ||
    value === 0
  ) {
    return renderDisplayValue(falseText);
  }

  return renderDisplayValue(value);
}

export function TableTdJson(props: { value: unknown; space?: number }) {
  const { value, space } = props;
  return <div>{safeStringify(value, space)}</div>;
}

export function TableTdList(props: {
  value: unknown;
  separator?: string;
  itemPath?: string;
}) {
  const { value, separator = ", ", itemPath } = props;

  if (!Array.isArray(value)) {
    return renderDisplayValue(value);
  }

  const text = value
    .map((item) => {
      if (!itemPath) {
        return toDisplayText(item);
      }

      if (!isObject(item)) {
        return toDisplayText(item);
      }

      return toDisplayText(get(item, itemPath));
    })
    .join(separator);

  return <div>{text}</div>;
}

function resolveLinkTarget(
  props: {
    link?: string;
    to?: string;
    href?: string;
  },
  original: Record<string, unknown>,
  value: unknown,
) {
  const template = props.to ?? props.link ?? props.href;

  if (!template) {
    return undefined;
  }

  return template.replace(/\{([^{}]+)\}/g, (_, rawToken: string) => {
    const token = rawToken.trim();
    const resolvedValue = token === "value" ? value : get(original, token);

    return toDisplayText(resolvedValue);
  });
}
