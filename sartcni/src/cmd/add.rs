use std::{future::Future, pin::Pin, time::Duration};

use rscni::{
    error::Error,
    types::{Args, CNIResult},
};

use crate::{
    config::Config,
    error::{ERROR_CODE_TIMEOUT, ERROR_MSG_TIMEOUT},
    proto::{self, connect, sart},
};

/*
 Warning  FailedCreatePodSandBox  24s                kubelet            Failed to create pod sandbox: rpc error: code = Unknown desc = failed to setup network for sandbox "c914f53223a7079d52ad7e3e736bec9f200b00fc5dc17cd8f041cc093f9ae0e8": plugin type="sart-cni" failed (add): netplugin failed but error parsing its diagnostic message "Args { container_id: \"c914f53223a7079d52ad7e3e736bec9f200b00fc5dc17cd8f041cc093f9ae0e8\", netns: Some(\"/var/run/netns/cni-8dd0c306-e4ae-ceb9-20c9-fff6a1902a27\"), ifname: \"eth0\", args: Some(\"IgnoreUnknown=1;K8S_POD_NAMESPACE=kube-system;K8S_POD_NAME=coredns-5dd5756b68-br76z;K8S_POD_INFRA_CONTAINER_ID=c914f53223a7079d52ad7e3e736bec9f200b00fc5dc17cd8f041cc093f9ae0e8;K8S_POD_UID=0de6b91a-5908-4119-aa1a-9e385206592e\"), path: [\"/opt/cni/bin\"], config: Some(NetConf { cni_version: \"1.0.0\", cni_versions: None, name: \"sart-network\", type: \"sart-cni\", disable_check: None, runtime_config: None, capabilities: None, ip_masq: None, ipam: None, dns: None, args: None, prev_result: None, custom: {\"endpoint\": String(\"http://localhost:6000\"), \"timeout\": Number(60)} }) }\n{\"cniVersion\":\"1.0.0\",\"code\":100,\"msg\":\"Custom error: Failed to connect gPRC server\",\"details\":\"transport error\"}": invalid character 'A' looking for beginning of value
*/
pub fn add(args: Args) -> Pin<Box<dyn Future<Output = Result<CNIResult, Error>>>> {
    let fut = async { inner_add(args).await };
    Box::pin(fut)
}

async fn inner_add(args: Args) -> Result<CNIResult, Error> {
    let conf = Config::parse(&args)?;

    let mut client = connect(&conf.endpoint).await?;

    let rpc_args = sart::Args::from(&args);

    let rpc_result = tokio::time::timeout(Duration::from_secs(conf.timeout), async move {
        client.add(rpc_args).await
    })
    .await
    .map_err(|_| {
        Error::Custom(
            ERROR_CODE_TIMEOUT,
            ERROR_MSG_TIMEOUT.to_string(),
            "CNI Add request timeout.".to_string(),
        )
    })?;

    match rpc_result {
        Ok(result) => {
            let res = CNIResult::from(result.get_ref());
            // println!("{:?}", res);
            Ok(res)
        }
        Err(status) => {
            let err_result: proto::CNIErrorDetail = serde_json::from_slice(status.details())
                .map_err(|e| Error::FailedToDecode(format!("sart-cni catch: {}", e)))?;
            Err(Error::from(&err_result))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::Path};

    use rscni::types::{Args, NetConf};
    use serde_json::json;
    use tokio::net::UnixListener;
    use tokio_stream::wrappers::UnixListenerStream;
    use tonic::transport::Server;

    use crate::{
        mock::{MockCNIApiServer, MockContainer},
        proto::sart::{cni_api_server::CniApiServer, CniResult, Interface, IpConf},
    };

    use super::*;

    #[tokio::test]
    async fn test_inner_add() {
        let containers = HashMap::from([(
            "pod1".to_string(),
            MockContainer {
                cni_result: CniResult {
                    interfaces: vec![Interface {
                        name: "eth0".to_string(),
                        mac: "ff:ff:ff:ff:ff:ff".to_string(),
                        sandbox: "/var/run/pod1".to_string(),
                    }],
                    ips: vec![IpConf {
                        interface: 1,
                        address: "10.0.0.1/24".to_string(),
                        gateway: "10.0.0.0/24".to_string(),
                    }],
                    routes: Vec::new(),
                    dns: None,
                },
                add: false,
                del: false,
                check: 0,
            },
        )]);

        let sock_addr = "127.0.0.1:12345".parse().unwrap();

        tokio::spawn(async move {
            Server::builder()
                .add_service(CniApiServer::new(MockCNIApiServer::new(containers)))
                .serve(sock_addr)
                .await
                .unwrap();
        });

        let cni_res = inner_add(Args {
            container_id: "pod1".to_string(),
            config: Some(NetConf {
                custom: HashMap::from([
                    (
                        "endpoint".to_string(),
                        json!("http://localhost:12345".to_string()),
                    ),
                    ("timeout".to_string(), json!(60)),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await
        .unwrap();

        assert_eq!(cni_res.interfaces.len(), 1);
        assert_eq!(cni_res.interfaces[0].name, "eth0");

        let res = inner_add(Args {
            container_id: "pod1".to_string(),
            config: Some(NetConf {
                custom: HashMap::from([
                    (
                        "endpoint".to_string(),
                        json!("http://localhost:12345".to_string()),
                    ),
                    ("timeout".to_string(), json!(60)),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await;
        assert!(res.is_err());
        if let Err(Error::Custom(code, _, _)) = res {
            assert_eq!(code, 120);
        }

        let res = inner_add(Args {
            container_id: "pod2".to_string(),
            config: Some(NetConf {
                custom: HashMap::from([
                    (
                        "endpoint".to_string(),
                        json!("http://localhost:12345".to_string()),
                    ),
                    ("timeout".to_string(), json!(60)),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await;
        assert!(res.is_err());
        if let Err(Error::NotExist(_)) = res {
        } else {
            panic!("Error::NotExist is expected {:?}", res)
        }
    }

    #[tokio::test]
    async fn test_inner_add_with_uds() {
        let containers = HashMap::from([(
            "pod1".to_string(),
            MockContainer {
                cni_result: CniResult {
                    interfaces: vec![Interface {
                        name: "eth0".to_string(),
                        mac: "ff:ff:ff:ff:ff:ff".to_string(),
                        sandbox: "/var/run/pod1".to_string(),
                    }],
                    ips: vec![IpConf {
                        interface: 1,
                        address: "10.0.0.1/24".to_string(),
                        gateway: "10.0.0.0/24".to_string(),
                    }],
                    routes: Vec::new(),
                    dns: None,
                },
                add: false,
                del: false,
                check: 0,
            },
        )]);

        let sock_path = Path::new("/var/run/sart.sock");
        let uds_listener = UnixListener::bind(sock_path).unwrap();
        let uds_stream = UnixListenerStream::new(uds_listener);

        tokio::spawn(async move {
            Server::builder()
                .add_service(CniApiServer::new(MockCNIApiServer::new(containers)))
                .serve_with_incoming(uds_stream)
                .await
                .unwrap();
        });

        let cni_res = inner_add(Args {
            container_id: "pod1".to_string(),
            config: Some(NetConf {
                custom: HashMap::from([
                    (
                        "endpoint".to_string(),
                        json!("/var/run/sart.sock".to_string()),
                    ),
                    ("timeout".to_string(), json!(60)),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await
        .unwrap();

        assert_eq!(cni_res.interfaces.len(), 1);
        assert_eq!(cni_res.interfaces[0].name, "eth0");

        let res = inner_add(Args {
            container_id: "pod1".to_string(),
            config: Some(NetConf {
                custom: HashMap::from([
                    (
                        "endpoint".to_string(),
                        json!("/var/run/sart.sock".to_string()),
                    ),
                    ("timeout".to_string(), json!(60)),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await;
        assert!(res.is_err());
        if let Err(Error::Custom(code, _, _)) = res {
            assert_eq!(code, 120);
        }

        let res = inner_add(Args {
            container_id: "pod2".to_string(),
            config: Some(NetConf {
                custom: HashMap::from([
                    (
                        "endpoint".to_string(),
                        json!("/var/run/sart.sock".to_string()),
                    ),
                    ("timeout".to_string(), json!(60)),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await;
        assert!(res.is_err());
        if let Err(Error::NotExist(_)) = res {
        } else {
            panic!("Error::NotExist is expected {:?}", res)
        }
    }
}
