import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
import { createBaseConfig } from './rollup.base.config.mjs';

const forgeComponentsSourceEntry = fileURLToPath(
  new URL('./node_modules/@frontend-forge/forge-components/src/index.ts', import.meta.url),
);

const isTypescriptSource = (id) => /\.[cm]?tsx?$/.test(id);
const isForgeComponentsSource = (id) => {
  const normalized = id.replaceAll('\\', '/');
  return (
    normalized.includes('/node_modules/@frontend-forge/forge-components/src/') ||
    normalized.includes('/packages/forge-components/src/')
  );
};

function resolveForgeComponentsSource() {
  const useSource = existsSync(forgeComponentsSourceEntry);

  return {
    name: 'forge-components-source',
    resolveId(id) {
      if (useSource && id === '@frontend-forge/forge-components') {
        return forgeComponentsSourceEntry;
      }
      return null;
    },
  };
}

function transpileForgeComponentsSource() {
  return {
    name: 'forge-components-source-transpile',
    transform(code, id) {
      if (!isTypescriptSource(id) || !isForgeComponentsSource(id)) {
        return null;
      }
      const result = ts.transpileModule(code, {
        fileName: id,
        compilerOptions: {
          target: ts.ScriptTarget.ES2022,
          module: ts.ModuleKind.ESNext,
          moduleResolution: ts.ModuleResolutionKind.Bundler,
          jsx: ts.JsxEmit.React,
          esModuleInterop: true,
          resolveJsonModule: true,
          importHelpers: true,
        },
      });
      return { code: result.outputText, map: null };
    },
  };
}

export default createBaseConfig({
  prePlugins: [resolveForgeComponentsSource(), transpileForgeComponentsSource()],
});
