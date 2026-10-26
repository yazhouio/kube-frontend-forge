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

## Menu metadata

Each `manifest.menus[]` entry supports `parent`, `name`, `title`, and optional
`icon`, `order`, `clusterModule`, and `workspaceModule`. Unknown fields are
rejected by the manifest schema. Normalization always sets
`skipWorkspaceAuth: true`.

`clusterModule` and `workspaceModule` hold a KubeSphere extension name. The
console reads `ksConfig.enabledExtensionModulesStatus[<name>].clusterSchedulingStatuses`
and hides the menu where the extension is not installed:

- `clusterModule` (menus under `cluster`): shown only on clusters where the
  extension is installed.
- `workspaceModule` (menus under `workspace`): shown only when at least one of
  the workspace's clusters has the extension installed.

When both fields are omitted, the menu is shown everywhere, as before. The
frontend-forge controller sets them for FrontendExtension packages only, so
controller releases that emit `workspaceModule` require a frontend-forge
release that accepts it.

The page body is supplied by a `PageRenderer` callback in library usage. The CLI uses a minimal placeholder page renderer, so it is mostly for testing the project-file pipeline independently from `forge-core`.

## Usage

```bash
cargo run -p forge-project-generator -- ../../examples/manifest.sample.json
cargo run -p forge-project-generator -- ../../examples/manifest.sample.json --out ../../.tmp/forge-project-generator.json
cargo test
```
