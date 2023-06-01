use ipnet::IpNet;
use tokio::sync::mpsc::UnboundedSender;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc::channel;

use crate::fib::bgp;
use crate::fib::kernel;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;

use super::error::Error;
use super::kernel::KernelRtPoller;
use super::rib::RequestType;
use super::rib::Rib;
use super::route;
use super::route::Route;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub(crate) struct Channel {
    pub name: String,
    pub ip_version: String,
    pub publishers: Vec<Protocol>,
    pub subscribers: Vec<Protocol>,
    #[serde(skip)]
    rib: Arc<Mutex<Rib>>,
}

impl From<Channel> for crate::proto::sart::Channel {
    fn from(ch: Channel) -> crate::proto::sart::Channel {
        crate::proto::sart::Channel {
            name: ch.name,
            subscribers: ch.subscribers.iter().map(crate::proto::sart::ChProtocol::from).collect(),
            publishers: ch.publishers.iter().map(crate::proto::sart::ChProtocol::from).collect(),
        }
    }
}

impl From<&Channel> for crate::proto::sart::Channel {
    fn from(ch: &Channel) -> crate::proto::sart::Channel {
        crate::proto::sart::Channel {
            name: ch.name.clone(),
            subscribers: ch.subscribers.iter().map(crate::proto::sart::ChProtocol::from).collect(),
            publishers: ch.publishers.iter().map(crate::proto::sart::ChProtocol::from).collect(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "protocol")]
pub(crate) enum Protocol {
    #[serde(rename = "kernel")]
    Kernel(kernel::Kernel),
    #[serde(rename = "bgp")]
    Bgp(bgp::Bgp),
}

impl Protocol {
    pub(crate) async fn publish(&self, req: RequestType, route: Route) -> Result<(), Error> {
        match self {
            Protocol::Bgp(b) => b.publish(req, route).await,
            Protocol::Kernel(k) => k.publish(req, route).await,
        }
    }

    pub(crate) fn register_publisher(&mut self, tx: UnboundedSender<(RequestType, Route)>) {
        match self {
            Protocol::Bgp(b) => b.register_publisher(tx),
            Protocol::Kernel(k) => k.register_publisher(tx),
        }
    }
}

impl From<&Protocol> for crate::proto::sart::ChProtocol {
    fn from(protocol: &Protocol) -> Self {
        match protocol {
            Protocol::Bgp(b) => crate::proto::sart::ChProtocol{
                r#type: "bgp".to_string(),
                endpoint: b.endpoint.clone(),
                tables: Vec::new(),
            },
            Protocol::Kernel(k) => crate::proto::sart::ChProtocol {
                r#type: "kernel".to_string(),
                endpoint: String::new(),
                tables: k.tables.iter().map(|&i| i as i32).collect(),
            }
        }
    }
}

impl Channel {
    #[tracing::instrument(skip(self, poller))]
    pub async fn register(
        &mut self,
        poller: &mut KernelRtPoller,
        publisher_tx: UnboundedSender<(RequestType, Route)>,
    ) -> Result<Vec<Receiver<(RequestType, Route)>>, Error> {
        let mut receivers: Vec<Receiver<(RequestType, Route)>> = Vec::new();
        for s in self.subscribers.iter() {
            let rx = match s {
                Protocol::Bgp(b) => b.subscribe().await,
                Protocol::Kernel(k) => {
                    let (kernel_tx, kernel_rx) = channel(128);
                    tracing::info!(subscriber=?k,"register to poller");
                    poller.register_subscriber(k, kernel_tx)?;
                    k.subscribe(kernel_rx).await
                }
            }?;
            receivers.push(rx);
        }

        for p in self.publishers.iter_mut() {
            p.register_publisher(publisher_tx.clone());
        }

        Ok(receivers)
    }

    pub(crate) fn get_route(
        &self,
        destination: &IpNet,
        protocol: route::Protocol,
    ) -> Option<Route> {
        let rib = self.rib.lock().unwrap();
        if let Some(routes) = rib.get(destination) {
            if let Some(route) = routes.iter().find(|r| r.protocol.eq(&protocol)) {
                Some(route.clone())
            } else {
                None
            }
        } else {
            None
        }
    }

    pub(crate) fn list_routes(&self) -> Option<Vec<Route>> {
        let rib = self.rib.lock().unwrap();
        rib.list()
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn register_route(&mut self, route: Route) -> Option<Route> {
        tracing::info!("register the route to rib");
        let mut rib = self.rib.lock().unwrap();
        rib.insert(route.destination, route)
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn remove_route(&mut self, route: Route) -> Option<Route> {
        let mut rib = self.rib.lock().unwrap();
        if let Some(routes) = rib.get(&route.destination) {
            if routes.len() == 0 {
                return None;
            }
            tracing::info!("remove the route from rib");
            rib.remove(route)
        } else {
            None
        }
    }
}
