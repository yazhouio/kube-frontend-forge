# @frontend-forge/project-generator

Generates a KubeSphere extension project from a manifest plus a page renderer.

## Highlights

- Validates and normalizes manifest fields.
- Renders the `scaffold/` template into a project file set.
- Uses a `pageRenderer(page, manifest)` callback to inject per-page TSX.
- Returns `VirtualFile[]` for in-memory generation (no filesystem writes).

## Usage

```ts
import { generateProjectFiles } from '@frontend-forge/project-generator';

const { files, warnings } = generateProjectFiles(manifest, {
  pageRenderer: (page) => `export default function ${page.entryComponent}() { return null; }`,
  onLog: (msg) => console.log(msg),
});
```

You can also use the facade class:

```ts
import { ProjectGenerator } from '@frontend-forge/project-generator';

const generator = new ProjectGenerator();
const result = generator.generateProjectFiles(manifest, { pageRenderer });
```

## Manifest Shape

```json
{
  "version": "1.0",
  "name": "demo",
  "routes": [{ "path": "/demo", "pageId": "DemoPage" }],
  "menus": [{ "parent": "root", "name": "demo", "title": "Demo" }],
  "locales": [{ "lang": "en", "messages": { "HELLO": "Hello" } }],
  "pages": [
    { "id": "DemoPage", "entryComponent": "DemoPage", "componentsTree": {} }
  ],
  "build": { "target": "kubesphere-extension", "moduleName": "demo", "systemjs": true }
}
```

Validation rules (non-exhaustive):

- `version` must be `"1.0"`.
- `name` must be a non-empty string.
- `routes[*].path` and `routes[*].pageId` are required.
- `menus[*].parent`, `menus[*].name`, `menus[*].title` are required.
- `locales[*].lang` must be unique; `messages` values must be strings.
- `pages[*].id` must be unique; `routes[*].pageId` must exist in `pages`.

## Templates & Scaffold

Templates live under `scaffold/` and are copied into the output set.
Template placeholders are replaced at generation time:

- `package.json.tpl`: `__NAME__`, `__VERSION__`
- `src/extensionConfig.ts.tpl`: `__MENUS__`
- `src/routes.tsx.tpl`: `__ROUTE_IMPORTS__`, `__ROUTE_ENTRIES__`
- `src/locales/index.ts.tpl`: `__LOCALE_IMPORTS__`, `__LOCALE_EXPORTS__`
- `src/pages/__PAGE__/index.tsx.tpl`: `__PAGE_CONTENT__`

Keep the `scaffold/` folder when publishing the package.

## Warnings

Options `build`/`archive` are stubs in this package and return warnings only.
They are reserved for higher-level orchestration in `forge-core`.

## Notes

- Writing files to disk is handled by the caller.
- The production path now uses the Rust `crates/forge-project-generator` and
  `apps/forge-job`; this TS package remains for legacy compatibility.
