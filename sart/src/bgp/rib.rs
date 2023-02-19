use clap::{Parser, Subcommand, ValueEnum};
use tabled::{Table, Tabled};

use crate::{
    cmd::Format,
    error::Error,
    proto::{
        self,
        sart::{AddressFamily, GetPathRequest},
    },
    rpc::connect_bgp,
};

#[derive(Debug, Clone, Parser)]
pub(crate) struct RibCmd {
    #[structopt(subcommand)]
    pub action: Action,

    #[arg(
        value_enum,
        short,
        long,
        global = true,
        default_value = "ipv4",
        help = "Address Family Information"
    )]
    pub afi: Afi,

    #[arg(
        value_enum,
        short,
        long,
        global = true,
        default_value = "unicast",
        help = "Sub Address Family Information"
    )]
    pub safi: Safi,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum Action {
    Get,
    Add { prefix: String },
}

#[derive(Debug, Clone, Parser, ValueEnum)]
pub(crate) enum Afi {
    Ipv4 = 1,
    Ipv6 = 2,
}

#[derive(Debug, Clone, Parser, ValueEnum)]
pub(crate) enum Safi {
    Unicast = 1,
    Multicast = 2,
}

pub(crate) async fn get(endpoint: &str, format: Format, afi: Afi, safi: Safi) -> Result<(), Error> {
    let mut client = connect_bgp(endpoint).await;

    let res = client
        .get_path(GetPathRequest {
            family: Some(AddressFamily {
                afi: afi as i32,
                safi: safi as i32,
            }),
        })
        .await
        .map_err(Error::FailedToGetResponse)?;
    match format {
        Format::Json => {}
        Format::Plain => {
            let display: Vec<DisplayedPath> = res
                .get_ref()
                .paths
                .iter()
                .map(|p| DisplayedPath::from(p))
                .collect();
            let data = Table::new(display).to_string();
            println!("{}", data);
        }
    }
    Ok(())
}

pub(crate) async fn add(endpoint: &str, afi: Afi, safi: Safi, prefix: &str) -> Result<(), Error> {
    Ok(())
}

#[derive(Tabled)]
struct DisplayedPath {
    best: bool,
    prefix: String,
    next_hop: String,
    as_path: String,
}

impl From<&proto::sart::Path> for DisplayedPath {
    fn from(value: &proto::sart::Path) -> Self {
        let mut as_path = String::new();
        for a in value.segments.iter() {
            if a.r#type == 2 {
                // AS_SEQ
                as_path += &format!("{:?}", a.elm);
            } else if a.r#type == 1 {
                as_path += &format!("[{:?}]", a.elm);
            }
        }
        DisplayedPath {
            best: value.best,
            prefix: value.nlri.clone(),
            next_hop: format!("{:?}", value.next_hops),
            as_path,
        }
    }
}
