use k8s_openapi::api::core::v1::ConfigMap;
use std::{error::Error, sync::Arc};

use kube::{
    api::{Patch, PatchParams},
    runtime::controller::Action,
    Api, Client, ResourceExt,
};

use crate::MarkdownView;

pub struct Context {
    pub client: Client,
}

async fn reconcile(obj: Arc<MarkdownView>, ctx: Arc<Context>) -> Result<Action, Box<dyn Error>> {
    let ns = obj.namespace().unwrap();
    let md_views: Api<MarkdownView> = Api::namespaced(ctx.client.clone(), &ns);
    let md_view = md_views.get(&obj.name_any()).await?;

    // reconcileConfigMap
    let cm_api: Api<ConfigMap> = Api::all(ctx.client.clone());

    let cm_name = format!("markdowns-{}", obj.name_any());
    let cm_pp = PatchParams::apply(&format!("{}-apply", cm_name));
    let cm_patch = serde_json::json!({
        "data":obj.spec.markdowns
    });
    let cm = cm_api
        .patch(&cm_name, &cm_pp, &Patch::Apply(&cm_patch))
        .await?;

    Ok(Action::await_change())
}
