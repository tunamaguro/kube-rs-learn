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

const CRD_NAME: &str = "topologies.tunamaguro.dev";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let client = Client::try_default().await?;

    let top_apply = PatchParams::apply("toplogy_apply").force();
    let crd_client: Api<CustomResourceDefinition> = Api::all(client.clone());

    crd_client
        .patch(CRD_NAME, &top_apply, &Patch::Apply(Topology::crd()))
        .await?;

    tracing::info!("Creating CRD: {}", serde_yaml::to_string(&Topology::crd())?);

    let establish = await_condition(crd_client, CRD_NAME, conditions::is_crd_established());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), establish).await?;

    let nodes: Api<Node> = Api::all(client.clone());

    let spec = create_spec(nodes).await;

    let topologies: Api<Topology> = Api::default_namespaced(client.clone());
    let tt = topologies
        .patch(
            "default",
            &top_apply,
            &Patch::Apply(&Topology::new("default", spec)),
        )
        .await?;

    tracing::info!("Applied 1 {}: {:?}", tt.name_any(), tt.spec);

    let obs = watcher(topologies, watcher::Config::default()).applied_objects();
    pin_mut!(obs);
    while let Some(o) = obs.try_next().await? {
        match o {
            _node => {
                let nodes: Api<Node> = Api::all(client.clone());

                let spec = create_spec(nodes).await;
                let topologies: Api<Topology> = Api::default_namespaced(client.clone());
                topologies
                    .patch(
                        "default",
                        &top_apply,
                        &Patch::Apply(&Topology::new("default", spec)),
                    )
                    .await?;
            }
        }
    }

    Ok(())
}

async fn create_spec(nodes: Api<Node>) -> TopologySpec {
    let node_list = nodes.list(&ListParams::default()).await.unwrap();
    let mut node_names = node_list
        .into_iter()
        .map(|node| node.metadata.name)
        .collect::<Option<Vec<_>>>()
        .unwrap();
    TopologySpec {
        name: "default".to_string(),
        nodes: node_names,
    }
}
