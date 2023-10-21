use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::{core::ApiResource, Resource, ResourceExt};

pub(crate) fn create_owner_reference<T: Resource<DynamicType = ()>>(owner: &T) -> OwnerReference {
    let res = ApiResource::erase::<T>(&());
    OwnerReference {
        name: owner.name_any(),
        api_version: res.api_version,
        kind: res.kind,
        uid: match &owner.meta().uid {
            Some(uid) => uid.clone(),
            None => "".to_string(),
        },
        block_owner_deletion: Some(true),
        controller: Some(true),
    }
}
