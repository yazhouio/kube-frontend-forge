import commonjs from '@rollup/plugin-commonjs';
import json from '@rollup/plugin-json';
import { nodeResolve } from '@rollup/plugin-node-resolve';
import typescript from '@rollup/plugin-typescript';
import postcss from 'rollup-plugin-postcss';

const external = [
  'react',
  'react-dom',
  'react-router-dom',
  'react-query',
  'styled-components',
  '@ks-console/shared',
  '@kubed/charts',
  '@kubed/code-editor',
  '@kubed/components',
  '@kubed/hooks',
  '@kubed/icons',
  'posthog-js',
  'wujie-react',
];

export default {
  input: 'src/index.ts',
  external,
  output: {
    dir: 'dist',
    format: 'system',
    entryFileNames: 'index.js',
    chunkFileNames: 'chunks/[name]-[hash].js',
    assetFileNames: 'assets/[name][extname]',
    name: __MODULE_NAME_JSON__,
  },
  treeshake: true,
  plugins: [
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
  ],
};
