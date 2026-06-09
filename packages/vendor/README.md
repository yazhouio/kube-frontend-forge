# @frontend-forge/vendor

Legacy vendored runtime dependency package.

The previous HTTP server runtime used this package to deploy a server-local
`vendor/node_modules`. The production path is now the Rust `apps/forge-job`
pipeline, so this package is retained only for compatibility with legacy TS
packages while the runtime is migrated.
