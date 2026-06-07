# component-generator-rs

Experimental Rust rewrite of `packages/component-generator`.

This is intentionally isolated from the current TypeScript packages. The goal is to validate the Rust generator and parser/backend shape before wiring it into `forge-core` or the server.

## Current Scope

- Reads a `PageConfig` JSON payload.
- Registers built-in node/data source definitions at process startup.
- Keeps node definitions in a `NodeSource` structure that mirrors the TypeScript `NodeDefinition` shape: `id`, `schema.templateInputs`, `generateCode.imports`, `generateCode.stats`, `generateCode.jsx`, and `generateCode.meta.inputPaths`.
- Keeps the generator IR backend-neutral: node/data source templates render to string-based fragments, while AST operations are hidden behind `JsCodeBackend`.
- Ships a default Oxc-backed `JsCodeBackend` for parsing snippets, renaming identifiers, extracting imports, validating module items, and emitting TSX.
- Provides an optional SWC-backed `JsCodeBackend` behind the `swc` Cargo feature. The generator IR does not expose SWC or Oxc AST types.
- Uses `snafu` for structured errors.
- Emits TSX code as a string.
- Merges common namespace/named imports structurally while still allowing `NodeSource.generateCode.imports` to use TypeScript-style import strings.
- Orders `generateCode.stats` by `depends` and reports missing/cyclic stat dependencies as structured errors.
- Emits scoped component nodes as functions and inlines non-scoped child nodes into their nearest scoped parent.
- Supports simple function body stats for built-in nodes, including `Text` node `useState`.
- Supports `StatementScope` placement for node and data source stats. `module.import` stats are extracted into the import registry, module scopes are emitted outside component functions, and function/block/control/jsx scopes are emitted inside the component boundary.
- Allocates local stat output names per component boundary, so repeated inline nodes do not generate duplicate declarations such as `text/setText`.
- Renames inline child stat references in both generated statements and JSX when output names collide.
- Allocates module-level data source stat output names, so repeated data source templates do not generate duplicate declarations such as `useStore`.
- Passes declared `runtimeProps` from scoped child nodes to their generated component calls, including binding expressions resolved in the parent boundary.
- Supports `generateCode.meta.runtimeDeps`, including automatic `useRuntimeContext` injection when a node declares a runtime dependency.
- Resolves basic `dataSource` bindings from `source/path/defaultValue` into generated output variables, including output-specific names such as `colsColumns`.
- Emits data source hook destructuring inside scoped components only for outputs that are actually bound.
- Supports data source `args` and orders hook destructuring so argument dependencies are initialized before dependent hooks.
- Resolves basic `runtime` bindings and injects `useRuntimeContext` plus `const __runtime__ = useRuntimeContext();` in the owning scoped component.
- Orders runtime initialization before data source hooks whose args read runtime values.
- Provides a `DataSourceSource` structure for TypeScript-style data source templates. Registered data sources now use this path, including identifier replacement for `HOOK_NAME` and template defaults such as `PAGE_ID`, `SCOPE`, and `CRD_CONFIG`.
- Supports a basic `actionGraphs` pipeline: parses graph schemas, injects event handler props such as `ON_CLICK`, creates per-graph context stores, and emits dispatch code for `assign`, `reset`, `navigate`, `goBack`, and `callDataSource`. `callDataSource` now distinguishes `static` set mode, `rest` request mode, and a generic mutate fallback for other data source types.
- Supports action graph context bindings in node props, for example `target: "context", source: "formGraph", path: "name"`.
- Keeps explicit node event props authoritative when action graph handlers target the same prop.
- Reports ambiguous binding sources when the same id exists as both a data source and an action graph and the binding does not specify `target`.
- Performs JSON Schema validation for declared node/data source template inputs via the Rust `jsonschema` crate. The generated prop schema follows the TypeScript behavior by allowing `binding` and `expression` values in addition to the declared data type.

Implemented nodes:

- `Layout`
- `Text`
- `Iframe`
- `CrdTable`

Registered data sources:

- `static`
- `rest`
- `crd-columns`
- `crd-page-state`
- `workspace-crd-page-state`

Full dependency graph parity with the TypeScript implementation is not complete yet. Render boundary semantics are represented by `meta.scope` nodes becoming functions, while non-scoped children are inlined into the nearest scoped parent with output-name collision handling. Data source and runtime usage is collected from node bindings, data source args, runtime props, and action graph `callDataSource` steps. The current action graph implementation covers the important `static`/`rest` request modes and keeps a mutate fallback for CRD-style data sources. Generated formatting intentionally follows the active `JsCodeBackend` output instead of trying to match Babel formatting exactly.

## Usage

From this directory:

```bash
cargo run -- ../../apps/server/examples/page.schema.json
cargo run -- ../../apps/server/examples/page.schema.json --out SamplePage.tsx
cargo run --features swc -- ../../apps/server/examples/page.schema.json --backend swc
cargo test
cargo test --features swc
```

The CLI accepts either a direct page schema or an object with `pageSchema`.
It uses the Oxc backend by default. Pass `--backend swc` with `--features swc` to run the SWC backend. Pass `--out <file>` to write generated TSX to a file instead of stdout. Unknown flags, missing flag values, and extra input paths are rejected instead of being ignored.

The default generator uses Oxc:

```rust
let code = ComponentGenerator::default().generate_page_code(&page)?;
```

The Oxc backend can also be selected explicitly:

```rust
let generator = ComponentGenerator::with_backend(default_registry(), OxcCodeBackend);
let code = generator.generate_page_code(&page)?;
```

The SWC backend is available only when the crate is built with `--features swc`:

```rust
let generator = ComponentGenerator::with_backend(default_registry(), SwcCodeBackend);
let code = generator.generate_page_code(&page)?;
```
