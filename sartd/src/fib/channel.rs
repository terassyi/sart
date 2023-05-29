use futures::FutureExt;
use tokio::sync::mpsc::channel;
use std::sync::Arc;
use std::sync::Mutex;
use tokio_stream::StreamExt;

use crate::fib::bgp;
use crate::fib::kernel;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;

use super::error::Error;
use super::kernel::KernelRtPoller;
use super::rib::RequestType;
use super::rib::Rib;
use super::rib::Route;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub(crate) struct Channel {
    pub name: String,
    pub ip_version: String,
    pub publishers: Vec<Protocol>,
    pub subscribers: Vec<Protocol>,
    #[serde(skip)]
    rib: Arc<Mutex<Rib>>,
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
    pub(crate) async fn publish(&self, route: Route) -> Result<(), Error> {
        match self {
            Protocol::Bgp(b) => b.publish(route).await,
            Protocol::Kernel(k) => k.publish().await,
        }
    }
}

impl Channel {

    #[tracing::instrument(skip(self, poller))]
    pub async fn register(&mut self, poller: &mut KernelRtPoller) -> Result<Vec<Receiver<(RequestType, Route)>>, Error> {
        let mut receivers: Vec<Receiver<(RequestType, Route)>> = Vec::new();
        for s in self.subscribers.iter() {
            let rx = match s {
                Protocol::Bgp(b) => b.subscribe().await,
                Protocol::Kernel(k) => {
                    let (kernel_tx, kernel_rx) = channel(128);
                    tracing::info!(subscriber=?k,"register to poller");
                    poller.register(k, kernel_tx)?;
                    k.subscribe(kernel_rx).await
                },
            }?;
            receivers.push(rx);
        }

        Ok(receivers)
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn register_route(&mut self, route: Route) -> Result<Route, Error> {
        let mut rib = self.rib.lock().unwrap();
        match rib.insert(route.destination, route) {
            Some(route) => {
                tracing::info!("Register the route");
                Ok(route)
            }
            None => {
                tracing::error!("failed to insert to rib");
                Err(Error::FailedToInsert)
            }
        }
    }

    pub(crate) fn remove_route(&mut self, route: Route) -> Result<Route, Error> {
        let mut rib = self.rib.lock().unwrap();
        if let Some(routes) = rib.get(&route.destination) {
            if routes.len() == 0 {
                tracing::error!("Multiple paths are not registered.");
                return Err(Error::DestinationNotFound);
            }
            match rib.remove(route) {
                Some(route) => {
                    tracing::info!("Remove the route");
                    Ok(route)
                }
                None => {
                    tracing::error!("Failed to remove the route");
                    Err(Error::FailedToRemove)
                }
            }
        } else {
            Err(Error::DestinationNotFound)
        }
    }
}
