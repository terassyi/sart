use kube::{
    api::{Api, ListParams},
    client::Client,
    core::DynamicObject,
    runtime::{
        controller::{Action, Controller},
        watcher::Config,
    },
    CustomResource, ResourceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
// #[cfg_attr(test, derive(Default))]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "ClusterBgp")]
#[kube(status = "ClusterBgpStatus")]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClusterBgpSpec {
    // pub policy: Policy,
    // pub peers: Option<Vec<peer::Peer>>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct ClusterBgpStatus {}
