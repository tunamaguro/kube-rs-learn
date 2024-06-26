use k8s_openapi::api::core::v1::Pod;
use kube::{api::ListParams, Api, Client, ResourceExt as _};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let client = Client::try_default().await?;

    let pods: Api<Pod> = Api::default_namespaced(client);

    for p in pods.list(&ListParams::default()).await? {
        println!("found pod {}", p.name_any());
    }
    Ok(())
}
