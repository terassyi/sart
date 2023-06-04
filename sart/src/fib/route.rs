use clap::Parser;
use tabled::{Table, Tabled};

use crate::{cmd::Format, error::Error, proto::sart::GetRoutesRequest, rpc::connect_fib};

use crate::proto::sart::Route;

#[derive(Debug, Clone, Parser)]
pub(crate) struct RouteCmd {
    pub name: String,
}

pub(crate) async fn get(endpoint: &str, format: Format, name: String) -> Result<(), Error> {
    let mut client = connect_fib(endpoint).await;

    let res = client
        .get_routes(GetRoutesRequest { channel: name })
        .await
        .map_err(Error::FailedToGetResponse)?;

    match format {
        Format::Json => {}
        Format::Plain => {
            let display: Vec<DisplayedRoute> = res
                .get_ref()
                .routes
                .iter()
                .map(DisplayedRoute::from)
                .collect();
            let data = Table::new(display).to_string();
            println!("{}", data);
        }
    }
    Ok(())
}

#[derive(Tabled)]
pub(crate) struct DisplayedRoute {
    destinaiton: String,
    next_hops: String,
    protocol: String,
    scope: String,
    kind: String,
    ad: i32,
    priority: u32,
}

impl From<&Route> for DisplayedRoute {
    fn from(route: &Route) -> Self {
        let mut next_hops_str = String::new();
        for nh in route.next_hops.iter() {
            next_hops_str += &format!("{} ", nh.gateway);
        }

        let protocol_str = match route.protocol {
            0 => "Unspec".to_string(),
            1 => "Redirect".to_string(),
            2 => "Kernel".to_string(),
            3 => "Boot".to_string(),
            4 => "Static".to_string(),
            186 => "Bgp".to_string(),
            187 => "IsIs".to_string(),
            188 => "Ospf".to_string(),
            189 => "Rib".to_string(),
            _ => format!("Unknown({})", route.protocol),
        };

        let scope_str = match route.scope {
            0 => "Universe".to_string(),
            200 => "Site".to_string(),
            253 => "Link".to_string(),
            254 => "Host".to_string(),
            255 => "Nowhere".to_string(),
            _ => format!("Unknown({})", route.scope),
        };

        let kind_str = match route.r#type {
            0 => "UnspecType".to_string(),
            1 => "Unicast".to_string(),
            2 => "Local".to_string(),
            3 => "Broadcast".to_string(),
            4 => "Anycast".to_string(),
            5 => "Multicast".to_string(),
            6 => "Blackhole".to_string(),
            7 => "Unreachable".to_string(),
            8 => "Prohibit".to_string(),
            9 => "Throw".to_string(),
            10 => "Nat".to_string(),
            _ => format!("Unknown({})", route.r#type),
        };

        DisplayedRoute {
            destinaiton: route.destination.clone(),
            next_hops: next_hops_str,
            protocol: protocol_str.to_string(),
            scope: scope_str.to_string(),
            kind: kind_str.to_string(),
            ad: route.ad,
            priority: route.priority,
        }
    }
}
