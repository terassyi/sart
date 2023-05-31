use std::collections::HashMap;

use netlink_packet_core::NetlinkPayload;
use netlink_packet_route::RtnlMessage;
use netlink_sys::{AsyncSocket, SocketAddr};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Receiver, Sender, UnboundedReceiver};
use tokio_stream::StreamExt;

use super::{channel::Channel, error::Error, rib::RequestType, route::Route, rt_client::RtClient};

use rtnetlink::{constants::*, new_connection};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub(crate) struct Kernel {
    pub tables: Vec<u32>,
}

impl Kernel {
    pub fn new(table_ids: Vec<u32>) -> Self {
        Self { tables: table_ids }
    }

    #[tracing::instrument(skip(self, kernel_rx))]
    pub async fn subscribe(
        &self,
        mut kernel_rx: Receiver<(RequestType, Route)>,
    ) -> Result<Receiver<(RequestType, Route)>, Error> {
        let (tx, rx) = tokio::sync::mpsc::channel::<(RequestType, Route)>(128);

        tokio::spawn(async move {
            while let Some((req, route)) = kernel_rx.recv().await {
                // send to rib
                match tx.send((req, route)).await {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!(error=?e,"failed to send to the rib channel")
                    }
                }
            }
        });

        Ok(rx)
    }

    pub async fn publish(&self, req: RequestType, route: Route) -> Result<(), Error> {
        let (conn, handler, _rx) = rtnetlink::new_connection().unwrap();
        tokio::spawn(conn);

        let rt = RtClient::new(handler);

        match req {
            // RequestType::AddRoute => rt.add_route(&route, false),
            RequestType::AddRoute => {}
            RequestType::AddMultiPathRoute => {}
            RequestType::DeleteRoute => {}
            RequestType::DeleteMultiPathRoute => {}
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct KernelRtPoller {
    groups: u32,
    tx_map: HashMap<u32, Sender<(RequestType, Route)>>,
}

impl KernelRtPoller {
    pub fn new() -> KernelRtPoller {
        let groups = RTMGRP_IPV4_ROUTE
            | RTMGRP_IPV4_MROUTE
            | RTMGRP_IPV4_RULE
            | RTMGRP_IPV6_ROUTE
            | RTMGRP_IPV6_MROUTE;

        KernelRtPoller {
            groups,
            tx_map: HashMap::new(),
        }
    }

    #[tracing::instrument(skip(self, subscriber_tx))]
    pub fn register(
        &mut self,
        kernel: &Kernel,
        subscriber_tx: Sender<(RequestType, Route)>,
    ) -> Result<(), Error> {
        for id in kernel.tables.iter() {
            tracing::info!(id = id, "register");
            self.tx_map.insert(*id, subscriber_tx.clone());
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn run(&self) -> Result<(), Error> {
        let (mut conn, mut _handle, mut messages) = new_connection().map_err(Error::StdIoErr)?;
        let addr = SocketAddr::new(0, self.groups);
        conn.socket_mut()
            .socket_mut()
            .bind(&addr)
            .map_err(Error::StdIoErr)?;

        tokio::spawn(conn);

        tracing::info!("start to poll kernel rtnetlink event");
        while let Some((message, _)) = messages.next().await {
            match message.payload {
                NetlinkPayload::Done => {
                    tracing::debug!("netlink message done")
                }
                NetlinkPayload::Error(em) => {
                    tracing::error!(error=?em, "netlink error message")
                }
                NetlinkPayload::Ack(_am) => {}
                NetlinkPayload::Noop => {}
                NetlinkPayload::Overrun(_bytes) => {}
                NetlinkPayload::InnerMessage(msg) => match msg {
                    RtnlMessage::NewRoute(msg) => {
                        let table = msg.header.table as u32;
                        if let Some(tx) = self.tx_map.get(&table) {
                            tracing::info!(table=table,"receive new route rtnetlink message for subscribing table");
                            let route = match Route::try_from(msg) {
                                Ok(route) => route,
                                Err(e) => {
                                    tracing::error!(error=?e, "failed to parse new route message");
                                    continue;
                                }
                            };
                            match tx.send((RequestType::AddRoute, route)).await {
                                Ok(_) => {}
                                Err(e) => {
                                    tracing::error!(error=?e,"failed to send to rib");
                                }
                            }
                        }
                    }
                    RtnlMessage::DelRoute(msg) => {
                        let table = msg.header.table as u32;
                        if let Some(tx) = self.tx_map.get(&table) {
                            tracing::info!(table=table,"receive delete route rtnetlink message for subscribing table");
                            let route = match Route::try_from(msg) {
                                Ok(route) => route,
                                Err(e) => {
                                    tracing::error!(error=?e, "failed to parse new route message");
                                    continue;
                                }
                            };
                            match tx.send((RequestType::DeleteRoute, route)).await {
                                Ok(_) => {}
                                Err(e) => {
                                    tracing::error!(error=?e,"failed to send to rib");
                                }
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        Ok(())
    }
}
