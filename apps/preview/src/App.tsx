import { Notify } from "@kubed/components";
import { NavLink, Outlet } from "react-router-dom";

export function HomePanels() {
  return (
    <>
      <section className="grid">
        <div className="panel">
          <h2>Token-ready</h2>
          <p>Components expose class hooks for theming and system tokens.</p>
        </div>
        <div className="panel">
          <h2>Composable</h2>
          <p>Bring your own layout or wire it into the forge pipeline.</p>
        </div>
        <div className="panel">
          <h2>Fast feedback</h2>
          <p>Preview changes instantly with Vite dev server.</p>
        </div>
      </section>
      <section className="panel">
        <h2>Example routes</h2>
        <p className="panel-note">
          Use the navigation above to check the Table and CRD store samples.
        </p>
      </section>
    </>
  );
}

export function App() {
  return (
    <div className="app">
      <header className="hero">
        <p className="eyebrow">Frontend Forge Preview</p>
        <nav className="nav">
          <NavLink
            to="/"
            end
            className={({ isActive }) =>
              `nav-link${isActive ? " is-active" : ""}`
            }
          >
            Overview
          </NavLink>
          <NavLink
            to="/cluster/host/table"
            className={({ isActive }) =>
              `nav-link${isActive ? " is-active" : ""}`
            }
          >
            Cluster crd table
          </NavLink>
          <NavLink
            to="/table-render-rules"
            className={({ isActive }) =>
              `nav-link${isActive ? " is-active" : ""}`
            }
          >
            Table render rules
          </NavLink>
          <NavLink
            to="/workspaces/system-workspace/table"
            className={({ isActive }) =>
              `nav-link${isActive ? " is-active" : ""}`
            }
          >
            Workspace crd table
          </NavLink>
        </nav>
      </header>
      <Outlet />
      <Notify position="top-right" />
    </div>
  );
}
