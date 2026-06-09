# @frontend-forge/code-export

SystemJS build pipeline (esbuild + SWC + Tailwind) as a reusable package.

## What it does

- Bundles TS/TSX/JS/CSS into a single SystemJS module.
- Optionally runs Tailwind v4 to emit a standalone CSS file.
- Enforces output safety (must contain `System.register`, forbids webpack tokens).

## Basic Usage

```ts
import { buildOnce } from '@frontend-forge/code-export';

const result = await buildOnce({
  files: [
    { path: 'src/index.tsx', content: 'export default function App() {}' },
  ],
  entry: 'src/index.tsx',
  externals: ['react', 'react-dom'],
  tailwind: { enabled: false },
});

console.log(result.js.path); // index.js
```

## Exports

- `buildOnce({ files, entry, externals, tailwind?, buildTimeoutMs?, childMaxOldSpaceMb?, vendorNodeModules?, rootNodeModules?, workDir? })`
  - Returns `{ js, css, meta }`, rejects if forbidden tokens or missing `System.register`.
- `buildVirtualFiles({ files, entry, externals, ... })`
  - Returns `{ files, meta }` where `files` is a `VirtualFile[]`.
- `CodeExporter`
  - Optional cache/scheduler wrapper around `buildOnce`, suitable for server usage.
- Helpers: `computeBuildKey`, `ALLOWED_FILE_RE`, `safeJoin`, `mkWorkDir`, `rmWorkDir`, `nowMs`, `binPath`, `sha256`.

## CodeExporter (Cache + Scheduler)

```ts
import { CodeExporter } from '@frontend-forge/code-export';

const exporter = new CodeExporter({
  defaultEntry: 'src/index.tsx',
  defaultExternals: ['react', 'react-dom'],
});

const { outputs, meta } = await exporter.build({
  files: [{ path: 'src/index.tsx', content: 'export default () => null' }],
});
```

The cache interface expects `{ hit, value }` so callers can differentiate
`memory`/`disk` hits and keep `queuedMs` timing data.

## Tailwind

```ts
tailwind: {
  enabled: true,
  input: 'src/index.css',
  config: 'tailwind.config.js'
}
```

- `input` defaults to `src/index.css`.
- If `config` is provided and exists, it is passed to `tailwindcss -c`.
- Tailwind runs in a temp workdir; the CLI is resolved from repo dependencies.

## Constraints & Safety

- Allowed file extensions: `ts|tsx|js|jsx|css|json` (`ALLOWED_FILE_RE`).
- Absolute paths and `..` are rejected (`safeJoin`).
- Output must include `System.register`.
- Forbidden tokens: `__webpack_require__`, `__webpack_exports__`, `webpackChunk`, `import(`.

## Resolution of `node_modules`

- `vendorNodeModules` is resolved by checking the provided path first, then falling back to legacy `packages/vendor/node_modules`, `vendor/node_modules`, and finally `node_modules`.
- This package is retained for legacy TS build helpers. The production build path now uses the Rust `apps/forge-job` CLI and generated Rollup projects instead of an HTTP server.
- If `workDir` is not provided, a temp dir is created and cleaned automatically.
