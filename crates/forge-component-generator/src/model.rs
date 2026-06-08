use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

use crate::error::{Error, Result};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageEnvelope {
    pub page_schema: Option<PageConfig>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageConfig {
    pub meta: PageMeta,
    #[serde(default)]
    pub data_sources: Vec<DataSourceNode>,
    #[serde(default)]
    pub action_graphs: Vec<ActionGraphSchema>,
    pub root: ComponentNode,
    #[serde(default)]
    pub context: Value,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PageMeta {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSourceNode {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub config: BTreeMap<String, Value>,
    #[serde(default)]
    pub args: Vec<Value>,
    #[serde(default)]
    pub auto_load: Option<bool>,
    #[serde(default)]
    pub polling: Option<Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ComponentNode {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub props: BTreeMap<String, Value>,
    #[serde(default)]
    pub meta: Option<ComponentMeta>,
    #[serde(default)]
    pub children: Vec<ComponentNode>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ComponentMeta {
    #[serde(default)]
    pub scope: bool,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionGraphSchema {
    pub id: String,
    #[serde(default)]
    pub context: Value,
    #[serde(default)]
    pub actions: BTreeMap<String, ActionNode>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ActionNode {
    pub on: String,
    #[serde(rename = "do", default)]
    pub steps: Vec<ActionStep>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ActionStep {
    #[serde(rename = "assign")]
    Assign { to: String, value: String },
    #[serde(rename = "callDataSource")]
    CallDataSource {
        id: String,
        #[serde(default)]
        args: Vec<String>,
    },
    #[serde(rename = "reset")]
    Reset { path: String },
    #[serde(rename = "navigate")]
    Navigate { path: String },
    #[serde(rename = "goBack")]
    GoBack,
}

pub fn unwrap_page_schema(value: Value) -> Result<PageConfig> {
    if value.get("pageSchema").is_some() {
        let envelope: PageEnvelope =
            serde_json::from_value(value).map_err(|source| Error::JsonValue { source })?;
        envelope.page_schema.ok_or(Error::MissingPageSchema)
    } else {
        serde_json::from_value(value).map_err(|source| Error::JsonValue { source })
    }
}
