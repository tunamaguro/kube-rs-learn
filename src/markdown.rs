use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "view.zoetrope.github.io",
    version = "v1",
    kind = "MarkdownView",
    namespaced
)]
#[kube(status = "MarkdownViewStatus")]
pub struct MarkdonwViewSpec {
    pub markdowns: BTreeMap<String, String>,
    pub replicas: u32,
    #[serde(rename = "viewerImage")]
    pub viewer_image: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, PartialOrd, Serialize, Deserialize, JsonSchema)]
pub struct MarkdownViewStatus {
    pub is_ok: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum MarkdownViewStatusEnum {
    #[default]
    NotReady,
    Available,
    Healthy,
}
