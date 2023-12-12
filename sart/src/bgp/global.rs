use clap::{Parser, Subcommand};

use crate::data::bgp::BgpInfo;
use crate::error::Error;
use crate::proto::sart::{HealthRequest, SetAsRequest, SetRouterIdRequest, ConfigureMultiPathRequest};
use crate::{cmd::Output, proto::sart::GetBgpInfoRequest, rpc::connect_bgp};

use super::rib::RibCmd;

#[derive(Debug, Clone, Parser)]
pub(crate) struct GlobalCmd {
    #[structopt(subcommand)]
    pub action: Action,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum Action {
    Get,
    Set {
        #[arg(long, help = "Local AS Number")]
        asn: Option<u32>,

        #[arg(long, help = "Local Router Id")]
        router_id: Option<String>,

        #[arg(long, help = "Multi path")]
        multi_path: Option<bool>
    },
    Rib(RibCmd),
    Policy,
    Health,
}

pub(crate) async fn get(endpoint: &str, format: Output) -> Result<(), Error> {
    let mut client = connect_bgp(endpoint).await;
    let res = client
        .get_bgp_info(GetBgpInfoRequest {})
        .await
        .map_err(Error::FailedToGetResponse)?;

    match format {
        Output::Json => {
            // TODO: fix me: I can't serialize prost_types::Any to Json
            let info = match &res.get_ref().info {
                Some(info) => info,
                None => return Err(Error::InvalidRPCResponse),
            };
            let info_data = BgpInfo::from(info);
            let json_data = serde_json::to_string_pretty(&info_data).map_err(Error::Serialize)?;
            println!("{json_data}");
        }
        Output::Plain => {
            let info = match &res.get_ref().info {
                Some(info) => info,
                None => return Err(Error::InvalidRPCResponse),
            };
            println!("Sartd BGP server is running at {}.", info.port,);
            println!("  ASN: {}, Router Id: {}", info.asn, info.router_id);
            println!("  Multi Path enabled: {}", info.multi_path);
        }
    }
    Ok(())
}

pub(crate) async fn set(
    endpoint: &str,
    asn: Option<u32>,
    router_id: Option<String>,
    multi_path: Option<bool>,
) -> Result<(), Error> {
    let mut client = connect_bgp(endpoint).await;
    if asn.is_none() && router_id.is_none() && multi_path.is_none() {
        return Err(Error::MissingArgument {
            msg: "--asn or --router-id or --multi-path".to_string(),
        });
    }
    if let Some(asn) = asn {
        let _res = client
            .set_as(SetAsRequest { asn })
            .await
            .map_err(Error::FailedToGetResponse)?;
    }
    if let Some(router_id) = router_id {
        let _res = client
            .set_router_id(SetRouterIdRequest { router_id })
            .await
            .map_err(Error::FailedToGetResponse)?;
    }
    if let Some(multi_path) = multi_path {
        let _res = client
            .configure_multi_path(ConfigureMultiPathRequest{ enable: multi_path })
            .await
            .map_err(Error::FailedToGetResponse)?;
    }
    Ok(())
}

pub(crate) async fn health(endpoint: &str) -> Result<(), Error> {
    let mut client = connect_bgp(endpoint).await;
    client
        .health(HealthRequest {})
        .await
        .map_err(Error::FailedToGetResponse)?;
    Ok(())
}
