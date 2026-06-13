# forge-project-generator

Rust project generator for Frontend Forge.

The crate mirrors the current TypeScript generator responsibilities:

- validate and normalize an extension manifest
- copy scaffold files
- render `package.json`, `extensionConfig`, routes, locales, and page files
- merge scaffold default locales with manifest locales
- return generated `VirtualFile` values and warnings

Generated projects build output with `build.mjs`. `manifest.build.format` can be
`esm` or `systemjs`; when it is omitted, legacy `manifest.build.systemjs`
continues to select the format and defaults to `systemjs`. The build uses
esbuild for bundling. Only the `systemjs` format runs SWC to convert bundled ESM
to `System.register`, then esbuild minifies the SystemJS output. It resolves
workspace-linked `@frontend-forge/forge-components` from `src/index.ts` only
when `FORGE_DEV_MODE=true` and that source entry exists. Production builds fall
back to the package default entry.

The page body is supplied by a `PageRenderer` callback in library usage. The CLI uses a minimal placeholder page renderer, so it is mostly for testing the project-file pipeline independently from `forge-core`.

## Usage

```bash
cargo run -p forge-project-generator -- ../../examples/manifest.sample.json
cargo run -p forge-project-generator -- ../../examples/manifest.sample.json --out ../../.tmp/forge-project-generator.json
cargo test
```
