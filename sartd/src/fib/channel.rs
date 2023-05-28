use futures::FutureExt;
use std::sync::Arc;
use std::sync::Mutex;
use tokio_stream::StreamExt;

use crate::fib::bgp;
use crate::fib::kernel;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;

use super::error::Error;
use super::rib::Rib;
use super::rib::Route;
use super::rib::RequestType;

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Channel {
    pub name: String,
    pub ip_version: String,
    pub publishers: Vec<Protocol>,
    pub subscribers: Vec<Protocol>,
    #[serde(skip)]
    rib: Arc<Mutex<Rib>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "protocol")]
pub(crate) enum Protocol {
    #[serde(rename = "kernel")]
    Kernel(kernel::Kernel),
    #[serde(rename = "bgp")]
    Bgp(bgp::Bgp),
}

impl Protocol {
    async fn subscribe(&self) -> Result<Receiver<(RequestType, Route)>, Error> {
        match self {
            Protocol::Bgp(b) => b.subscribe().await,
            Protocol::Kernel(k) => k.subscribe().await
        }
    }

    async fn publish(&self, route: Route) -> Result<(), Error> {
        match self {
            Protocol::Bgp(b) => b.publish(route).await,
            Protocol::Kernel(k) => k.publish().await,
        }
    }
}

impl Channel {
    #[tracing::instrument(skip(self))]
    pub async fn run(&mut self) -> Result<(), Error> {
        let mut receivers: Vec<Receiver<(RequestType, Route)>> = Vec::new();
        for s in self.subscribers.iter() {
            s.subscribe().await?;
        }

        let mut fused_receivers = futures::stream::select_all(
            receivers
                .into_iter()
                .map(tokio_stream::wrappers::ReceiverStream::new),
        );

        let sender: Vec<Sender<Route>> = Vec::new();
        for p in self.publishers.iter() {
            match p {
                Protocol::Bgp(b) => {}
                Protocol::Kernel(k) => {}
            }
        }

        loop {
            futures::select_biased! {
                request = fused_receivers.next().fuse() => {
                    if let Some(request) = request {
                        let res = match request.0 {
                            RequestType::AddRoute | RequestType::AddMultiPathRoute => {
                                self.register(request.1)
                            },
                            RequestType::DeleteRoute | RequestType::DeleteMultiPathRoute => {
                                self.remove(request.1)
                            },
                        };
                        match res {
                            Ok(route) => {
                                for p in self.publishers.iter() {
                                    match p.publish(route.clone()).await {
                                        Ok(_) => {},
                                        Err(e) => tracing::error!(error=?e, "failed to publish the route")
                                    }
                                }
                            },
                            Err(e) => {
                                tracing::error!(error=?e, "failed to handle rib");
                            }
                        }
                    }
                }
            }
        }
    }

    #[tracing::instrument(skip(self))]
    fn register(&mut self, route: Route) -> Result<Route, Error> {
        let mut rib = self.rib.lock().unwrap();
        match rib.insert(route.destination, route) {
            Some(route) => {
                tracing::info!("Register the route");
                Ok(route)
            },
            None => {
                tracing::error!("failed to insert to rib");
                Err(Error::FailedToInsert)
            }
        }
    }

    fn remove(&mut self, route: Route) -> Result<Route, Error> {
        let mut rib = self.rib.lock().unwrap();
        if let Some(routes) =  rib.get(&route.destination) {
            if routes.len() == 0 {
                tracing::error!("Multiple paths are not registered.");
                return Err(Error::DestinationNotFound)
            }
            match rib.remove(route) {
                Some(route) => {
                    tracing::info!("Remove the route");
                    Ok(route)
                },
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
