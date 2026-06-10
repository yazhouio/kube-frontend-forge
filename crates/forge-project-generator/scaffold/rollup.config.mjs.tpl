import commonjs from '@rollup/plugin-commonjs';
import json from '@rollup/plugin-json';
import { nodeResolve } from '@rollup/plugin-node-resolve';
import terser from '@rollup/plugin-terser';
import typescript from '@rollup/plugin-typescript';
import postcss from 'rollup-plugin-postcss';

const externalPackages = [
  '@ks-console/shared',
  '@kubed/code-editor',
  '@kubed/components',
  '@kubed/icons',
  'react',
  'react-dom',
  'react-query',
  'react-router-dom',
  'styled-components',
];

const external = (id) => externalPackages.includes(id);
const hasStyleSideEffects = (id) => /\.(css|less|sass|scss|styl)(\?.*)?$/.test(id);

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
    replaceNodeEnv(),
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
