use std::str::FromStr;

use ipnet::IpNet;

// K8S_POD_INFRA_CONTAINER_ID=0a6a4b09df59d64e3be5cf662808076fee664447a1c90dd05a5d5588e2cd6b5a;K8S_POD_UID=b0e1fc4a-f842-4ec2-8e23-8c0c8da7b5e5;IgnoreUnknown=1;K8S_POD_NAMESPACE=kube-system;K8S_POD_NAME=coredns-787d4945fb-7xrrd
const K8S_POD_INFRA_CONTAINER_ID: &str = "K8S_POD_INFRA_CONTAINER_ID";
const K8S_POD_UID: &str = "K8S_POD_UID";
const K8S_POD_NAMESPACE: &str = "K8S_POD_NAMESPACE";
const K8S_POD_NAME: &str = "K8S_POD_NAME";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodInfo {
    pub container_id: String,
    pub uid: String,
    pub namespace: String,
    pub name: String,
}

impl FromStr for PodInfo {
    type Err = rscni::error::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut info = PodInfo {
            container_id: String::new(),
            uid: String::new(),
            namespace: String::new(),
            name: String::new(),
        };
        let split_str = s.split(';');
        for kv in split_str.into_iter() {
            let split_kv: Vec<&str> = kv.split('=').collect();
            if split_kv.len() != 2 {
                continue;
            }
            let key = split_kv[0];
            let value = split_kv[1];
            match key {
                K8S_POD_INFRA_CONTAINER_ID => info.container_id = value.to_string(),
                K8S_POD_UID => info.uid = value.to_string(),
                K8S_POD_NAMESPACE => info.namespace = value.to_string(),
                K8S_POD_NAME => info.name = value.to_string(),
                _ => {}
            }
        }
        if info.container_id.is_empty() {
            return Err(rscni::error::Error::FailedToDecode(format!(
                "{} is not set",
                K8S_POD_INFRA_CONTAINER_ID
            )));
        }
        if info.uid.is_empty() {
            return Err(rscni::error::Error::FailedToDecode(format!(
                "{} is not set",
                K8S_POD_UID
            )));
        }
        if info.namespace.is_empty() {
            return Err(rscni::error::Error::FailedToDecode(format!(
                "{} is not set",
                K8S_POD_NAMESPACE
            )));
        }
        if info.name.is_empty() {
            return Err(rscni::error::Error::FailedToDecode(format!(
                "{} is not set",
                K8S_POD_NAME
            )));
        }
        Ok(info)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodAllocation {
    pub block: String,
    pub addr: IpNet,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pod_info_from_str() {
        let s = "K8S_POD_INFRA_CONTAINER_ID=0a6a4b09df59d64e3be5cf662808076fee664447a1c90dd05a5d5588e2cd6b5a;K8S_POD_UID=b0e1fc4a-f842-4ec2-8e23-8c0c8da7b5e5;IgnoreUnknown=1;K8S_POD_NAMESPACE=kube-system;K8S_POD_NAME=coredns-787d4945fb-7xrrd";
        let expected = PodInfo {
            container_id: "0a6a4b09df59d64e3be5cf662808076fee664447a1c90dd05a5d5588e2cd6b5a"
                .to_string(),
            uid: "b0e1fc4a-f842-4ec2-8e23-8c0c8da7b5e5".to_string(),
            namespace: "kube-system".to_string(),
            name: "coredns-787d4945fb-7xrrd".to_string(),
        };
        let info = PodInfo::from_str(s).unwrap();
        assert_eq!(expected, info);
    }
}
