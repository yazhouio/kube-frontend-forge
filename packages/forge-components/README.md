# @frontend-forge/forge-components

React component library for the Frontend Forge pipeline.

## Usage

```tsx
import { ForgeButton } from "@frontend-forge/forge-components";

export function Demo() {
  return <ForgeButton variant="ghost">Hello</ForgeButton>;
}
```

## Runtime context

```tsx
import {
  RuntimeProvider,
  type RuntimeContextInfo,
  useRuntimeContext,
} from "@frontend-forge/forge-components";

const runtime: RuntimeContextInfo = {
  page: { id: "page-demo" },
  route: { current: "/", params: {}, query: {} },
  location: { pathname: "/", search: "", hash: "" },
  navigation: { navigate: () => {}, goBack: () => {} },
};

function Page() {
  const runtime = useRuntimeContext();
  return <div>{runtime.page.id}</div>;
}

export function PageWithRuntime() {
  return (
    <RuntimeProvider value={runtime}>
      <Page />
    </RuntimeProvider>
  );
}
```

For react-router v6 integration, use the `withPageRuntime` helper from
the project generator scaffold.

## Table cell render config

`TableTd` accepts JSON-friendly render rules through `meta`. Existing
`type/path/payload` rules are still supported, and new configs can use
`path`, `valueType`, `displayType`, `payload`, and `emptyText`.

```tsx
<TableTd
  original={row}
  meta={{
    path: "status.phase",
    valueType: "enum",
    displayType: "enum",
    payload: {
      map: {
        Running: "Running",
        Failed: "Failed",
      },
    },
  }}
/>
```

Built-in display types are `text`, `date`, `enum`, `link`, `number`,
`boolean`, `json`, and `list`. Link rules support row-data templates such as
`/projects/{metadata.namespace}/{metadata.name}`.

## Notes

- The build outputs ESM and keeps `react`/`react-dom` as peer dependencies.
- Style tokens use class names like `ff-button` for consumers to theme.
