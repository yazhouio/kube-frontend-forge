import { Card, Empty } from "@kubed/components";
import * as React from "react";
import { useRuntimeContext } from "../../runtime";

export type PageTableData = {
  data?: unknown[];
  total?: number;
  status?: number;
  ok?: boolean;
};

export type PageTable404EmptyProps = {
  titleI18nKey?: string;
  descriptionI18nKey?: string;
  command?: string;
};

export type PageTableFallbackConfig = {
  type: string;
  is: (data?: PageTableData) => boolean;
  component: React.ComponentType<any>;
  props?: Record<string, unknown>;
};

export type PageTableFallbacks = PageTableFallbackConfig[];

const DEFAULT_CRD_NOT_FOUND_TITLE_I18N =
  "PROJECT_GENERATOR.CRD_NOT_FOUND_TITLE";
const DEFAULT_CRD_NOT_FOUND_DESCRIPTION_I18N =
  "PROJECT_GENERATOR.CRD_NOT_FOUND_DESCRIPTION";
const DEFAULT_CRD_NOT_FOUND_COMMAND =
  "kubectl label crd servicemonitors.monitoring.coreos.com kubesphere.io/resource-served=true";

export function CRDTable404Empty(props: PageTable404EmptyProps) {
  const {
    titleI18nKey = DEFAULT_CRD_NOT_FOUND_TITLE_I18N,
    descriptionI18nKey = DEFAULT_CRD_NOT_FOUND_DESCRIPTION_I18N,
    command = DEFAULT_CRD_NOT_FOUND_COMMAND,
  } = props;
  const runtime = useRuntimeContext();
  const t = runtime?.capabilities?.t ?? ((d: string) => d);

  return (
    <Card padding={32}>
      <Empty
        title={t(titleI18nKey)}
        description={
          <div style={{ textAlign: "center" }}>
            <div>{t(descriptionI18nKey)}</div>
            {command ? <div>{command}</div> : null}
          </div>
        }
      />
    </Card>
  );
}

export const CRDTable404Fallback: PageTableFallbackConfig = {
  type: "crd404",
  is: (data) => data?.status === 404,
  component: CRDTable404Empty,
  props: {
    titleI18nKey: DEFAULT_CRD_NOT_FOUND_TITLE_I18N,
    descriptionI18nKey: DEFAULT_CRD_NOT_FOUND_DESCRIPTION_I18N,
    command: DEFAULT_CRD_NOT_FOUND_COMMAND,
  },
};

export function resolveMatchedFallback(
  fallbacks: PageTableFallbacks | undefined,
  data: PageTableData | undefined,
) {
  const validFallbacks = (fallbacks ?? []).filter(
    (fallback) =>
      Boolean(fallback) &&
      typeof fallback.type === "string" &&
      fallback.type.length > 0 &&
      typeof fallback.is === "function" &&
      Boolean(fallback.component),
  );

  return validFallbacks.find((fallback) => {
    try {
      return Boolean(fallback.is(data));
    } catch {
      return false;
    }
  });
}
