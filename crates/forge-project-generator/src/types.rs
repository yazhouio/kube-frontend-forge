use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteMeta {
    pub path: String,
    pub page_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuMeta {
    pub parent: String,
    pub name: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_workspace_auth: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LocaleMeta {
    pub lang: String,
    pub messages: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageMeta {
    pub id: String,
    pub entry_component: String,
    pub components_tree: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildMeta {
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub systemjs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<BuildFormat>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildFormat {
    Esm,
    Systemjs,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionManifest {
    pub version: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub routes: Vec<RouteMeta>,
    pub menus: Vec<MenuMeta>,
    pub locales: Vec<LocaleMeta>,
    pub pages: Vec<PageMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildMeta>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ManifestEnvelope {
    pub manifest: ExtensionManifest,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct VirtualFile {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Debug, Default)]
pub struct GenerateProjectFilesOptions {
    pub build: bool,
    pub archive: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct GenerateProjectFilesResult {
    pub files: Vec<VirtualFile>,
    pub warnings: Vec<String>,
}
