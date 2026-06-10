use serde::Serialize;

#[derive(Clone, Copy, Debug)]
pub struct PackageDependency {
    pub name: &'static str,
    pub version: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildConfigSummary {
    pub external_packages: Vec<&'static str>,
    pub minify: bool,
    pub tree_shaking: TreeShakingSummary,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeShakingSummary {
    pub enabled: bool,
    pub preset: &'static str,
    pub preserve_external_side_effects: bool,
    pub preserve_style_side_effects: bool,
}

pub const DEPENDENCIES: &[PackageDependency] = &[
    PackageDependency {
        name: "@frontend-forge/forge-components",
        version: "^0.1.0",
    },
    PackageDependency {
        name: "@ks-console/shared",
        version: "4.2.1",
    },
    PackageDependency {
        name: "@kubed/charts",
        version: "^0.2.35",
    },
    PackageDependency {
        name: "@kubed/code-editor",
        version: "^0.2.35",
    },
    PackageDependency {
        name: "@kubed/components",
        version: "^0.2.35",
    },
    PackageDependency {
        name: "@kubed/hooks",
        version: "^0.2.35",
    },
    PackageDependency {
        name: "@kubed/icons",
        version: "^0.2.35",
    },
    PackageDependency {
        name: "@tanstack/react-table",
        version: "^8.21.3",
    },
    PackageDependency {
        name: "es-toolkit",
        version: "^1.43.0",
    },
    PackageDependency {
        name: "js-yaml",
        version: "^3.13.1",
    },
    PackageDependency {
        name: "qs",
        version: "6.14.1",
    },
    PackageDependency {
        name: "react",
        version: "^17.0.2",
    },
    PackageDependency {
        name: "react-dom",
        version: "^17.0.2",
    },
    PackageDependency {
        name: "react-query",
        version: "^3.32.1",
    },
    PackageDependency {
        name: "react-router-dom",
        version: "^6.22.3",
    },
    PackageDependency {
        name: "semver",
        version: "^7.7.3",
    },
    PackageDependency {
        name: "styled-components",
        version: "5.3.3",
    },
    PackageDependency {
        name: "swr",
        version: "^2.3.8",
    },
    PackageDependency {
        name: "zustand",
        version: "^4.5.5",
    },
];

pub const EXTERNAL_PACKAGES: &[&str] = &[
    "@ks-console/shared",
    "@kubed/code-editor",
    "@kubed/components",
    "@kubed/icons",
    "react",
    "react-dom",
    "react-query",
    "react-router-dom",
    "styled-components",
];

pub const MINIFY_ENABLED: bool = true;
pub const TREE_SHAKING_PRESET: &str = "smallest";

pub fn build_config_summary() -> BuildConfigSummary {
    BuildConfigSummary {
        external_packages: EXTERNAL_PACKAGES.to_vec(),
        minify: MINIFY_ENABLED,
        tree_shaking: TreeShakingSummary {
            enabled: true,
            preset: TREE_SHAKING_PRESET,
            preserve_external_side_effects: true,
            preserve_style_side_effects: true,
        },
    }
}
