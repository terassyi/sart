use std::{time::Duration, pin::Pin, future::Future};

use rscni::{
    error::Error,
    types::{Args, CNIResult},
};

use crate::{
    config::Config,
    error::{ERROR_CODE_GRPC, ERROR_CODE_TIMEOUT, ERROR_MSG_GRPC, ERROR_MSG_TIMEOUT},
    proto::{connect, sart, self},
};

pub fn check(args: Args) -> Pin<Box<dyn Future<Output = Result<CNIResult, Error>>>> {
    let fut = async {
        inner_check(args).await
    };
    Box::pin(fut)
}

async fn inner_check(args: Args) -> Result<CNIResult, Error> {
    let conf = Config::parse(&args)?;

    let mut client = connect(&conf.endpoint).await?;

    let rpc_args = sart::Args::from(&args);

    let rpc_result = tokio::time::timeout(Duration::from_secs(conf.timeout), async move {
        client.check(rpc_args).await
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
        Ok(result) => Ok(CNIResult::from(result.get_ref())),
        Err(status) => {
            let err_result: proto::CNIErrorDetail = serde_json::from_slice(status.details())
                .map_err(|e| Error::FailedToDecode(e.to_string()))?;
            Err(Error::from(&err_result))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rscni::types::{Args, NetConf};
    use serde_json::json;
    use tonic::transport::Server;

    use crate::{
        mock::{MockCNIApiServer, MockContainer},
        proto::sart::{cni_api_server::CniApiServer, CniResult, Interface, IpConf},
    };

    use super::*;

    #[tokio::test]
    async fn test_inner_check() {
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

        let sock_addr = "0.0.0.0:12345".parse().unwrap();

        tokio::spawn(async move {
            Server::builder()
                .add_service(CniApiServer::new(MockCNIApiServer::new(containers)))
                .serve(sock_addr)
                .await
                .unwrap();
        });

        let cni_res = inner_check(Args {
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

        let _res = inner_check(Args {
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
        .await.unwrap();

        let res = inner_check(Args {
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
}
