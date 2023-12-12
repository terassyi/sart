use std::{collections::HashMap, net::IpAddr};

use futures::{FutureExt, StreamExt, TryStreamExt};
use ipnet::IpNet;
use netlink_packet_core::NetlinkPayload;
use netlink_packet_route::{
    route::{NextHop, NextHopFlags, Nla},
    RtnlMessage,
};
use netlink_sys::{AsyncSocket, SocketAddr};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{unbounded_channel, Receiver, Sender, UnboundedReceiver, UnboundedSender};

use super::{
    error::Error,
    rib::RequestType,
    route::{ip_version_into, Route},
};

use rtnetlink::{constants::*, new_connection};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Kernel {
    pub tables: Vec<u32>,
    #[serde(skip)]
    publish_tx: Option<UnboundedSender<(RequestType, Route)>>,
}

impl Kernel {
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

    #[tracing::instrument(skip(self))]
    pub async fn publish(&self, req: RequestType, route: Route) -> Result<(), Error> {
        if let Some(tx) = &self.publish_tx {
            for table_id in self.tables.iter() {
                let mut route = route.clone();
                route.table = *table_id as u8;
                tx.send((req, route))
                    .map_err(|_| Error::FailedToRecvSendViaChannel)?;
            }
            Ok(())
        } else {
            tracing::warn!(protocol="kernel",tables=?self.tables, "publisher is not registered. nothing to do.");
            Ok(())
        }
    }

    pub fn register_publisher(&mut self, tx: UnboundedSender<(RequestType, Route)>) {
        self.publish_tx = Some(tx);
    }
}

#[derive(Debug)]
pub struct KernelRtPoller {
    groups: u32,
    tx_map: HashMap<u32, Sender<(RequestType, Route)>>,
    rx: UnboundedReceiver<(RequestType, Route)>,
}

impl KernelRtPoller {
    pub fn new() -> (KernelRtPoller, UnboundedSender<(RequestType, Route)>) {
        let groups = RTMGRP_IPV4_ROUTE
            | RTMGRP_IPV4_MROUTE
            | RTMGRP_IPV4_RULE
            | RTMGRP_IPV6_ROUTE
            | RTMGRP_IPV6_MROUTE;

        let (tx, rx) = unbounded_channel();

        (
            KernelRtPoller {
                groups,
                tx_map: HashMap::new(),
                rx,
            },
            tx,
        )
    }

    #[tracing::instrument(skip(self, subscriber_tx))]
    pub fn register_subscriber(
        &mut self,
        kernel: &Kernel,
        subscriber_tx: Sender<(RequestType, Route)>,
    ) -> Result<(), Error> {
        for id in kernel.tables.iter() {
            tracing::info!(id = id, "register the subscriber");
            self.tx_map.insert(*id, subscriber_tx.clone());
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn run(mut self) -> Result<(), Error> {
        let (mut conn, handle, mut messages) = new_connection().map_err(Error::StdIoErr)?;
        let addr = SocketAddr::new(0, self.groups);
        conn.socket_mut()
            .socket_mut()
            .bind(&addr)
            .map_err(Error::StdIoErr)?;

        tokio::spawn(conn);

        tracing::info!("start to poll kernel rtnetlink event");
        loop {
            futures::select_biased! {
                // subscribe
                message = messages.next().fuse() => {
                    if let Some((message, _)) = message {
                        match message.payload {
                            NetlinkPayload::Done(_) => {
                                tracing::debug!("netlink message done")
                            }
                            NetlinkPayload::Error(em) => {
                                tracing::error!(error=?em, "netlink error message")
                            }
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
                                        match tx.send((RequestType::Add, route)).await {
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
                                        match tx.send((RequestType::Delete, route)).await {
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
                }
                // publish
                request = self.rx.recv().fuse() => {
                    if let Some((req, route)) = request {
                        let res = match req {
                            RequestType::Add | RequestType::Replace => add_route(&handle, req, route).await,
                            RequestType::Delete => delete_route(&handle, route).await,
                            RequestType::AddMultiPath => add_multi_path_route(&handle, route).await,
                            RequestType::DeleteMultiPath => delete_multi_path_route(&handle, route).await,
                        };
                        match res {
                            Ok(_) => {},
                            Err(e) => tracing::error!(error=?e,"failed to handle route with kernel"),
                        }

                    }
                },

            }
        }
    }
}

#[tracing::instrument(skip(handle))]
async fn add_route(
    handle: &rtnetlink::Handle,
    req: RequestType,
    route: Route,
) -> Result<(), Error> {
    tracing::info!("add or replace the rouet to kernel");
    let rt = handle.route();

    let mut existing = false;
    let mut res = rt.get(route.version.clone()).execute();
    while let Some(r) = res.try_next().await? {
        if let Some((dst, prefix_len)) = r.destination_prefix() {
            if route.destination.prefix_len() == prefix_len
                && route.destination.addr().eq(&dst)
                && r.header.table == route.table
            {
                tracing::info!(
                    destination = route.destination.to_string(),
                    "an existing route is found"
                );
                // replace or not
                if req == RequestType::Replace {
                    tracing::info!(
                        destination = route.destination.to_string(),
                        "replace the existing route"
                    );
                    rt.del(r).execute().await?;
                    existing = true;
                }
            }
        }
    }
    if !(req == RequestType::Replace) && existing {
        return Err(Error::AlreadyExists);
    }
    let mut request = rt
        .add()
        .table_id(route.table as u32)
        .kind(route.kind as u8)
        .protocol(route.protocol.into())
        .scope(route.scope as u8);
    let msg = request.message_mut();
    msg.header.address_family = ip_version_into(&route.version);
    // destination
    msg.header.destination_prefix_length = route.destination.prefix_len();
    match route.destination.addr() {
        IpAddr::V4(addr) => msg.nlas.push(Nla::Destination(addr.octets().to_vec())),
        IpAddr::V6(addr) => msg.nlas.push(Nla::Destination(addr.octets().to_vec())),
    };
    // source
    if let Some(src) = route.source {
        match src {
            IpAddr::V4(addr) => msg.nlas.push(Nla::Source(addr.octets().to_vec())),
            IpAddr::V6(addr) => msg.nlas.push(Nla::Source(addr.octets().to_vec())),
        }
    }
    // priority
    if route.priority != 0 {
        msg.nlas.push(Nla::Priority(route.priority));
    }
    // next hops
    if route.next_hops.len() > 1 {
        // multiple paths
        let n = route
            .next_hops
            .iter()
            .map(|h| {
                // we don't have any convinient way to create nexthop struct
                let mut next = NextHop::default();
                next.flags = NextHopFlags::from_bits_truncate(h.flags as u8);
                if h.weight > 0 {
                    next.hops = (h.weight - 1) as u8;
                }
                if h.interface != 0 {
                    next.interface_id = h.interface;
                }
                match h.gateway {
                    IpAddr::V4(addr) => next.nlas.push(Nla::Gateway(addr.octets().to_vec())),
                    IpAddr::V6(addr) => next.nlas.push(Nla::Gateway(addr.octets().to_vec())),
                }
                next
            })
            .collect();
        msg.nlas.push(Nla::MultiPath(n));
    } else if route.next_hops.is_empty() {
        return Err(Error::GatewayNotFound);
    } else {
        match route.next_hops[0].gateway {
            IpAddr::V4(addr) => msg.nlas.push(Nla::Gateway(addr.octets().to_vec())),
            IpAddr::V6(addr) => msg.nlas.push(Nla::Gateway(addr.octets().to_vec())),
        }
    }

    tracing::info!("send the adding route message to kernel");
    match route.version {
        rtnetlink::IpVersion::V4 => request.v4().execute().await?,
        rtnetlink::IpVersion::V6 => request.v6().execute().await?,
    }

    Ok(())
}

#[tracing::instrument(skip(handle))]
async fn delete_route(handle: &rtnetlink::Handle, route: Route) -> Result<(), Error> {
    tracing::info!("delete the route from kernel");
    let rt = handle.route();
    let mut res = rt.get(route.version.clone()).execute();

    while let Some(r) = res.try_next().await? {
        if let Some((dst, prefix_len)) = r.destination_prefix() {
            if route.destination.prefix_len() == prefix_len
                && route.destination.addr().eq(&dst)
                && route.table == r.header.table
            {
                tracing::info!("send the deleting message to kernel");
                rt.del(r).execute().await?;
            }
        }
    }
    Err(Error::DestinationNotFound)
}

#[tracing::instrument(skip(handle))]
async fn add_multi_path_route(handle: &rtnetlink::Handle, route: Route) -> Result<(), Error> {
    tracing::info!("add a multiple paths route to kernel");
    let mut r = get_route(handle, route.table, &route.version, &route.destination).await?;
    for mut next_hop in route.next_hops.into_iter() {
        next_hop.weight = 1;
        r.next_hops.push(next_hop);
    }

    add_route(handle, RequestType::Replace, r).await
}

#[tracing::instrument(skip(handle))]
async fn delete_multi_path_route(handle: &rtnetlink::Handle, route: Route) -> Result<(), Error> {
    let mut r = get_route(handle, route.table, &route.version, &route.destination).await?;

    r.next_hops
        .retain(|n| route.next_hops.iter().any(|nn| nn.gateway.ne(&n.gateway)));

    add_route(handle, RequestType::Replace, r).await
}

#[tracing::instrument(skip(handle))]
async fn list_routes(
    handle: &rtnetlink::Handle,
    table: u8,
    ip_version: &rtnetlink::IpVersion,
) -> Result<Vec<Route>, Error> {
    tracing::info!("list routes from kernel");
    let rt = handle.route();
    let mut res = rt.get(ip_version.clone()).execute();
    let mut routes = Vec::new();
    while let Some(msg) = res.try_next().await? {
        if msg.header.table != table || msg.header.address_family != ip_version_into(ip_version) {
            continue;
        }
        routes.push(Route::try_from(msg)?);
    }
    Ok(routes)
}

#[tracing::instrument(skip(handle))]
async fn get_route(
    handle: &rtnetlink::Handle,
    table: u8,
    ip_version: &rtnetlink::IpVersion,
    destination: &IpNet,
) -> Result<Route, Error> {
    let routes = list_routes(handle, table, ip_version).await?;
    routes
        .into_iter()
        .find(|r| r.destination.eq(destination))
        .ok_or(Error::DestinationNotFound)
}
