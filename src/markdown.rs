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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MarkdownViewStatus {
    status: MarkdownViewStatusEnum,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum MarkdownViewStatusEnum {
    NotReady,
    Available,
    Healthy,
}
