# @frontend-forge/vendor

Vendored runtime dependencies (React ecosystem helpers, polyfills, etc.) stored under `packages/vendor` and deployed separately for the server runtime.

Install with:
```bash
pnpm --config.inject-workspace-packages=true --filter @frontend-forge/vendor deploy --prod apps/server/vendor
```

Used by the SystemJS build step to locate `node_modules` for externals without pulling them into the generated bundle.
