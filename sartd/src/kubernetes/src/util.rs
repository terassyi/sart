use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::{core::ApiResource, Resource, ResourceExt};

use super::error::Error;

pub fn create_owner_reference<T: Resource<DynamicType = ()>>(owner: &T) -> OwnerReference {
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

pub fn get_namespace<T: Resource<DynamicType = ()>>(resource: &T) -> Result<String, Error> {
    resource.namespace().ok_or(Error::GetNamespace)
}

pub fn get_namespaced_name<T: Resource<DynamicType = ()>>(resource: &T) -> String {
    match resource.namespace() {
        Some(ns) => format!("{ns}/{}", resource.name_any()),
        None => resource.name_any(),
    }
}

pub fn escape_slash(s: &str) -> String {
    s.replace('/', "~1")
}
