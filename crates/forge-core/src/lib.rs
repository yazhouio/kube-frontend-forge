mod error;

use forge_component_generator::{
    ComponentGenerator, builtins::default_registry, component_tree_schema,
    data_source_source_schema, node_source_schema, unwrap_page_schema,
};
use forge_project_generator::{
    ExtensionManifest, GenerateProjectFilesOptions, GenerateProjectFilesResult, PageMeta,
    VirtualFile, generate_project_files, manifest_schema,
};
use serde::Serialize;
use serde_json::Value;

pub use error::{Error, Result};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildPlan {
    pub manifest_name: String,
    pub module_name: Option<String>,
    pub entry: String,
    pub files: Vec<VirtualFile>,
    pub expectations: BuildExpectations,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildExpectations {
    pub systemjs: bool,
    pub dist_dir: String,
}

pub struct ForgeCore {
    component_generator: ComponentGenerator,
}

impl Default for ForgeCore {
    fn default() -> Self {
        Self::new()
    }
}

impl ForgeCore {
    pub fn try_new() -> Result<Self> {
        Ok(Self {
            component_generator: ComponentGenerator::try_with_backend(
                default_registry(),
                forge_component_generator::OxcCodeBackend,
            )?,
        })
    }

    pub fn new() -> Self {
        Self::try_new().expect("forge core component generator registry must be valid")
    }

    pub fn generate_page_code(&self, page_schema: Value) -> Result<String> {
        let page = unwrap_page_schema(page_schema)?;
        self.component_generator
            .generate_page_code(&page)
            .map_err(Error::from)
    }

    pub fn component_tree_schema(&self) -> Value {
        component_tree_schema(self.component_generator.registry())
    }

    pub fn node_source_schema(&self) -> Value {
        node_source_schema()
    }

    pub fn data_source_source_schema(&self) -> Value {
        data_source_source_schema()
    }

    pub fn manifest_schema(&self) -> Value {
        manifest_schema(self.component_tree_schema())
    }

    pub fn generate_project_files(
        &self,
        manifest: &ExtensionManifest,
    ) -> Result<GenerateProjectFilesResult> {
        let result = generate_project_files(
            manifest,
            |page, _manifest| self.render_manifest_page(page),
            GenerateProjectFilesOptions::default(),
        )?;
        Ok(result)
    }

    pub fn create_build_plan(&self, manifest: &ExtensionManifest) -> Result<BuildPlan> {
        let result = self.generate_project_files(manifest)?;
        Ok(BuildPlan {
            manifest_name: manifest.name.clone(),
            module_name: manifest
                .build
                .as_ref()
                .and_then(|build| build.module_name.clone()),
            entry: "src/index.ts".to_owned(),
            expectations: BuildExpectations {
                systemjs: manifest
                    .build
                    .as_ref()
                    .and_then(|build| build.systemjs)
                    .unwrap_or(true),
                dist_dir: "dist".to_owned(),
            },
            files: result.files,
            warnings: result.warnings,
        })
    }

    fn render_manifest_page(&self, page: &PageMeta) -> forge_project_generator::Result<String> {
        self.generate_page_code(page.components_tree.clone())
            .map_err(|source| forge_project_generator::Error::RenderPage {
                page_id: page.id.clone(),
                message: source.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_project_generator::unwrap_manifest;

    fn sample_manifest() -> ExtensionManifest {
        unwrap_manifest(serde_json::json!({
            "manifest": {
                "version": "1.0",
                "name": "ff-test",
                "routes": [{ "path": "/sample", "pageId": "SamplePage" }],
                "menus": [],
                "locales": [{ "lang": "en", "messages": { "HELLO": "Hello" } }],
                "pages": [{
                    "id": "SamplePage",
                    "entryComponent": "SamplePage",
                    "componentsTree": {
                        "meta": { "id": "page-1", "name": "Sample", "path": "/sample" },
                        "root": {
                            "id": "root",
                            "meta": { "scope": true, "title": "SamplePage" },
                            "type": "Layout",
                            "props": { "TEXT": "Hello" },
                            "children": [
                                {
                                    "id": "child",
                                    "type": "Text",
                                    "props": { "TEXT": "World", "DEFAULT_VALUE": 1 }
                                }
                            ]
                        },
                        "context": {}
                    }
                }],
                "build": {
                    "target": "kubesphere-extension",
                    "moduleName": "ff-test",
                    "systemjs": true
                }
            }
        }))
        .unwrap()
    }

    #[test]
    fn create_build_plan_generates_project_files_without_executing_build() {
        let core = ForgeCore::new();
        let plan = core.create_build_plan(&sample_manifest()).unwrap();

        assert_eq!(plan.entry, "src/index.ts");
        assert_eq!(plan.expectations.dist_dir, "dist");
        assert!(plan.expectations.systemjs);
        assert!(
            plan.files
                .iter()
                .any(|file| file.path == "rollup.config.mjs")
        );
        assert!(
            plan.files
                .iter()
                .any(|file| file.path == "src/pages/SamplePage/Page.tsx"
                    && file.content.contains("function SamplePage"))
        );
    }
}
