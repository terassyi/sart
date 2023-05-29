use std::collections::HashMap;

use netlink_packet_core::NetlinkPayload;
use netlink_packet_route::RtnlMessage;
use netlink_sys::{AsyncSocket, SocketAddr};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_stream::StreamExt;

use super::{
    channel::Channel,
    error::Error,
    rib::{RequestType, Route},
};

use rtnetlink::{constants::*, new_connection};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub(crate) struct Kernel {
    pub tables: Vec<u32>,
}

impl Kernel {
    pub fn new(table_ids: Vec<u32>) -> Self {
        Self { tables: table_ids }
    }

    pub async fn subscribe(&self) -> Result<Receiver<(RequestType, Route)>, Error> {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<(RequestType, Route)>(128);

        let groups = RTMGRP_IPV4_ROUTE
            | RTMGRP_IPV4_MROUTE
            | RTMGRP_IPV4_RULE
            | RTMGRP_IPV6_ROUTE
            | RTMGRP_IPV6_MROUTE;
        let (mut conn, mut _handle, mut messages) =
            new_connection().map_err(|e| format!("{}", e)).unwrap();
        // Create new socket that listens for the messages described above.
        let addr = SocketAddr::new(0, groups);
        conn.socket_mut()
            .socket_mut()
            .bind(&addr)
            .expect("Failed to bind");

        tokio::spawn(conn);

        while let Some((message, _)) = messages.next().await {
            match message.payload {
                NetlinkPayload::Done => {
                    println!("Done");
                }
                NetlinkPayload::Error(em) => {
                    eprintln!("Error: {:?}", em);
                }
                NetlinkPayload::Ack(_am) => {}
                NetlinkPayload::Noop => {}
                NetlinkPayload::Overrun(_bytes) => {}
                NetlinkPayload::InnerMessage(m) => {
                    println!("{:?}", m);
                }
                _ => {}
            }
        }

        Ok(rx)
    }

    pub async fn publish(&self) -> Result<(), Error> {
        Ok(())
    }
}

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

    pub fn register(
        &mut self,
        kernel: &Kernel,
        subscriber_tx: Sender<(RequestType, Route)>,
    ) -> Result<(), Error> {
        for id in kernel.tables.iter() {
            if self.tx_map.insert(*id, subscriber_tx.clone()).is_none() {
                return Err(Error::FailedToRegister);
            }
        }
        Ok(())
    }

    pub async fn run(&self) -> Result<(), Error> {
        let (mut conn, mut _handle, mut messages) = new_connection().map_err(Error::StdIoErr)?;
        let addr = SocketAddr::new(0, self.groups);
        conn.socket_mut()
            .socket_mut()
            .bind(&addr)
            .map_err(Error::StdIoErr)?;

        tokio::spawn(conn);

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
                NetlinkPayload::InnerMessage(msg) => {
                    tracing::info!(message=?msg, "netlink message");
                    match msg {
                        RtnlMessage::NewRoute(msg) => {}
                        RtnlMessage::DelAddress(msg) => {}
                        _ => {
                            tracing::info!("other type netlink message");
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
