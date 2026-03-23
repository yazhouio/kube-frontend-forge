import type { ExtensionManifest } from "@frontend-forge/forge-core";
import type {
  BuildOutputs,
  CodeExporterRequestBody,
  TailwindOptions,
  VirtualFile,
} from "@frontend-forge/forge-core/advanced";

export type { BuildOutputs, ExtensionManifest, TailwindOptions, VirtualFile };

export type BuildRequestBody = CodeExporterRequestBody;

export type PageSchemaRequestBody = {
  pageSchema?: unknown;
};

export type ProjectManifestRequestBody = {
  manifest?: ExtensionManifest;
};

export type ProjectJsBundleParams = {
  name: string;
  extensionName: string;
  namespace?: string;
  cluster?: string;
};

export type ProjectJsBundleRequestBody = ProjectManifestRequestBody & {
  params: ProjectJsBundleParams;
};

export type CacheValue = {
  outputs: BuildOutputs;
  meta: { buildMs: number; queuedMs: number };
};

export type CacheHit = "memory" | "disk" | null;

export type BuildKeyInput = {
  files: VirtualFile[];
  entry: string;
  externals: string[];
  tailwind: TailwindOptions;
};
