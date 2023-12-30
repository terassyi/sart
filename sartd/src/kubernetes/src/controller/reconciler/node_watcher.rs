use std::sync::Arc;

use futures::StreamExt;
use k8s_openapi::api::core::v1::Node;
use kube::{
    api::{DeleteParams, ListParams, PostParams},
    runtime::{
        controller::Action,
        finalizer::{finalizer, Event},
        watcher::Config,
        Controller,
    },
    Api, Client, ResourceExt,
};

use crate::{
    context::{error_policy, Context, State},
    controller::error::Error,
    crd::{
        cluster_bgp::{ClusterBGP, ClusterBGPStatus},
        node_bgp::NodeBGP,
    },
};

pub const NODE_FINALIZER: &str = "node.sart.terassyi.net/finalizer";

#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn reconciler(node: Arc<Node>, ctx: Arc<Context>) -> Result<Action, Error> {
    let nodes = Api::<Node>::all(ctx.client.clone());
    finalizer(&nodes, NODE_FINALIZER, node, |event| async {
        match event {
            Event::Apply(node) => reconcile(&nodes, &node, ctx.clone()).await,
            Event::Cleanup(node) => cleanup(&nodes, &node, ctx.clone()).await,
        }
    })
    .await
    .map_err(|e| Error::Finalizer(Box::new(e)))
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile(_api: &Api<Node>, node: &Node, ctx: Arc<Context>) -> Result<Action, Error> {
    tracing::info!(name = node.name_any(), "Reconcile Node");

    // sync node labels
    let node_bgp_api = Api::<NodeBGP>::all(ctx.client.clone());
    if let Some(mut nb) = node_bgp_api
        .get_opt(&node.name_any())
        .await
        .map_err(Error::Kube)?
    {
        let node_labels = node.labels();
        nb.metadata.labels = Some(node_labels.clone());

        node_bgp_api
            .replace(&nb.name_any(), &PostParams::default(), &nb)
            .await
            .map_err(Error::Kube)?;
    }

    let cluster_bgp_api = Api::<ClusterBGP>::all(ctx.client.clone());

    let cb_list = cluster_bgp_api
        .list(&ListParams::default())
        .await
        .map_err(Error::Kube)?;

    for cb in cb_list.iter() {
        let member = is_member(cb, node);
        if let Some(new_cb) = update_cluster_bgp_member(cb, &node.name_any(), member) {
            tracing::info!(
                node = node.name_any(),
                cluster_bgp = cb.name_any(),
                nodes =? new_cb.status,
                "Update ClusterBGP's status by node_watcher"
            );
            cluster_bgp_api
                .replace_status(
                    &cb.name_any(),
                    &PostParams::default(),
                    serde_json::to_vec(&new_cb).map_err(Error::Serialization)?,
                )
                .await
                .map_err(Error::Kube)?;
        }
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn cleanup(_api: &Api<Node>, node: &Node, ctx: Arc<Context>) -> Result<Action, Error> {
    let node_bgp_api = Api::<Node>::all(ctx.client.clone());

    node_bgp_api
        .delete(&node.name_any(), &DeleteParams::default())
        .await
        .map_err(Error::Kube)?;

    Ok(Action::await_change())
}

pub async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let nodes = Api::<Node>::all(client.clone());

    Controller::new(nodes, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<Node, Error, Context>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

fn is_member(cb: &ClusterBGP, node: &Node) -> bool {
    let node_selector = &cb.spec.node_selector;
    match node_selector {
        Some(node_selector) => {
            for (k, v) in node.labels() {
                if let Some(value) = node_selector.get(k) {
                    if value.eq(v.as_str()) {
                        return true;
                    }
                }
            }
        }
        None => return true,
    }
    false
}

fn update_cluster_bgp_member(cb: &ClusterBGP, name: &str, member: bool) -> Option<ClusterBGP> {
    match member {
        true => add_member_of_cluster_bgp(cb, name),
        false => remove_member_of_cluster_bgp(cb, name),
    }
}

fn add_member_of_cluster_bgp(cb: &ClusterBGP, name: &str) -> Option<ClusterBGP> {
    let mut new_cb = cb.clone();
    let mut add = true;

    match new_cb.status.as_mut() {
        Some(status) => match status.desired_nodes.as_mut() {
            Some(nodes) => {
                if !nodes.iter().any(|n| name.eq(n.as_str())) {
                    nodes.push(name.to_string());
                    nodes.sort();
                } else {
                    add = false;
                }
            }
            None => status.desired_nodes = Some(vec![name.to_string()]),
        },
        None => {
            new_cb.status = Some(ClusterBGPStatus {
                desired_nodes: Some(vec![name.to_string()]),
                nodes: None,
            })
        }
    }
    if add {
        return Some(new_cb);
    }
    None
}

fn remove_member_of_cluster_bgp(cb: &ClusterBGP, name: &str) -> Option<ClusterBGP> {
    let mut new_cb = cb.clone();
    let mut remove = false;

    if let Some(status) = new_cb.status.as_mut() {
        if let Some(nodes) = status.desired_nodes.as_mut() {
            if nodes.iter().any(|n| name.eq(n.as_str())) {
                nodes.retain(|n| name.ne(n.as_str()));
                remove = true;
            }
        }
    }
    if remove {
        return Some(new_cb);
    }
    None
}

#[cfg(test)]
mod tests {
    use kube::core::ObjectMeta;
    use rstest::rstest;

    use crate::crd::cluster_bgp::{ClusterBGP, ClusterBGPSpec, ClusterBGPStatus};

    use super::{add_member_of_cluster_bgp, remove_member_of_cluster_bgp};

    #[rstest(
        cb,
        name,
        expected,
        case(
            ClusterBGP{
                metadata: ObjectMeta{
                    name: Some("test-cb".to_string()),
                    ..Default::default()
                },
                spec: ClusterBGPSpec::default(),
                status: None
            },
            "test-node",
            Some(vec!["test-node".to_string()]),
        ),
        case(
            ClusterBGP{
                metadata: ObjectMeta{
                    name: Some("test-cb".to_string()),
                    ..Default::default()
                },
                spec: ClusterBGPSpec::default(),
                status: Some(ClusterBGPStatus{
                    desired_nodes: Some(vec!["test-node".to_string()]),
                    nodes: None,
                }),
            },
            "test-node",
            None,
        ),
        case(
            ClusterBGP{
                metadata: ObjectMeta{
                    name: Some("test-cb".to_string()),
                    ..Default::default()
                },
                spec: ClusterBGPSpec::default(),
                status: Some(ClusterBGPStatus{
                    desired_nodes: Some(vec!["test-node".to_string()]),
                    nodes: None,
                }),
            },
            "test-node2",
            Some(vec!["test-node".to_string(), "test-node2".to_string()]),
        ),
    )]
    fn test_add_member_of_cluster_bgp(cb: ClusterBGP, name: String, expected: Option<Vec<String>>) {
        let res = add_member_of_cluster_bgp(&cb, &name);
        let nodes = res.and_then(|cb| cb.status.and_then(|status| status.desired_nodes));
        assert_eq!(expected, nodes);
    }

    #[rstest(
        cb,
        name,
        expected,
        case(
            ClusterBGP{
                metadata: ObjectMeta{
                    name: Some("test-cb".to_string()),
                    ..Default::default()
                },
                spec: ClusterBGPSpec::default(),
                status: None
            },
            "test-node",
            None,
        ),
        case(
            ClusterBGP{
                metadata: ObjectMeta{
                    name: Some("test-cb".to_string()),
                    ..Default::default()
                },
                spec: ClusterBGPSpec::default(),
                status: Some(ClusterBGPStatus{
                    desired_nodes: Some(vec!["test-node".to_string()]),
                    nodes: None,
                }),
            },
            "test-node",
            Some(Vec::new()),
        ),
        case(
            ClusterBGP{
                metadata: ObjectMeta{
                    name: Some("test-cb".to_string()),
                    ..Default::default()
                },
                spec: ClusterBGPSpec::default(),
                status: Some(ClusterBGPStatus{
                    desired_nodes: Some(vec!["test-node".to_string(), "test-node2".to_string()]),
                    nodes: None,
                }),
            },
            "test-node",
            Some(vec!["test-node2".to_string()]),
        ),
    )]
    fn test_remove_member_of_cluster_bgp(
        cb: ClusterBGP,
        name: String,
        expected: Option<Vec<String>>,
    ) {
        let res = remove_member_of_cluster_bgp(&cb, &name);
        let nodes = res.and_then(|cb| cb.status.and_then(|status| status.desired_nodes));
        assert_eq!(expected, nodes);
    }
}
