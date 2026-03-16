import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import type {
  ExtensionManifest,
  GenerateProjectFilesOptions,
  GenerateProjectFilesResult,
  PageMeta,
  VirtualFile,
} from "./projectTypes.js";

const SCAFFOLD_DIR = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "scaffold",
);

function logMessage(
  options: { onLog?: (message: string) => void },
  message: string,
): void {
  if (typeof options.onLog === "function") options.onLog(message);
}

function assertString(value: unknown, label: string): string {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value;
}

function assertObject(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value as Record<string, unknown>;
}

function assertArray(value: unknown, label: string): unknown[] {
  if (!Array.isArray(value)) throw new Error(`${label} must be an array`);
  return value;
}

function renderTemplate(content: string, vars: Record<string, string>): string {
  let out = content;
  for (const [key, value] of Object.entries(vars)) {
    out = out.split(`__${key}__`).join(value);
  }
  return out;
}

function ensureSafeFileName(name: string, label: string): void {
  if (!/^[A-Za-z0-9._-]+$/.test(name)) {
    throw new Error(`${label} contains unsupported characters: ${name}`);
  }
}

function isValidIdentifier(name: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(name);
}

function toIdentifier(name: string): string {
  const safe = name.replace(/[^A-Za-z0-9_]/g, "_");
  if (safe.length === 0) return "lang";
  if (!/^[A-Za-z_]/.test(safe)) return `lang_${safe}`;
  return safe;
}

function toComponentIdentifier(name: string, fallback: string): string {
  const parts = name.match(/[A-Za-z0-9]+/g) || [];
  if (parts.length === 0) return fallback;

  const pascal = parts
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join("");

  if (!/^[A-Za-z_]/.test(pascal)) return `${fallback}${pascal}`;
  return pascal;
}

function validateManifest(manifest: ExtensionManifest): void {
  assertObject(manifest, "manifest");
  if (manifest.version !== "1.0") {
    throw new Error('manifest.version must be "1.0"');
  }
  assertString(manifest.name, "manifest.name");

  const routes = assertArray(manifest.routes, "manifest.routes");
  for (const [i, r] of routes.entries()) {
    const route = assertObject(r, `routes[${i}]`);
    assertString(route.path, `routes[${i}].path`);
    assertString(route.pageId, `routes[${i}].pageId`);
  }

  const menus = assertArray(manifest.menus, "manifest.menus");
  for (const [i, m] of menus.entries()) {
    const menu = assertObject(m, `menus[${i}]`);
    assertString(menu.parent, `menus[${i}].parent`);
    assertString(menu.name, `menus[${i}].name`);
    assertString(menu.title, `menus[${i}].title`);
    if (menu.icon != null && typeof menu.icon !== "string") {
      throw new Error(`menus[${i}].icon must be a string`);
    }
    if (menu.order != null && typeof menu.order !== "number") {
      throw new Error(`menus[${i}].order must be a number`);
    }
    if (menu.clusterModule != null && typeof menu.clusterModule !== "string") {
      throw new Error(`menus[${i}].clusterModule must be a string`);
    }
  }

  const locales = assertArray(manifest.locales, "manifest.locales");
  for (const [i, l] of locales.entries()) {
    const locale = assertObject(l, `locales[${i}]`);
    assertString(locale.lang, `locales[${i}].lang`);
    const messages = assertObject(locale.messages, `locales[${i}].messages`);
    for (const [k, v] of Object.entries(messages)) {
      if (typeof v !== "string") {
        throw new Error(`locales[${i}].messages[${k}] must be a string`);
      }
    }
  }

  const pages = assertArray(manifest.pages, "manifest.pages");
  for (const [i, p] of pages.entries()) {
    const page = assertObject(p, `pages[${i}]`);
    assertString(page.id, `pages[${i}].id`);
    assertString(page.entryComponent, `pages[${i}].entryComponent`);
  }

  if (manifest.build) {
    const build = assertObject(manifest.build, "manifest.build");
    if (build.target !== "kubesphere-extension") {
      throw new Error('manifest.build.target must be "kubesphere-extension"');
    }
    if (build.moduleName != null && typeof build.moduleName !== "string") {
      throw new Error("manifest.build.moduleName must be a string");
    }
    if (build.systemjs != null && typeof build.systemjs !== "boolean") {
      throw new Error("manifest.build.systemjs must be a boolean");
    }
  }

  const pageIds = new Set(pages.map((p) => (p as PageMeta).id));
  if (pageIds.size !== pages.length) {
    throw new Error("pages.id must be unique");
  }
  const localeLangs = new Set(locales.map((l) => (l as { lang: string }).lang));
  if (localeLangs.size !== locales.length) {
    throw new Error("locales.lang must be unique");
  }
  for (const [i, r] of routes.entries()) {
    const pageId = (r as PageMeta & { pageId: string }).pageId;
    if (!pageIds.has(pageId)) {
      throw new Error(`routes[${i}].pageId not found in pages: ${pageId}`);
    }
  }
}

function normalizeManifest(manifest: ExtensionManifest): ExtensionManifest {
  return {
    version: "1.0",
    name: String(manifest.name),
    displayName: manifest.displayName ?? undefined,
    description: manifest.description ?? undefined,
    routes: manifest.routes.map((r) => ({
      path: String(r.path),
      pageId: String(r.pageId),
    })),
    menus: manifest.menus.map((m) => ({
      parent: String(m.parent),
      name: String(m.name),
      title: String(m.title),
      icon: m.icon == null ? undefined : String(m.icon),
      order: m.order == null ? undefined : Number(m.order),
      clusterModule:
        m.clusterModule == null ? undefined : String(m.clusterModule),
      skipWorkspaceAuth: true,
    })),
    locales: manifest.locales.map((l) => ({
      lang: String(l.lang),
      messages: { ...l.messages },
    })),
    pages: manifest.pages.map((p) => ({
      id: String(p.id),
      entryComponent: String(p.entryComponent),
      componentsTree: p.componentsTree,
    })),
    build: manifest.build
      ? {
          target: "kubesphere-extension",
          moduleName: manifest.build.moduleName
            ? String(manifest.build.moduleName)
            : undefined,
          systemjs:
            manifest.build.systemjs == null
              ? undefined
              : Boolean(manifest.build.systemjs),
        }
      : undefined,
  };
}

function normalizeRelPath(relPath: string): string {
  if (typeof relPath !== "string" || relPath.length === 0) {
    throw new Error("invalid file path");
  }
  if (path.isAbsolute(relPath)) throw new Error("absolute path is not allowed");
  const normalized = path.posix.normalize(relPath.replace(/\\/g, "/"));
  if (normalized.startsWith("..") || normalized.includes("/../")) {
    throw new Error("path traversal is not allowed");
  }
  return normalized;
}

function resolveParentRoute(routePath: string): string | undefined {
  const normalized = routePath.replace(/\/+$/g, "");
  if (
    normalized === "/clusters/:cluster" ||
    normalized.startsWith("/clusters/:cluster/")
  ) {
    return "/clusters/:cluster";
  }
  if (
    normalized === "/workspaces/:workspace" ||
    normalized.startsWith("/workspaces/:workspace/")
  ) {
    return "/workspaces/:workspace";
  }
  return undefined;
}

function collectScaffoldFiles(root: string): VirtualFile[] {
  const out: VirtualFile[] = [];
  const queue: string[] = [root];
  while (queue.length > 0) {
    const current = queue.pop() as string;
    const entries = fs.readdirSync(current, { withFileTypes: true });
    for (const entry of entries) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) {
        queue.push(full);
        continue;
      }
      const rel = normalizeRelPath(path.relative(root, full));
      out.push({ path: rel, content: fs.readFileSync(full, "utf8") });
    }
  }
  return out;
}

function requiredTemplate(
  scaffold: Map<string, string>,
  relPath: string,
): string {
  const p = normalizeRelPath(relPath);
  const raw = scaffold.get(p);
  if (typeof raw !== "string") {
    throw new Error(`scaffold template not found: ${p}`);
  }
  return raw;
}

function assertLocaleMessages(
  value: unknown,
  label: string,
): Record<string, string> {
  const messages = assertObject(value, label);
  const normalized: Record<string, string> = {};

  for (const [key, message] of Object.entries(messages)) {
    if (typeof message !== "string") {
      throw new Error(`${label}[${key}] must be a string`);
    }
    normalized[key] = message;
  }

  return normalized;
}

function readScaffoldDefaultLocales(
  scaffold: Map<string, string>,
): Array<{ lang: string; messages: Record<string, string> }> {
  return Array.from(scaffold.entries())
    .filter(
      ([relPath]) =>
        relPath.startsWith("src/locales/defaults/") && relPath.endsWith(".json"),
    )
    .sort(([leftPath], [rightPath]) => leftPath.localeCompare(rightPath))
    .map(([relPath, content]) => {
      const lang = path.posix.basename(relPath, ".json");
      ensureSafeFileName(lang, "default locale lang");
      let parsed: unknown;
      try {
        parsed = JSON.parse(content);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        throw new Error(`invalid scaffold locale JSON: ${relPath}: ${message}`);
      }

      return {
        lang,
        messages: assertLocaleMessages(parsed, `${relPath} messages`),
      };
    });
}

function mergeLocales(
  defaults: Array<{ lang: string; messages: Record<string, string> }>,
  locales: ExtensionManifest["locales"],
) {
  const merged = new Map<string, Record<string, string>>();
  const order: string[] = [];

  for (const locale of defaults) {
    if (!merged.has(locale.lang)) {
      order.push(locale.lang);
    }
    merged.set(locale.lang, { ...locale.messages });
  }

  for (const locale of locales) {
    if (!merged.has(locale.lang)) {
      order.push(locale.lang);
    }
    merged.set(locale.lang, {
      ...(merged.get(locale.lang) || {}),
      ...locale.messages,
    });
  }

  return order.map((lang) => ({
    lang,
    messages: merged.get(lang) || {},
  }));
}

function warningsFor(options: {
  build?: boolean;
  archive?: boolean;
}): string[] {
  const warnings: string[] = [];
  if (options.build) warnings.push("build is not implemented");
  if (options.archive) warnings.push("archive is not implemented");
  return warnings;
}

export function generateProjectFiles(
  manifest: ExtensionManifest,
  options: GenerateProjectFilesOptions,
): GenerateProjectFilesResult {
  if (!options || typeof options !== "object") {
    throw new Error("options is required");
  }
  if (typeof options.pageRenderer !== "function") {
    throw new Error("options.pageRenderer must be a function");
  }
  if (!fs.existsSync(SCAFFOLD_DIR)) {
    throw new Error(`scaffold directory not found: ${SCAFFOLD_DIR}`);
  }

  validateManifest(manifest);
  const normalized = normalizeManifest(manifest);

  logMessage(options, "copy scaffold");
  const scaffoldFiles = collectScaffoldFiles(SCAFFOLD_DIR);
  const scaffoldMap = new Map(scaffoldFiles.map((f) => [f.path, f.content]));
  const mergedLocales = mergeLocales(
    readScaffoldDefaultLocales(scaffoldMap),
    normalized.locales,
  );

  logMessage(options, "render templates");
  const out: VirtualFile[] = [];

  const excluded = new Set([
    "package.json.tpl",
    "src/extensionConfig.ts.tpl",
    "src/routes.tsx.tpl",
    "src/routes.ts.tpl",
    "src/locales/index.ts.tpl",
    "src/pages/__PAGE__/index.tsx.tpl",
    "src/pages/__PAGE__/page.tsx.tpl",
  ]);

  for (const f of scaffoldFiles) {
    if (excluded.has(f.path)) continue;
    if (f.path.startsWith("src/pages/__PAGE__/")) continue;
    if (f.path.startsWith("src/locales/defaults/")) continue;
    out.push(f);
  }

  const packageJsonTpl = requiredTemplate(scaffoldMap, "package.json.tpl");
  out.push({
    path: "package.json",
    content: renderTemplate(packageJsonTpl, {
      NAME: normalized.name,
      VERSION: normalized.version,
    }),
  });

  const extensionConfigTpl = requiredTemplate(
    scaffoldMap,
    "src/extensionConfig.ts.tpl",
  );
  out.push({
    path: "src/extensionConfig.ts",
    content: renderTemplate(extensionConfigTpl, {
      MENUS: JSON.stringify(normalized.menus, null, 2),
    }),
  });

  const routesTemplatePath = scaffoldMap.has("src/routes.tsx.tpl")
    ? "src/routes.tsx.tpl"
    : scaffoldMap.has("src/routes.ts.tpl")
      ? "src/routes.ts.tpl"
      : null;
  if (!routesTemplatePath) {
    throw new Error("scaffold routes template not found");
  }
  const routesTpl = requiredTemplate(scaffoldMap, routesTemplatePath);

  if (routesTemplatePath.endsWith(".tsx.tpl")) {
    const used = new Set<string>();
    const nameByPageId = new Map<string, string>();
    for (const page of normalized.pages) {
      const base = toComponentIdentifier(page.id, "Page");
      let name = base;
      let idx = 2;
      while (used.has(name)) {
        name = `${base}_${idx}`;
        idx += 1;
      }
      used.add(name);
      nameByPageId.set(page.id, name);
    }

    const importPageIds = Array.from(
      new Set(normalized.routes.map((r) => r.pageId)),
    );
    const imports = importPageIds
      .map(
        (pageId) =>
          `import ${nameByPageId.get(pageId)} from './pages/${pageId}';`,
      )
      .join("\n");

    const routes = normalized.routes
      .map((r) => {
        const component = nameByPageId.get(r.pageId) || "Page";
        const pathLiteral = JSON.stringify(r.path);
        const parentRoute = resolveParentRoute(r.path);
        const parentRouteLine = parentRoute
          ? `    parentRoute: ${JSON.stringify(parentRoute)},\n`
          : "";
        return `  {\n${parentRouteLine}    path: ${pathLiteral},\n    element: <${component} />,\n  },`;
      })
      .join("\n");

    out.push({
      path: "src/routes.tsx",
      content: renderTemplate(routesTpl, {
        ROUTE_IMPORTS: imports,
        ROUTE_ENTRIES: routes,
      }),
    });
  } else {
    const routes = normalized.routes.map((r) => {
      const parentRoute = resolveParentRoute(r.path);
      return parentRoute
        ? {
            parentRoute,
            path: r.path,
            component: `./pages/${r.pageId}`,
          }
        : {
            path: r.path,
            component: `./pages/${r.pageId}`,
          };
    });
    out.push({
      path: "src/routes.ts",
      content: renderTemplate(routesTpl, {
        ROUTES: JSON.stringify(routes, null, 2),
      }),
    });
  }

  const localesIndexTpl = requiredTemplate(
    scaffoldMap,
    "src/locales/index.ts.tpl",
  );
  const localeInfos = mergedLocales.map((locale) => {
    ensureSafeFileName(locale.lang, "locale lang");
    const variableName = toIdentifier(locale.lang);
    return {
      lang: locale.lang,
      variableName,
      fileName: `${locale.lang}.json`,
    };
  });
  const varNames = new Set(localeInfos.map((l) => l.variableName));
  if (varNames.size !== localeInfos.length) {
    throw new Error("locales.lang results in duplicate identifiers");
  }

  for (const locale of mergedLocales) {
    out.push({
      path: normalizeRelPath(`src/locales/${locale.lang}.json`),
      content: JSON.stringify(locale.messages, null, 2) + "\n",
    });
  }

  const localeImports = localeInfos
    .map((l) => `import ${l.variableName} from './${l.fileName}';`)
    .join("\n");

  const localeExports = localeInfos
    .map((l) => {
      if (isValidIdentifier(l.lang)) return `  ${l.variableName},`;
      return `  '${l.lang}': ${l.variableName},`;
    })
    .join("\n");

  out.push({
    path: "src/locales/index.ts",
    content: renderTemplate(localesIndexTpl, {
      LOCALE_IMPORTS: localeImports,
      LOCALE_EXPORTS: localeExports,
    }),
  });

  const pageIndexTemplate = requiredTemplate(
    scaffoldMap,
    "src/pages/__PAGE__/index.tsx.tpl",
  );
  const pageContentTemplate = requiredTemplate(
    scaffoldMap,
    "src/pages/__PAGE__/page.tsx.tpl",
  );
  const pageComponentFile = /['"]\.\/page['"]/.test(pageIndexTemplate)
    ? "page.tsx"
    : "Page.tsx";
  for (const page of normalized.pages) {
    const pageContent = String(options.pageRenderer(page, normalized) ?? "");
    out.push({
      path: normalizeRelPath(`src/pages/${page.id}/index.tsx`),
      content: renderTemplate(pageIndexTemplate, { PAGE_ID: page.id }),
    });
    const content = pageContentTemplate.includes("__PAGE_CONTENT__")
      ? renderTemplate(pageContentTemplate, { PAGE_CONTENT: pageContent })
      : pageContent;
    out.push({
      path: normalizeRelPath(`src/pages/${page.id}/${pageComponentFile}`),
      content,
    });
  }

  return { files: out, warnings: warningsFor(options) };
}
