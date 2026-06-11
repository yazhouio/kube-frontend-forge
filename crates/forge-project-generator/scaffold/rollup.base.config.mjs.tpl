import commonjs from '@rollup/plugin-commonjs';
import json from '@rollup/plugin-json';
import { nodeResolve } from '@rollup/plugin-node-resolve';
import { transformSync } from 'esbuild';
import postcss from 'rollup-plugin-postcss';

const externalPackages = __EXTERNAL_PACKAGES__;

const external = (id) => externalPackages.includes(id);
const hasStyleSideEffects = (id) => /\.(css|less|sass|scss|styl)(\?.*)?$/.test(id);
const isTypescriptSource = (id) => /\.[cm]?tsx?$/.test(id);
const loaderFor = (id) => (id.endsWith('x') ? 'tsx' : 'ts');

export function replaceNodeEnv() {
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

function esbuildTranspile() {
  return {
    name: 'forge-esbuild-transpile',
    transform(code, id) {
      if (!isTypescriptSource(id)) {
        return null;
      }
      const result = transformSync(code, {
        loader: loaderFor(id),
        target: 'es2022',
        format: 'esm',
        jsx: 'transform',
        sourcemap: false,
        sourcefile: id,
      });
      return { code: result.code, map: null };
    },
  };
}

export function esbuildMinify() {
  return {
    name: 'forge-esbuild-minify',
    renderChunk(code) {
      const result = transformSync(code, {
        loader: 'js',
        target: 'es2022',
        format: 'esm',
        minify: true,
        legalComments: 'none',
        sourcemap: false,
      });
      return { code: result.code, map: null };
    },
  };
}

export function createBaseConfig({ prePlugins = [], postPlugins = [] } = {}) {
  return {
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
      ...prePlugins,
      replaceNodeEnv(),
      nodeResolve({
        browser: true,
        extensions: ['.mjs', '.js', '.jsx', '.json', '.ts', '.tsx'],
      }),
      commonjs(),
      json(),
      esbuildTranspile(),
      postcss({
        extract: 'style.css',
        minimize: true,
      }),
      ...postPlugins,
    ],
  };
}
