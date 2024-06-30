mod markdown;

use futures::{pin_mut, TryStreamExt};
use k8s_openapi::{
    api::core::v1::Node,
    apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
};
use kube::{
    api::{ListParams, Patch, PatchParams},
    runtime::{conditions, wait::await_condition, watcher, WatchStreamExt},
    Api, Client, CustomResource, CustomResourceExt, ResourceExt as _,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "tunamaguro.dev",
    version = "v1",
    kind = "Topology",
    namespaced
)]
#[kube(status = "TopologyStatus")]
struct TopologySpec {
    pub name: String,
    pub nodes: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
struct TopologyStatus {
    pub is_ok: bool,
}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "view.zoetrope.github.io",
    version = "v1",
    kind = "MarkdownView",
    namespaced
)]
struct MarkdonwViewSpec {
    pub markdowns: BTreeMap<String, String>,
    pub replicas: u32,
    #[serde(rename = "viewerImage")]
    pub viewer_image: String,
}

const CRD_NAME: &str = "topologies.tunamaguro.dev";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let client = Client::try_default().await?;

    Ok(())
}
