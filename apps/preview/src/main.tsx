import React from "react";
import ReactDOM from "react-dom";
import { createBrowserRouter, RouterProvider } from "react-router-dom";
import { CssBaseline, KubedConfigProvider } from "@kubed/components";
import { QueryClient, QueryClientProvider } from "react-query";
import { init } from "@frontend-forge/forge-components";
import { App, HomePanels } from "./App";
import { TablePreview } from "./TablePreview";
import { TableRenderRulePreview } from "./TableRenderRulePreview";
import { WorkspaceTablePreview } from "./WorkspaceTablePreview";
import "./index.css";

init();

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      staleTime: 100 * 5,
      retry: false,
    },
  },
});
const win = window as typeof window & { t?: (label: string) => string };

const root = document.getElementById("root");
if (!root) {
  throw new Error("Root element not found");
}

win.t = (label) => label;

const router = createBrowserRouter([
  {
    path: "/",
    element: <App />,
    children: [
      { index: true, element: <HomePanels /> },
      { path: "table", element: <TablePreview /> },
      {
        path: "/cluster/:cluster/table",
        element: <TablePreview />,
      },
      {
        path: "/table-render-rules",
        element: <TableRenderRulePreview />,
      },
      {
        path: "/workspaces/:workspace/table",
        element: <WorkspaceTablePreview />,
      },
    ],
  },
]);

ReactDOM.render(
  <QueryClientProvider contextSharing={true} client={queryClient}>
    <React.StrictMode>
      <KubedConfigProvider>
        <CssBaseline />
        <RouterProvider router={router} />
      </KubedConfigProvider>
    </React.StrictMode>
  </QueryClientProvider>,
  root,
);
