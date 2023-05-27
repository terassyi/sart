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

impl Channel {
    #[tracing::instrument(skip(self))]
    pub async fn run(&mut self) -> Result<(), Error> {
        let mut receivers: Vec<Receiver<Route>> = Vec::new();
        for s in self.subscribers.iter() {
            match s {
                Protocol::Bgp(b) => {
                    receivers.push(b.subscribe().await?);
                }
                Protocol::Kernel(k) => {}
            }
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
                route = fused_receivers.next().fuse() => {
                    if let Some(r) = route {
                        match self.register(r) {
                            Ok(_) => {},
                            Err(e) => {
                                tracing::error!(error=?e);
                            }
                        }
                    }
                }
            }
        }
    }

    #[tracing::instrument(skip(self))]
    fn register(&mut self, route: Route) -> Result<(), Error> {
        let mut rib = self.rib.lock().unwrap();
        match rib.insert(route.destination, route) {
            Some(route) => Ok(()),
            None => {
                tracing::error!("failed to insert to rib");
                Err(Error::FailedToInsert)
            }
        }
    }
}
