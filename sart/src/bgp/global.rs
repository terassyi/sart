use clap::{Parser, Subcommand};

use crate::error::Error;
use crate::proto::sart::{SetAsRequest, SetRouterIdRequest};
use crate::{cmd::Format, proto::sart::GetBgpInfoRequest, rpc::connect_bgp};

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
    },
    Rib(RibCmd),
    Policy,
}

pub(crate) async fn get(endpoint: &str, format: Format) -> Result<(), Error> {
    let mut client = connect_bgp(endpoint).await;
    let res = client
        .get_bgp_info(GetBgpInfoRequest {})
        .await
        .map_err(Error::FailedToGetResponse)?;

    match format {
        Format::Json => {
            // TODO: fix me: I can't serialize prost_types::Any to Json
        }
        Format::Plain => {
            let info = match &res.get_ref().info {
                Some(info) => info,
                None => return Err(Error::InvalidRPCResponse),
            };
            println!("Sartd BGP server is running at {}.", info.port,);
            println!("  ASN: {}, Router Id: {}", info.asn, info.router_id);
        }
    }
    Ok(())
}

pub(crate) async fn set(
    endpoint: &str,
    asn: Option<u32>,
    router_id: Option<String>,
) -> Result<(), Error> {
    let mut client = connect_bgp(endpoint).await;
    if asn.is_none() && router_id.is_none() {
        return Err(Error::MissingArgument {
            msg: "--asn or --router-id".to_string(),
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
    Ok(())
}
