use k8s_openapi::{
    api::{
        apps::v1::{Deployment, DeploymentSpec},
        core::v1::{
            ConfigMap, ConfigMapVolumeSource, Container, ContainerPort, HTTPGetAction, PodSpec,
            PodTemplateSpec, Probe, Volume, VolumeMount,
        },
    },
    apimachinery::pkg::{apis::meta::v1::LabelSelector, util::intstr::IntOrString},
};
use std::{error::Error, sync::Arc};

use kube::{
    api::{ObjectMeta, Patch, PatchParams},
    runtime::controller::Action,
    Api, Client, ResourceExt,
};

use crate::MarkdownView;

pub struct Context {
    pub client: Client,
}

const CONTROLLER_NAME: &str = "markdown-manager";

async fn reconcile_configmap(
    obj: Arc<MarkdownView>,
    ctx: Arc<Context>,
) -> Result<(), Box<dyn Error>> {
    let cm_api: Api<ConfigMap> = Api::namespaced(ctx.client.clone(), &obj.namespace().unwrap());

    let cm_name = format!("markdowns-{}", obj.name_any());
    let cm_pp = PatchParams::apply(CONTROLLER_NAME);
    let cm_patch = serde_json::json!({
        "data":obj.spec.markdowns
    });
    let cm = cm_api
        .patch(&cm_name, &cm_pp, &Patch::Apply(&cm_patch))
        .await?;
    Ok(())
}

async fn reconcile_deployment(
    obj: Arc<MarkdownView>,
    ctx: Arc<Context>,
) -> Result<(), Box<dyn Error>> {
    let dep_name = format!("viewer-{}", obj.name_any());

    let viewer_image = obj
        .spec
        .viewer_image
        .as_deref()
        .unwrap_or("peaceiris/mdbook:latest");

    let labels = serde_json::json!({
        "app.kubernetes.io/name":"mdbook",
        "app.kubernetes.io/instance":obj.name_any(),
        "app.kubernetes.io/created-by": "markdown-view-controller"
    });
    let probe_http = HTTPGetAction {
        port: IntOrString::String("http".to_string()),
        path: Some("/".to_string()),
        ..Default::default()
    };
    let cm_name = format!("markdowns-{}", obj.name_any());
    let deployment = Deployment {
        metadata: ObjectMeta {
            name: Some(dep_name),
            labels: Some(serde_json::from_value(labels.clone())?),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(obj.spec.replicas as i32),
            selector: LabelSelector {
                match_labels: Some(serde_json::from_value(labels.clone())?),
                match_expressions: None,
            },
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(serde_json::from_value(labels.clone())?),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: "mdbook".to_string(),
                        image: Some(viewer_image.to_string()),
                        command: Some(vec!["mdbook".to_string()]),
                        args: Some(vec![
                            "serve".to_string(),
                            "--hostname".to_string(),
                            "0.0.0.0".to_string(),
                        ]),
                        image_pull_policy: Some("IfNotPresent".to_string()),

                        volume_mounts: Some(vec![VolumeMount {
                            name: "markdowns".to_string(),
                            mount_path: "/book/src".to_string(),
                            ..Default::default()
                        }]),
                        ports: Some(vec![ContainerPort {
                            name: Some("http".to_string()),
                            protocol: Some("TCP".to_string()),
                            container_port: 3000,
                            ..Default::default()
                        }]),
                        liveness_probe: Some(Probe {
                            http_get: Some(probe_http.clone()),
                            ..Default::default()
                        }),
                        readiness_probe: Some(Probe {
                            http_get: Some(probe_http.clone()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],

                    volumes: Some(vec![Volume {
                        name: "markdowns".to_string(),
                        config_map: Some(ConfigMapVolumeSource {
                            name: Some(cm_name.to_string()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }]),
                    ..Default::default()
                }),
                ..Default::default()
            },

            ..Default::default()
        }),
        ..Default::default()
    };

    let dep_api: Api<Deployment> = Api::namespaced(ctx.client.clone(), &obj.namespace().unwrap());
    let dep_patch = dep_api
        .patch(
            deployment.metadata.name.as_deref().unwrap(),
            &PatchParams::apply(CONTROLLER_NAME),
            &Patch::Apply(&deployment),
        )
        .await?;
    Ok(())
}

async fn reconcile(obj: Arc<MarkdownView>, ctx: Arc<Context>) -> Result<Action, Box<dyn Error>> {
    let ns = obj.namespace().unwrap();
    let md_views: Api<MarkdownView> = Api::namespaced(ctx.client.clone(), &ns);

    reconcile_configmap(Arc::clone(&obj), ctx).await?;

    Ok(Action::await_change())
}
