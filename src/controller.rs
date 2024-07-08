use futures::StreamExt;
use k8s_openapi::{
    api::{
        apps::v1::{Deployment, DeploymentSpec},
        core::v1::{
            ConfigMap, ConfigMapVolumeSource, Container, ContainerPort, HTTPGetAction, PodSpec,
            PodTemplateSpec, Probe, Service, ServicePort, ServiceSpec, Volume, VolumeMount,
        },
    },
    apimachinery::pkg::{apis::meta::v1::LabelSelector, util::intstr::IntOrString},
};
use std::{sync::Arc, time::Duration};
use thiserror::Error;

use kube::{
    api::{ObjectMeta, Patch, PatchParams},
    runtime::{controller::Action, reflector::Lookup, watcher::Config, Controller},
    Api, Client, Resource, ResourceExt,
};

use crate::{MarkdownView, MarkdownViewStatusEnum};

#[derive(Error, Debug)]
pub enum Error {
    #[error("SerializationError: {0}")]
    SerializationError(#[source] serde_json::Error),
    #[error("Kube Error: {0}")]
    KubeError(#[source] kube::Error),
    #[error("Finalizer Error: {0}")]
    // NB: awkward type because finalizer::Error embeds the reconciler error (which is this)
    // so boxing this error to break cycles
    FinalizerError(#[source] Box<kube::runtime::finalizer::Error<Error>>),

    #[error("Unknown Error")]
    UnknownError,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub struct Context {
    pub client: Client,
}

const CONTROLLER_NAME: &str = "markdown-view-manager";

async fn reconcile_configmap(obj: Arc<MarkdownView>, ctx: Arc<Context>) -> Result<()> {
    println!("reconcile configmap start!");
    let obj_namespace = ResourceExt::namespace(obj.as_ref()).expect("cannot found");
    let cm_api: Api<ConfigMap> = Api::namespaced(ctx.client.clone(), &obj_namespace);

    let cm_name = format!("markdowns-{}", obj.name_any());
    let cm_pp = PatchParams::apply(CONTROLLER_NAME);
    let config_map = ConfigMap {
        metadata: ObjectMeta {
            name: Some(cm_name.clone()),
            owner_references: Some(vec![obj.controller_owner_ref(&()).unwrap()]),
            ..Default::default()
        },
        data: Some(obj.spec.markdowns.clone()),
        ..Default::default()
    };
    let cm = cm_api
        .patch(
            config_map.metadata.name.as_deref().unwrap(),
            &cm_pp,
            &Patch::Apply(&config_map),
        )
        .await
        .map_err(Error::KubeError)?;
    Ok(())
}

async fn reconcile_deployment(obj: Arc<MarkdownView>, ctx: Arc<Context>) -> Result<()> {
    println!("reconcile deployment start!");
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
            labels: Some(
                serde_json::from_value(labels.clone()).map_err(Error::SerializationError)?,
            ),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(obj.spec.replicas as i32),
            selector: LabelSelector {
                match_labels: Some(
                    serde_json::from_value(labels.clone()).map_err(Error::SerializationError)?,
                ),
                match_expressions: None,
            },
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(
                        serde_json::from_value(labels.clone())
                            .map_err(Error::SerializationError)?,
                    ),
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
    let dep_api: Api<Deployment> = Api::namespaced(
        ctx.client.clone(),
        &ResourceExt::namespace(obj.as_ref()).expect("cannot found"),
    );
    let dep_patch = dep_api
        .patch(
            deployment.metadata.name.as_deref().unwrap(),
            &PatchParams::apply(CONTROLLER_NAME),
            &Patch::Apply(&deployment),
        )
        .await
        .map_err(Error::KubeError)?;

    Ok(())
}

async fn reconcile_service(obj: Arc<MarkdownView>, ctx: Arc<Context>) -> Result<()> {
    println!("reconcile service start!");
    let obj_namaespace = ResourceExt::namespace(obj.as_ref()).expect("cannot found namespace");
    let svc_name = format!("viewer-{}", obj.name_any());
    let labels = serde_json::json!({
        "app.kubernetes.io/name":"mdbook",
        "app.kubernetes.io/instance":obj.name_any(),
        "app.kubernetes.io/created-by": "markdown-view-controller"
    });
    let svc = Service {
        metadata: ObjectMeta {
            labels: Some(
                serde_json::from_value(labels.clone()).map_err(Error::SerializationError)?,
            ),
            name: Some(svc_name),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            selector: Some(
                serde_json::from_value(labels.clone()).map_err(Error::SerializationError)?,
            ),
            type_: Some("ClusterIP".to_string()),
            ports: Some(vec![ServicePort {
                protocol: Some("TCP".to_string()),
                port: 80,
                target_port: Some(IntOrString::Int(3000)),
                ..Default::default()
            }]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let svc_api: Api<Service> = Api::namespaced(ctx.client.clone(), &obj_namaespace);
    let svc_patch = svc_api
        .patch(
            svc.metadata.name.as_deref().unwrap(),
            &PatchParams::apply(CONTROLLER_NAME),
            &Patch::Apply(&svc),
        )
        .await
        .map_err(Error::KubeError)?;

    Ok(())
}

async fn update_status(obj: Arc<MarkdownView>, ctx: Arc<Context>) -> Result<Action> {
    println!("start update status!");
    let obj_name = obj.name().expect("cannot found");
    let obj_namespace = ResourceExt::namespace(obj.as_ref()).expect("cannot found");
    let dep_name = format!("viewer-{}", obj_name);
    let dep_api: Api<Deployment> = Api::namespaced(ctx.client.clone(), &obj_namespace);
    let dep = dep_api.get(&dep_name).await.map_err(Error::KubeError)?;

    let dep_replicas = dep.spec.as_ref().and_then(|spec| spec.replicas);

    let status = match dep_replicas {
        None => MarkdownViewStatusEnum::NotReady,
        Some(0) => MarkdownViewStatusEnum::NotReady,
        Some(replicas) if replicas == obj.spec.replicas as i32 => MarkdownViewStatusEnum::Available,
        Some(_) => MarkdownViewStatusEnum::Healthy,
    };

    let md_view_api: Api<MarkdownView> = Api::namespaced(ctx.client.clone(), &obj_namespace);
    let mut md_view = md_view_api.get(&obj_name).await.map_err(Error::KubeError)?;
    if md_view.status != Some(status) {
        md_view.status = Some(status);
        md_view_api
            .patch(
                &obj_name,
                &PatchParams::apply(CONTROLLER_NAME).force(),
                &Patch::Apply(md_view),
            )
            .await
            .map_err(Error::KubeError)?;
    }

    Ok(Action::requeue(Duration::from_secs(10)))
}

async fn reconcile(obj: Arc<MarkdownView>, ctx: Arc<Context>) -> Result<Action> {
    println!("reconcile start!");
    let ns = ResourceExt::namespace(obj.as_ref()).unwrap();
    let md_views: Api<MarkdownView> = Api::namespaced(ctx.client.clone(), &ns);

    reconcile_configmap(Arc::clone(&obj), Arc::clone(&ctx)).await?;

    reconcile_deployment(Arc::clone(&obj), Arc::clone(&ctx)).await?;

    reconcile_service(Arc::clone(&obj), Arc::clone(&ctx)).await?;

    return update_status(Arc::clone(&obj), Arc::clone(&ctx)).await;
}

fn error_policy(md_view: Arc<MarkdownView>, error: &Error, ctx: Arc<Context>) -> Action {
    println!("something went wrong: {}", error);
    Action::requeue(Duration::from_secs(5))
}

pub async fn run() {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");
    let md_views: Api<MarkdownView> = Api::all(client.clone());
    Controller::new(md_views, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconcile,
            error_policy,
            Arc::new(Context {
                client: client.clone(),
            }),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}
