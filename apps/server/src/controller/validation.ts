import { ForgeError, type ForgeCore } from "@frontend-forge/forge-core";
import type { CodeExporter } from "@frontend-forge/forge-core/advanced";
import type {
  ExtensionManifest,
  PageSchemaRequestBody,
  ProjectJsBundleRequestBody,
  ProjectManifestRequestBody,
} from "../types.js";
import { normalizeDns1123Label } from "../k8sClient.js";

export function requireExporter(forge: ForgeCore): CodeExporter {
  const exporter = forge.getCodeExporter();
  if (!exporter || typeof exporter !== "object") {
    throw new ForgeError("codeExporter is required", 500);
  }
  if (typeof (exporter as CodeExporter).build !== "function") {
    throw new ForgeError("codeExporter.build is required", 500);
  }
  return exporter as CodeExporter;
}

export function requirePageSchema(
  body: PageSchemaRequestBody | unknown,
): unknown {
  if (body && typeof body === "object" && "pageSchema" in body) {
    const wrapper = body as PageSchemaRequestBody;
    if (wrapper.pageSchema == null) {
      throw new ForgeError("pageSchema is required", 400);
    }
    return wrapper.pageSchema;
  }
  if (body == null) {
    throw new ForgeError("pageSchema is required", 400);
  }
  return body;
}

export function requireManifest(
  body: ProjectManifestRequestBody | unknown,
): ExtensionManifest {
  if (!body || typeof body !== "object") {
    throw new ForgeError("manifest is required", 400);
  }
  const wrapper = body as ProjectManifestRequestBody;
  const hasManifest = Object.prototype.hasOwnProperty.call(wrapper, "manifest");
  const manifest = hasManifest ? wrapper.manifest : body;
  if (!manifest || typeof manifest !== "object") {
    throw new ForgeError("manifest is required", 400);
  }
  return manifest as ExtensionManifest;
}

export function requireJsBundleParams(
  body: ProjectJsBundleRequestBody | unknown,
): {
  name: string;
  extensionName: string;
  namespace: string | null;
  cluster: string | null;
} {
  if (!body || typeof body !== "object") {
    throw new ForgeError("params is required", 400);
  }
  if (!("params" in body)) {
    throw new ForgeError("params is required", 400);
  }

  const params = (body as ProjectJsBundleRequestBody).params as unknown;
  if (!params || typeof params !== "object") {
    throw new ForgeError("params must be an object", 400);
  }

  const rawName = (params as { name?: unknown }).name;
  if (typeof rawName !== "string" || rawName.trim().length === 0) {
    throw new ForgeError("params.name is required", 400);
  }
  const name = normalizeDns1123Label(rawName.trim(), "name");

  const rawExtensionName = (params as { extensionName?: unknown })
    .extensionName;
  if (
    typeof rawExtensionName !== "string" ||
    rawExtensionName.trim().length === 0
  ) {
    throw new ForgeError("params.extensionName is required", 400);
  }
  const extensionName = normalizeDns1123Label(
    rawExtensionName.trim(),
    "extensionName",
  );

  const namespaceRaw = (params as { namespace?: unknown }).namespace;
  const namespace =
    typeof namespaceRaw === "string" && namespaceRaw.trim().length > 0
      ? normalizeDns1123Label(namespaceRaw.trim(), "namespace")
      : null;

  const clusterRaw = (params as { cluster?: unknown }).cluster;
  const cluster =
    typeof clusterRaw === "string" && clusterRaw.trim().length > 0
      ? normalizeDns1123Label(clusterRaw.trim(), "cluster")
      : null;

  return { name, extensionName, namespace, cluster };
}
