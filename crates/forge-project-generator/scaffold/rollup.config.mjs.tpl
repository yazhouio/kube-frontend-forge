import commonjs from '@rollup/plugin-commonjs';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import json from '@rollup/plugin-json';
import { nodeResolve } from '@rollup/plugin-node-resolve';
import terser from '@rollup/plugin-terser';
import ts from 'typescript';
import typescript from '@rollup/plugin-typescript';
import postcss from 'rollup-plugin-postcss';

const externalPackages = __EXTERNAL_PACKAGES__;
const forgeComponentsSourceEntry = fileURLToPath(
  new URL('./node_modules/@frontend-forge/forge-components/src/index.ts', import.meta.url),
);

const external = (id) => externalPackages.includes(id);
const hasStyleSideEffects = (id) => /\.(css|less|sass|scss|styl)(\?.*)?$/.test(id);
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

function replaceNodeEnv() {
  const production = JSON.stringify('production');
  const replacements = [
    ['process.env.NODE_ENV', production],
    ['process.env["NODE_ENV"]', production],
    ["process.env['NODE_ENV']", production],
  ];

  return {
    name: 'forge-replace-node-env',
    transform(code) {
      if (!code.includes('process.env')) {
        return null;
      }
      let next = code;
      for (const [from, to] of replacements) {
        next = next.replaceAll(from, to);
      }
      if (next === code) {
        return null;
      }
      return { code: next, map: null };
    },
  };
}

export default {
  input: 'src/index.ts',
  external,
  output: {
    dir: 'dist',
    format: 'system',
    entryFileNames: 'index.js',
    chunkFileNames: 'chunks/[name]-[hash].js',
    assetFileNames: 'assets/[name][extname]',
  },
  treeshake: {
    preset: 'smallest',
    moduleSideEffects: (id, external) => external || hasStyleSideEffects(id),
  },
  plugins: [
    resolveForgeComponentsSource(),
    replaceNodeEnv(),
    transpileForgeComponentsSource(),
    nodeResolve({
      browser: true,
      extensions: ['.mjs', '.js', '.jsx', '.json', '.ts', '.tsx'],
    }),
    commonjs(),
    json(),
    typescript({
      tsconfig: './tsconfig.json',
      declaration: false,
      declarationMap: false,
      sourceMap: false,
      jsx: 'react',
    }),
    postcss({
      extract: 'style.css',
      minimize: true,
    }),
    terser({
      format: {
        comments: false,
      },
    }),
  ],
};
