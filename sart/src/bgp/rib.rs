use clap::{Parser, Subcommand, ValueEnum};
use prost_types::Any;
use tabled::{Table, Tabled};

use crate::{
    cmd::Format,
    error::Error,
    proto::{
        self,
        sart::{AddPathRequest, AddressFamily, GetPathRequest},
    },
    rpc::connect_bgp,
    util,
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
    Add {
        prefixes: Vec<String>,
        #[arg(
            value_enum,
            long = "attr",
            short = 't',
            help = "Attributes to attach prefixes"
        )]
        attributes: Vec<String>,
    },
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

#[derive(Debug, Clone)]
pub(crate) enum Attribute {
    Origin,
    LocalPref,
    Med,
}

impl TryFrom<&str> for Attribute {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "origin" => Ok(Attribute::Origin),
            "local_pref" => Ok(Attribute::LocalPref),
            "med" | "multi_exit_disc" => Ok(Attribute::Med),
            _ => Err(Error::UnacceptableAttribute),
        }
    }
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
                .map(DisplayedPath::from)
                .collect();
            let data = Table::new(display).to_string();
            println!("{}", data);
        }
    }
    Ok(())
}

pub(crate) async fn add(
    endpoint: &str,
    afi: Afi,
    safi: Safi,
    prefixes: Vec<String>,
    attributes: Vec<String>,
) -> Result<(), Error> {
    let mut client = connect_bgp(endpoint).await;

    let attr_any = attributes
        .iter()
        .map(|a| str_to_attr_any(a).unwrap())
        .collect();

    let _res = client
        .add_path(AddPathRequest {
            family: Some(proto::sart::AddressFamily {
                afi: afi as i32,
                safi: safi as i32,
            }),
            prefixes,
            attributes: attr_any,
        })
        .await
        .map_err(Error::FailedToGetResponse)?;
    Ok(())
}

fn str_to_attr_any(s: &str) -> Result<Any, Error> {
    let key_value = s.split('=').collect::<Vec<&str>>();

    let key = Attribute::try_from(key_value[0])?;

    match key {
        Attribute::Origin => {
            let value: u32 = match key_value[1].parse() {
                Ok(value) => value,
                Err(_) => {
                    let value_str = key_value[1].to_lowercase();
                    if value_str == "igp" || value_str == "i" {
                        0
                    } else if value_str == "egp" || value_str == "e" {
                        1
                    } else if value_str == "incomplete" || value_str == "?" {
                        2
                    } else {
                        return Err(Error::InvalidOriginValue);
                    }
                }
            };
			if value > 2 {
				return Err(Error::InvalidOriginValue);
			}
            let origin = proto::sart::OriginAttribute { value };
            Ok(util::to_any(origin, "OriginAttribute"))
        }
        Attribute::LocalPref => {
            let value: u32 = key_value[1].parse().unwrap();
            let local_pref = proto::sart::LocalPrefAttribute { value };
            Ok(util::to_any(local_pref, "LocalPrefAttribute"))
        }
        Attribute::Med => {
            let value: u32 = key_value[1].parse().unwrap();
            let med = proto::sart::MultiExitDiscAttribute { value };
            Ok(util::to_any(med, "MultiExitDiscAttribute"))
        }
    }
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
