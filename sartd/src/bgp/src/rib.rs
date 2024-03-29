use ipnet::IpNet;
use std::{
    cmp::Ordering,
    collections::HashMap,
    net::{IpAddr, Ipv4Addr},
};
use tokio::sync::mpsc::Sender;

use super::path::PathKind;
use sartd_proto::sart::{
    AddMultiPathRouteRequest, AddRouteRequest, DeleteMultiPathRouteRequest, DeleteRouteRequest,
};

use super::{
    api_server::ApiResponse,
    error::{ControlError, Error, RibError},
    event::RibEvent,
    family::{AddressFamily, Afi},
    packet::attribute::Attribute,
    path::{BestPathReason, Path, PathBuilder},
    peer::neighbor::NeighborPair,
    server::Bgp,
};

#[derive(Debug)]
pub struct Table {
    inner: HashMap<IpNet, Path>,
    received: usize,
    dropped: usize,
}

impl Table {
    fn new() -> Self {
        Self {
            inner: HashMap::new(),
            received: 0,
            dropped: 0,
        }
    }

    fn insert(&mut self, prefix: IpNet, path: Path) -> Option<Path> {
        self.received += 1;
        self.inner.insert(prefix, path)
    }

    fn remove(&mut self, prefix: &IpNet) -> Option<Path> {
        self.dropped += 1;
        self.inner.remove(prefix)
    }

    fn get(&self, prefix: &IpNet) -> Option<&Path> {
        self.inner.get(prefix)
    }

    fn get_mut(&mut self, prefix: &IpNet) -> Option<&mut Path> {
        self.inner.get_mut(prefix)
    }

    fn get_all(&self) -> Vec<&Path> {
        self.inner.values().collect()
    }

    fn clear(&mut self) {
        self.inner = HashMap::new();
        self.received = 0;
        self.dropped = 0;
    }
}

#[derive(Debug)]
pub struct AdjRib {
    table: HashMap<AddressFamily, Table>,
}

impl AdjRib {
    pub fn new(families: Vec<AddressFamily>) -> Self {
        let mut table = HashMap::new();
        for family in families.into_iter() {
            table.insert(family, Table::new());
        }
        Self { table }
    }

    pub fn add_reach(&mut self, family: AddressFamily) {
        match self.table.get(&family) {
            Some(_) => {}
            None => {
                self.table.insert(family, Table::new());
            }
        }
    }

    #[tracing::instrument(skip(self, prefix,path), fields(prefix=prefix.to_string(), path_id=path.id))]
    pub fn insert(
        &mut self,
        family: &AddressFamily,
        prefix: IpNet,
        path: Path,
    ) -> Result<Option<Path>, RibError> {
        match self.table.get_mut(family) {
            Some(table) => Ok(table.insert(prefix, path)),
            None => Err(RibError::AddressFamilyNotSet),
        }
    }

    pub fn get(&self, family: &AddressFamily, prefix: &IpNet) -> Option<&Path> {
        match self.table.get(family) {
            Some(table) => table.get(prefix),
            None => None,
        }
    }

    pub fn get_mut(&mut self, family: &AddressFamily, prefix: &IpNet) -> Option<&mut Path> {
        match self.table.get_mut(family) {
            Some(table) => table.get_mut(prefix),
            None => None,
        }
    }

    pub fn get_all(&self, family: &AddressFamily) -> Option<Vec<&Path>> {
        match self.table.get(family) {
            Some(table) => match table.inner.is_empty() {
                true => None,
                false => Some(table.get_all()),
            },
            None => None,
        }
    }

    pub fn prefixes(
        &self,
        family: &AddressFamily,
    ) -> Option<std::collections::hash_map::Keys<'_, IpNet, Path>> {
        self.table.get(family).map(|table| table.inner.keys())
    }

    pub fn remove(&mut self, family: &AddressFamily, prefix: &IpNet) -> Option<Path> {
        match self.table.get_mut(family) {
            Some(table) => table.remove(prefix),
            None => None,
        }
    }

    pub fn clear(&mut self) {
        for (_family, table) in self.table.iter_mut() {
            table.clear();
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum RibKind {
    In = 1,
    Out = 2,
}

impl TryFrom<u32> for RibKind {
    type Error = Error;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(RibKind::In),
            2 => Ok(RibKind::Out),
            _ => Err(Error::Rib(RibError::UnknownRibKind)),
        }
    }
}

#[derive(Debug)]
pub struct LocRib {
    table: HashMap<Afi, HashMap<IpNet, Vec<Path>>>,
    multi_path: bool,
    received: usize,
    dropped: usize,
}

impl LocRib {
    pub fn new(protocols: Vec<Afi>, multi_path: bool) -> Self {
        let mut loc_rib = Self {
            table: HashMap::new(),
            multi_path,
            received: 0,
            dropped: 0,
        };
        for protocol in protocols.into_iter() {
            loc_rib.table.insert(protocol, HashMap::new());
        }
        loc_rib
    }

    fn set_multipath(&mut self, enable: bool) -> Result<(), Error> {
        self.multi_path = enable;
        Ok(())
    }

    fn set_protocol(&mut self, protocol: Afi) -> Result<(), Error> {
        match self.table.get(&protocol) {
            Some(_) => Err(Error::Rib(RibError::ProtocolIsAlreadyRegistered)),
            None => {
                self.table.insert(protocol, HashMap::new());
                Ok(())
            }
        }
    }

    #[tracing::instrument(skip(self))]
    fn get(&self, family: &AddressFamily, prefix: &IpNet) -> Option<&Vec<Path>> {
        match self.table.get(&family.afi) {
            Some(table) => table.get(prefix),
            None => None,
        }
    }

    fn get_mut(&mut self, family: &AddressFamily, prefix: &IpNet) -> Option<&mut Vec<Path>> {
        match self.table.get_mut(&family.afi) {
            Some(table) => table.get_mut(prefix),
            None => None,
        }
    }

    fn get_by_id(&self, family: &AddressFamily, prefix: &IpNet, id: u64) -> Option<&Path> {
        match self.get(family, prefix) {
            Some(paths) => paths.iter().find(|p| p.id == id),
            None => None,
        }
    }

    fn get_best_path(&self, family: &AddressFamily, prefix: &IpNet) -> Option<Vec<&Path>> {
        self.get(family, prefix)
            .map(|paths| paths.iter().filter(|p| p.best).collect::<Vec<&Path>>())
    }

    fn get_all_best(&self, family: &AddressFamily) -> Option<Vec<Path>> {
        match self.table.get(&family.afi) {
            Some(table) => {
                let mut best_paths = Vec::new();
                for (_prefix, paths) in table.iter() {
                    if paths.is_empty() {
                        continue;
                    }
                    // paths[0] must be best
                    // TODO: handle add_path capability
                    best_paths.push(paths[0].clone());
                }
                match best_paths.is_empty() {
                    true => None,
                    false => Some(best_paths),
                }
            }
            None => None,
        }
    }

    fn get_all(&self, family: &AddressFamily) -> Option<Vec<&Path>> {
        match self.table.get(&family.afi) {
            Some(table) => {
                let mut all = Vec::new();
                for (_prefix, paths) in table.iter() {
                    for path in paths.iter() {
                        all.push(path);
                    }
                }
                Some(all)
            }
            None => None,
        }
    }

    #[tracing::instrument(skip(self, family, path), fields(prefix=path.prefix().to_string(),path_id=path.id))]
    fn insert(&mut self, family: &AddressFamily, path: &mut Path) -> Result<LocRibStatus, Error> {
        let prefix = path.prefix;
        let table = self
            .table
            .get_mut(&family.afi)
            .ok_or(Error::Rib(RibError::InvalidAddressFamily))?;

        self.received += 1;

        if let Some(paths) = table.get_mut(&path.prefix) {
            if paths.is_empty() {
                path.best = true;
                path.reason = BestPathReason::OnlyPath;
                paths.push(path.clone());
                return Ok(LocRibStatus::BestPathChanged(prefix, path.id));
            }

            let best_idx = paths
                .iter()
                .rposition(|p| p.best)
                .ok_or(Error::Rib(RibError::PathNotFound))?;
            for p in paths.iter_mut() {
                if p.eq(&path) {
                    // we should sort again
                    p.timestamp = path.timestamp;
                }
            }
            // best path selection
            let funcs = get_comparison_funcs(self.multi_path);
            let mut idx = 0;
            let mut reason = BestPathReason::NotBest;
            for p in paths.iter() {
                for (r, f) in funcs.iter() {
                    let res = f(&path, p);
                    if res == Ordering::Less {
                        reason = *r;
                        break;
                    }
                }
                if reason != BestPathReason::NotBest {
                    tracing::info!(reason=?reason,"Best path is decided");
                    break;
                }
                idx += 1;
            }
            // checking weather best paths are changed
            if self.multi_path {
                if best_idx >= idx {
                    // idx should be 0
                    // in this case, the best path is updated by a new path, and existing best paths are unmarked
                    path.best = true;
                    path.reason = reason;
                    for p in paths.iter_mut() {
                        if p.best {
                            p.best = false;
                            p.reason = BestPathReason::NotBest;
                        }
                    }

                    paths.insert(idx, path.clone());
                    tracing::info!("best path is changed");
                    return Ok(LocRibStatus::BestPathChanged(prefix, path.id));
                } else if best_idx + 1 == idx {
                    if path.is_equal_cost(&paths[best_idx]) {
                        // a new path is one of the best paths
                        path.best = true;
                        path.reason = BestPathReason::EqualCostMiltiPath;
                        if paths[best_idx].reason != BestPathReason::EqualCostMiltiPath {
                            paths[best_idx].reason = BestPathReason::EqualCostMiltiPath;
                        }

                        paths.insert(idx, path.clone());
                        return Ok(LocRibStatus::MultiPathAdded);
                    }

                    paths.insert(idx, path.clone());
                    return Ok(LocRibStatus::NotChanged);
                }
            } else if idx == 0 {
                path.best = true;
                path.reason = reason;

                // unmark old best path
                paths[0].best = false;
                paths[0].reason = BestPathReason::NotBest;

                paths.insert(idx, path.clone());
                tracing::info!("best path is changed");
                return Ok(LocRibStatus::BestPathChanged(prefix, path.id));
            }

            paths.insert(idx, path.clone());
        } else {
            path.best = true;
            path.reason = BestPathReason::OnlyPath;
            table.insert(path.prefix, vec![path.clone()]);
            tracing::info!("create new loc-rib entry");

            return Ok(LocRibStatus::BestPathChanged(prefix, path.id));
        }

        Ok(LocRibStatus::NotChanged)
    }

    #[tracing::instrument(skip(self, family, prefix), fields(prefix=prefix.to_string(),path_id=id))]
    fn drop(
        &mut self,
        family: &AddressFamily,
        prefix: &IpNet,
        id: u64,
    ) -> Result<LocRibStatus, Error> {
        tracing::info!("drop from loc-rib");
        let table = self
            .table
            .get_mut(&family.afi)
            .ok_or(Error::Rib(RibError::InvalidAddressFamily))?;
        if let Some(paths) = table.get_mut(prefix) {
            if paths.is_empty() {
                return Err(Error::Rib(RibError::PathNotFound));
            }

            self.dropped += 1;

            if let Some(idx) = paths.iter().position(|p| p.id == id) {
                let removed = paths.remove(idx);
                if removed.best {
                    // recalculate best path
                    if paths.is_empty() {
                        return Ok(LocRibStatus::Withdrawn(*prefix, removed.kind()));
                    }
                    if paths.len() == 1 {
                        paths[0].best = true;
                        paths[0].reason = BestPathReason::OnlyPath;
                        return Ok(LocRibStatus::BestPathChanged(*prefix, paths[0].id));
                    }
                    if self.multi_path {
                        let next_hop = if removed.next_hops.is_empty() {
                            None
                        } else {
                            Some(removed.next_hops[0])
                        };
                        if idx == 0 {
                            // when idx is best, we don't have to update best paths.
                            if paths[idx].best {
                                // TODO: the best path selected reason may be changed
                                return Ok(LocRibStatus::MultiPathWithdrawn(
                                    *prefix,
                                    paths[0].id,
                                    next_hop,
                                ));
                            }
                            // when idx is not best, we have to calculate best path again with considering multiple best paths.
                            let funcs = get_comparison_funcs(self.multi_path);
                            for (r, f) in funcs.iter() {
                                if f(&paths[0], &paths[1]) == Ordering::Less {
                                    paths[0].reason = *r;
                                    break;
                                }
                            }
                            paths[0].best = true;
                            if paths[0].reason == BestPathReason::NotBest
                                || paths[0].reason == BestPathReason::EqualCostMiltiPath
                            {
                                for i in 0..(paths.len() - 1) {
                                    if !paths[i].is_equal_cost(&paths[i + 1]) {
                                        break;
                                    }
                                    paths[i].reason = BestPathReason::EqualCostMiltiPath;
                                    paths[i + 1].best = true;
                                    paths[i + 1].reason = BestPathReason::EqualCostMiltiPath;
                                }
                            }
                            return Ok(LocRibStatus::BestPathChanged(*prefix, paths[0].id));
                        }
                        // when idx > 0 and it is best
                        return Ok(LocRibStatus::MultiPathWithdrawn(
                            *prefix,
                            paths[0].id,
                            next_hop,
                        ));
                    } else {
                        paths[0].best = true; // best path must exist at index 0
                        for (r, f) in get_comparison_funcs(self.multi_path).iter() {
                            if f(&paths[0], &paths[1]) == Ordering::Less {
                                paths[0].reason = *r;
                                break;
                            }
                        }
                        return Ok(LocRibStatus::BestPathChanged(*prefix, paths[0].id));
                    }
                } else {
                    // TODO: even if removed path is not the best, the best path selected reason may be changed
                    return Ok(LocRibStatus::NotChanged);
                }
            }
        }
        Err(Error::Rib(RibError::PathNotFound))
    }
}

#[derive(Debug, PartialEq, PartialOrd)]
enum LocRibStatus {
    NotChanged,
    Withdrawn(IpNet, PathKind),
    BestPathChanged(IpNet, u64),
    MultiPathAdded,
    MultiPathWithdrawn(IpNet, u64, Option<IpAddr>),
    AddtionalPathAdded(IpNet, u64),
    AdditionalPathWithdrawn(IpNet, u64),
}

#[derive(Debug)]
pub struct RibManager {
    asn: u32,
    router_id: Ipv4Addr,
    loc_rib: LocRib,
    endpoint: Option<sartd_proto::sart::fib_api_client::FibApiClient<tonic::transport::Channel>>,
    table_id: u8,
    peers_tx: HashMap<NeighborPair, Sender<RibEvent>>,
    api_tx: Sender<ApiResponse>,
}

impl RibManager {
    #[tracing::instrument(skip(api_tx))]
    pub async fn new(
        asn: u32,
        router_id: Ipv4Addr,
        endpoint: Option<String>,
        table_id: u8,
        protocols: Vec<Afi>,
        multi_path_enabled: bool,
        api_tx: Sender<ApiResponse>,
    ) -> Result<Self, Error> {
        let endpoint = match endpoint {
            Some(endpoint) => {
                match sartd_proto::sart::fib_api_client::FibApiClient::connect(format!(
                    "http://{}",
                    endpoint
                ))
                .await
                {
                    Ok(conn) => {
                        tracing::info!("success to connect to fib endpoint");
                        Some(conn)
                    }
                    Err(e) => {
                        tracing::error!("{}", e);
                        return Err(Error::Rib(RibError::FailedToConnectToFibEndpoint));
                    }
                }
            }
            None => {
                tracing::warn!("fib endpoint is not configured");
                None
            }
        };
        Ok(Self {
            asn,
            router_id,
            loc_rib: LocRib::new(protocols, multi_path_enabled),
            endpoint,
            table_id,
            peers_tx: HashMap::new(),
            api_tx,
        })
    }

    #[tracing::instrument(skip(self, event))]
    pub async fn handle(&mut self, event: RibEvent) -> Result<(), Error> {
        tracing::info!(event=%event);
        match event {
            RibEvent::SetAsn(asn) => {
                self.asn = asn;
                Ok(())
            }
            RibEvent::SetRouterId(id) => {
                self.router_id = id;
                Ok(())
            }
            RibEvent::SetMultiPath(enable) => {
                self.set_multipath(enable)
            }
            RibEvent::AddPeer(neighbor, rib_event_tx) => self.add_peer(neighbor, rib_event_tx),
            RibEvent::DeletePeer(neighbor) => self.delete_peer(neighbor),
            RibEvent::Init(family, neighbor) => self.init(family, neighbor).await,
            RibEvent::Flush(family, neighbor) => self.flush(family, neighbor).await,
            RibEvent::AddPath(networks, attrs) => self.add_path(networks, attrs).await,
            RibEvent::DeletePath(family, network) => self.delete_path(family, network).await,
            RibEvent::InstallPaths(neighbor, paths) => {
                self.install_paths(neighbor, paths, true).await
            }
            RibEvent::DropPaths(neighbor, family, path_ids) => {
                self.drop_paths(neighbor, family, path_ids, true).await
            }
            RibEvent::GetPath(family) => self.get_path(family).await,
            RibEvent::GetPathByPrefix(prefix, family) => {
                self.get_path_by_prefix(prefix, family).await
            }
            _ => Err(Error::Rib(RibError::UnhandlableEvent)),
        }
    }

    #[tracing::instrument(skip(self, tx))]
    fn add_peer(&mut self, neighbor: NeighborPair, tx: Sender<RibEvent>) -> Result<(), Error> {
        match self.peers_tx.get(&neighbor) {
            Some(_) => Err(Error::Rib(RibError::PeerAlreadyRegistered)),
            None => {
                self.peers_tx.insert(neighbor, tx);
                Ok(())
            }
        }
    }

    #[tracing::instrument(skip(self))]
    fn delete_peer(&mut self, neighbor: NeighborPair) -> Result<(), Error> {
        tracing::warn!("delete peer from rib manager");
        match self.peers_tx.remove(&neighbor) {
            Some(_neighbor) => Ok(()),
            None => Err(Error::Rib(RibError::PeerNotFound)),
        }
    }

    fn set_multipath(&mut self, enable: bool) -> Result<(), Error> {
        self.loc_rib.set_multipath(enable)
    }

    #[tracing::instrument(skip(self, neighbor), fields(peer.asn=neighbor.asn,peer.addr=neighbor.addr.to_string()))]
    async fn init(&self, family: AddressFamily, neighbor: NeighborPair) -> Result<(), Error> {
        tracing::debug!("initialize path");
        if let Some(paths) = self.loc_rib.get_all_best(&family) {
            // exclude best paths from target neighbor
            let p = paths
                .iter()
                .filter(|&p| {
                    !p.peer_addr.eq(&neighbor.addr)
                        || if self.asn == neighbor.asn {
                            // ibgp peer
                            p.kind() != PathKind::Internal
                        } else {
                            false
                        }
                })
                .cloned()
                .collect::<Vec<Path>>();
            if let Some(tx) = self.peers_tx.get(&neighbor) {
                let prefixes = p
                    .iter()
                    .map(|pp| pp.prefix().to_string())
                    .collect::<Vec<String>>();
                tracing::info!(prefixes=?prefixes, "advertise initially installed paths");
                tx.send(RibEvent::Advertise(p))
                    .await
                    .map_err(|_| Error::Control(ControlError::FailedToSendRecvChannel))?;
            } else {
                return Err(Error::Rib(RibError::PeerNotFound));
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self, neighbor), fields(peer.asn=neighbor.asn,peer.addr=neighbor.addr.to_string()))]
    async fn flush(&self, family: AddressFamily, neighbor: NeighborPair) -> Result<(), Error> {
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn add_path(&mut self, networks: Vec<IpNet>, attrs: Vec<Attribute>) -> Result<(), Error> {
        let mut builder = PathBuilder::builder_local(self.router_id, self.asn);
        for attr in attrs.into_iter() {
            builder.attr(attr)?;
        }
        let paths = builder
            .nlri(networks.iter().map(|&n| n.into()).collect())
            .build()?;
        self.install_paths(
            NeighborPair::new(IpAddr::V4(self.router_id), self.asn),
            paths,
            false,
        )
        .await
    }

    #[tracing::instrument(skip(self))]
    async fn delete_path(
        &mut self,
        family: AddressFamily,
        network: Vec<IpNet>,
    ) -> Result<(), Error> {
        let mut path_ids = Vec::new();
        for prefix in network.iter() {
            if let Some(stored_paths) = self.loc_rib.get(&family, prefix) {
                let local_paths: Vec<&Path> = stored_paths
                    .iter()
                    .filter(|p| p.is_local_originated())
                    .collect();
                for local_path in local_paths.iter() {
                    path_ids.push((local_path.prefix(), local_path.id));
                }
            }
        }

        self.drop_paths(
            NeighborPair::new(IpAddr::V4(self.router_id), self.asn),
            family,
            path_ids,
            false,
        )
        .await
    }

    #[tracing::instrument(skip(self, neighbor), fields(peer.asn=neighbor.asn,peer.addr=neighbor.addr.to_string()))]
    async fn install_paths(
        &mut self,
        neighbor: NeighborPair,
        mut paths: Vec<Path>,
        apply_to_fib: bool,
    ) -> Result<(), Error> {
        if paths.is_empty() {
            return Ok(());
        }
        let family = paths[0].family;

        let mut changed_prefixes = Vec::new();
        for path in paths.iter_mut() {
            match self.loc_rib.insert(&family, path)? {
                LocRibStatus::BestPathChanged(prefix, id) => {
                    tracing::info!(family=?family,prefix=?prefix,path_id=id,"best path changed");
                    changed_prefixes.push((prefix, id));

                    // send the new path to fib endpoint
                    let ibgp_path = !path.is_external();
                    let priority = if ibgp_path {
                        Bgp::AD_IBGP
                    } else {
                        Bgp::AD_EBGP
                    };
                    if apply_to_fib && self.endpoint.is_some() {
                        if path.next_hops.is_empty() {
                            return Err(Error::Rib(RibError::NextHopMustBeSet));
                        }
                        let next_hop = path.next_hops[0]; // must have at least one next_hop // TODO: handle multiple next_hops(ipv6)
                        let endpoint = self.endpoint.as_mut().unwrap();
                        tracing::info!("apply to fib endpoint to add the route");
                        endpoint
                            .add_route(AddRouteRequest {
                                table: self.table_id as u32,
                                version: path.family().afi.inet() as i32,
                                route: Some(sartd_proto::sart::Route {
                                    table: self.table_id as u32,
                                    version: path.family().afi.inet() as i32,
                                    destination: path.prefix.to_string(),
                                    protocol: Bgp::RTPROTO_BGP as i32,
                                    scope: Bgp::RT_SCOPE_UNIVERSE as i32,
                                    r#type: path.family().safi.inet() as i32,
                                    next_hops: vec![sartd_proto::sart::NextHop {
                                        gateway: next_hop.to_string(),
                                        weight: priority as u32,
                                        flags: 0,
                                        interface: 0,
                                    }],
                                    source: String::new(),
                                    ad: priority as i32,
                                    priority: priority as u32,
                                    ibgp: ibgp_path,
                                }),
                                replace: true,
                            })
                            .await
                            .map_err(|e| Error::Endpoint { e })?;
                    }
                }
                // TODO: should send event to fib endpoint
                LocRibStatus::MultiPathAdded => {
                    tracing::info!(family=?family,prefix=?path.prefix,"multiple best paths added");
                    // send the new path to fib endpoint
                    let ibgp_path = !path.is_external();
                    let priority = if ibgp_path {
                        Bgp::AD_IBGP
                    } else {
                        Bgp::AD_EBGP
                    };
                    if apply_to_fib && self.endpoint.is_some() {
                        if path.next_hops.is_empty() {
                            return Err(Error::Rib(RibError::NextHopMustBeSet));
                        }
                        let next_hop = path.next_hops[0]; // must have at least one next_hop // TODO: handle multiple next_hops(ipv6)
                        tracing::info!("apply to fib endpoint to add the multi path");
                        let endpoint = self.endpoint.as_mut().unwrap();
                        endpoint
                            .add_multi_path_route(AddMultiPathRouteRequest {
                                table: self.table_id as u32,
                                version: path.family().afi.inet() as i32,
                                route: Some(sartd_proto::sart::Route {
                                    table: self.table_id as u32,
                                    version: path.family().afi.inet() as i32,
                                    destination: path.prefix.to_string(),
                                    protocol: Bgp::RTPROTO_BGP as i32,
                                    scope: Bgp::RT_SCOPE_UNIVERSE as i32,
                                    r#type: path.family().safi.inet() as i32,
                                    next_hops: vec![sartd_proto::sart::NextHop {
                                        gateway: next_hop.to_string(),
                                        weight: priority as u32,
                                        flags: 0,
                                        interface: 0,
                                    }],
                                    source: String::new(),
                                    ad: priority as i32,
                                    priority: priority as u32,
                                    ibgp: ibgp_path,
                                }),
                            })
                            .await
                            .map_err(|e| Error::Endpoint { e })?;
                    }
                }
                _ => {}
            }
        }

        for (peer, tx) in self.peers_tx.iter() {
            let mut advertise = Vec::new();
            for (prefix, id) in changed_prefixes.iter() {
                let p = self
                    .loc_rib
                    .get_by_id(&family, prefix, *id)
                    .ok_or(Error::Rib(RibError::PathNotFound))?;

                // filter an advertising path to each peer
                match p.kind() {
                    PathKind::Local => {}
                    PathKind::External => {
                        if peer.eq(&neighbor) {
                            continue;
                        }
                    }
                    PathKind::Internal => {
                        // if peer is the external peer(has different as number), advertise paths
                        if self.asn == peer.asn {
                            continue;
                        }
                    }
                }

                advertise.push(p.clone());
            }
            if !advertise.is_empty() {
                tx.send(RibEvent::Advertise(advertise))
                    .await
                    .map_err(|_e| Error::Control(ControlError::FailedToSendRecvChannel))?;
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self, neighbor), fields(peer.asn=neighbor.asn,peer.addr=neighbor.addr.to_string()))]
    async fn drop_paths(
        &mut self,
        neighbor: NeighborPair,
        family: AddressFamily,
        path_ids: Vec<(IpNet, u64)>,
        apply_to_fib: bool,
    ) -> Result<(), Error> {
        let mut advertise_prefixes = Vec::new();
        let mut withdraw_prefixes = Vec::new();
        for (prefix, path_id) in path_ids.iter() {
            match self.loc_rib.drop(&family, prefix, *path_id)? {
                LocRibStatus::BestPathChanged(prefix, id) => {
                    tracing::info!(new_best_id = id, "best path changed");
                    advertise_prefixes.push((prefix, id));

                    // send the new path to fib endpoint
                    let path = match self.loc_rib.get_by_id(&family, &prefix, id) {
                        Some(path) => path,
                        None => return Err(Error::Rib(RibError::PathNotFound)),
                    };
                    let ibgp_path = !path.is_external();
                    let priority = if ibgp_path {
                        Bgp::AD_IBGP
                    } else {
                        Bgp::AD_EBGP
                    };
                    if apply_to_fib && self.endpoint.is_some() {
                        if path.next_hops.is_empty() {
                            return Err(Error::Rib(RibError::NextHopMustBeSet));
                        }
                        let next_hop = path.next_hops[0]; // must have at least one next_hop // TODO: handle multiple next_hops(ipv6)
                        let endpoint = self.endpoint.as_mut().unwrap();
                        tracing::info!("apply to fib endpoint to replace the route");
                        endpoint
                            .add_route(AddRouteRequest {
                                table: self.table_id as u32,
                                version: path.family().afi.inet() as i32,
                                route: Some(sartd_proto::sart::Route {
                                    table: self.table_id as u32,
                                    version: path.family().afi.inet() as i32,
                                    destination: path.prefix.to_string(),
                                    protocol: Bgp::RTPROTO_BGP as i32,
                                    scope: Bgp::RT_SCOPE_UNIVERSE as i32,
                                    r#type: path.family().safi.inet() as i32,
                                    next_hops: vec![sartd_proto::sart::NextHop {
                                        gateway: next_hop.to_string(),
                                        weight: priority as u32,
                                        flags: 0,
                                        interface: 0,
                                    }],
                                    source: String::new(),
                                    ad: priority as i32,
                                    priority: priority as u32,
                                    ibgp: ibgp_path,
                                }),
                                replace: true,
                            })
                            .await
                            .map_err(|e| Error::Endpoint { e })?;
                    }
                }
                LocRibStatus::MultiPathWithdrawn(prefix, id, gateway) => {
                    // TODO: update fib
                    tracing::info!("multiple best paths withdrawn");
                    let path = match self.loc_rib.get_by_id(&family, &prefix, id) {
                        Some(path) => path,
                        None => return Err(Error::Rib(RibError::PathNotFound)),
                    };
                    if apply_to_fib && self.endpoint.is_some() && gateway.is_some() {
                        let endpoint = self.endpoint.as_mut().unwrap();
                        tracing::info!("apply to fib endpoint to withdraw multi path");
                        endpoint
                            .delete_multi_path_route(DeleteMultiPathRouteRequest {
                                table: self.table_id as u32,
                                version: path.family().afi.inet() as i32,
                                destination: prefix.to_string(),
                                gateways: vec![gateway.unwrap().to_string()],
                            })
                            .await
                            .map_err(|e| Error::Endpoint { e })?;
                    }
                }
                LocRibStatus::Withdrawn(prefix, kind) => {
                    withdraw_prefixes.push((prefix, kind));
                    tracing::info!(path_kind=?kind,"all path are withdrawn");

                    // send to delete the path
                    if apply_to_fib && self.endpoint.is_some() {
                        let endpoint = self.endpoint.as_mut().unwrap();
                        tracing::info!("apply to fib endpoint to delete the route");
                        endpoint
                            .delete_route(DeleteRouteRequest {
                                table: self.table_id as u32,
                                version: family.afi.inet() as i32,
                                destination: prefix.to_string(),
                            })
                            .await
                            .map_err(|e| Error::Endpoint { e })?;
                    }
                }
                _ => {}
            }
        }

        // advertise withdrawn paths to peers
        for (peer, tx) in self.peers_tx.iter() {
            let mut withdraw = Vec::new();
            for (prefix, kind) in withdraw_prefixes.iter() {
                match kind {
                    PathKind::Local => {}
                    PathKind::External => {
                        if peer.eq(&neighbor) {
                            continue;
                        }
                    }
                    PathKind::Internal => {
                        if self.asn == peer.asn {
                            continue;
                        }
                    }
                }
                withdraw.push(*prefix);
            }

            // advertise withdrawn paths
            if !withdraw.is_empty() {
                tx.send(RibEvent::Withdraw(withdraw))
                    .await
                    .map_err(|_e| Error::Control(ControlError::FailedToSendRecvChannel))?;
            }
        }

        // advertise new best paths
        for (peer, tx) in self.peers_tx.iter() {
            let mut advertise = Vec::new();
            for (prefix, id) in advertise_prefixes.iter() {
                let p = self
                    .loc_rib
                    .get_by_id(&family, prefix, *id)
                    .ok_or(Error::Rib(RibError::PathNotFound))?;

                // filter an advertising path to each peer

                match p.kind() {
                    PathKind::Local => {}
                    PathKind::External => {}
                    PathKind::Internal => {
                        // if peer is the external peer(has different as number), advertise paths
                        if self.asn == peer.asn {
                            continue;
                        }
                    }
                }
                advertise.push(p.clone());
            }

            if !advertise.is_empty() {
                tx.send(RibEvent::Advertise(advertise))
                    .await
                    .map_err(|_e| Error::Control(ControlError::FailedToSendRecvChannel))?;
            }
        }
        Ok(())
    }

    async fn get_path(&self, family: AddressFamily) -> Result<(), Error> {
        if let Some(all_paths) = self.loc_rib.get_all(&family) {
            let paths = all_paths
                .iter()
                .map(|&p| sartd_proto::sart::Path::from(p))
                .collect();
            self.api_tx
                .send(ApiResponse::Paths(paths))
                .await
                .map_err(|_| Error::Control(ControlError::FailedToSendRecvChannel))?;
        }
        Ok(())
    }

    async fn get_path_by_prefix(&self, prefix: IpNet, family: AddressFamily) -> Result<(), Error> {
        if let Some(all_paths) = self.loc_rib.get(&family, &prefix) {
            let paths = all_paths
                .iter()
                .map(sartd_proto::sart::Path::from)
                .collect();
            self.api_tx
                .send(ApiResponse::Paths(paths))
                .await
                .map_err(|_| Error::Control(ControlError::FailedToSendRecvChannel))?;
        }
        Ok(())
    }
}

fn get_comparison_funcs(multi_path: bool) -> Vec<(BestPathReason, ComparisonFunc)> {
    let mut funcs: Vec<(BestPathReason, ComparisonFunc)> = vec![
        (BestPathReason::Weight, compare_weight),
        (BestPathReason::LocalPref, compare_local_pref),
        (BestPathReason::LocalOriginated, compare_local_originated),
        (BestPathReason::ASPath, compare_as_path),
        (BestPathReason::Origin, compare_origin),
        (BestPathReason::MultiExitDisc, compare_med),
        (BestPathReason::External, compare_external),
    ];
    if multi_path {
        return funcs;
    }
    funcs.push((BestPathReason::OlderRoute, compare_older_route));
    funcs.push((BestPathReason::RouterId, compare_peer_router_id));
    funcs.push((BestPathReason::PeerAddr, compare_peer_addr));
    funcs
}

// ComparisonFunc(a, b): a will be a new path, b will be an existing path
// when ComparisonFunc(a, b) returns Ordering::Less, a is preferred to b
type ComparisonFunc = fn(&Path, &Path) -> Ordering;

fn compare_weight(_a: &Path, _b: &Path) -> Ordering {
    Ordering::Equal
}

fn compare_local_pref(a: &Path, b: &Path) -> Ordering {
    b.local_pref.cmp(&a.local_pref)
}

fn compare_local_originated(a: &Path, b: &Path) -> Ordering {
    let a_l = a.is_local_originated();
    let b_l = b.is_local_originated();
    if a_l == b_l {
        return Ordering::Equal;
    }
    if a_l {
        return Ordering::Less;
    }
    Ordering::Greater
}

fn compare_as_path(a: &Path, b: &Path) -> Ordering {
    let a_len = if a.as_sequence.is_empty() {
        0
    } else {
        a.as_sequence.len()
    };
    let b_len = if b.as_sequence.is_empty() {
        0
    } else {
        b.as_sequence.len()
    };
    a_len.cmp(&b_len)
}

fn compare_origin(a: &Path, b: &Path) -> Ordering {
    a.origin.cmp(&b.origin)
}

fn compare_med(a: &Path, b: &Path) -> Ordering {
    a.med.cmp(&b.med)
}

fn compare_external(a: &Path, b: &Path) -> Ordering {
    let a_ibgp = a.local_asn == a.peer_asn;
    let b_ibgp = b.local_asn == b.peer_asn;
    if a_ibgp == b_ibgp {
        return Ordering::Equal;
    }
    if !a_ibgp {
        return Ordering::Less;
    }
    Ordering::Greater
}

fn compare_older_route(a: &Path, b: &Path) -> Ordering {
    let a_ebgp = a.local_asn != a.peer_asn;
    let b_ebgp = b.local_asn != b.peer_asn;
    if a_ebgp == b_ebgp {
        return a.timestamp.cmp(&b.timestamp);
    }
    Ordering::Equal
}

fn compare_peer_router_id(a: &Path, b: &Path) -> Ordering {
    a.peer_id.cmp(&b.peer_id)
}

fn compare_peer_addr(a: &Path, b: &Path) -> Ordering {
    a.peer_addr.cmp(&b.peer_addr)
}

#[cfg(test)]
mod tests {
    use ipnet::{IpNet, Ipv4Net};
    use rstest::rstest;
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::SystemTime;
    use tokio::sync::mpsc::channel;
    use tokio::time::{timeout, Duration};

    use crate::packet::attribute::{ASSegment, Base};
    use crate::packet::message::Message;
    use crate::packet::prefix::Prefix;
    use crate::path::BestPathReason;
    use crate::server::Bgp;
    use crate::{
        event::RibEvent,
        family::{AddressFamily, Afi},
        packet::attribute::Attribute,
        path::{Path, PathBuilder, PathKind},
        peer::neighbor::NeighborPair,
    };

    use super::{LocRib, LocRibStatus, RibManager, Table};

    #[tokio::test]
    async fn works_rib_manager_add_peer() {
        let (api_tx, _api_rx) = tokio::sync::mpsc::channel(10);
        let mut manager = RibManager::new(
            100,
            Ipv4Addr::new(1, 1, 1, 1),
            None,
            Bgp::ROUTE_TABLE_MAIN,
            vec![Afi::IPv4],
            false,
            api_tx,
        )
        .await
        .unwrap();
        let (tx, _rx) = channel::<RibEvent>(128);
        manager
            .add_peer(
                NeighborPair::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 100),
                tx,
            )
            .unwrap();
    }

    #[rstest(
		msg,
		expected_received,
		case(Message::Update {
            	withdrawn_routes: Vec::new(),
            	attributes: vec![
              		Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_INCOMPLETE),
              		Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![30]}, ASSegment{ segment_type: Attribute::AS_SET, segments: vec![10, 20]}]),
              		Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(10, 0, 0, 9)),
              		Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
              		Attribute::Aggregator(Base::new(Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE, Attribute::AGGREGATOR), 30, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9))),
            	],
            	nlri: vec![Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(172, 16, 0, 0), 21).unwrap()), None)],
        	},
			1,
		),
		case(
			Message::Update {
            	withdrawn_routes: Vec::new(),
            	attributes: vec![
            	    Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
            	    Attribute::AS4Path(Base::new(Attribute::FLAG_TRANSITIVE + Attribute::FLAG_OPTIONAL, Attribute::AS4_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![655361, 2621441]}]),
            	    Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![23456, 23456]}]),
            	    Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(172, 16, 3, 1)),
            	],
            	nlri: vec![
            	    Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(40, 0, 0, 0), 8).unwrap()), None),
            	],
        	},
			1,
		),
		case(
			Message::Update {
            	withdrawn_routes: Vec::new(),
            	attributes: vec![
            	    Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
            	    Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![65001]}]),
            	    Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
            	    Attribute::MPReachNLRI(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MP_REACH_NLRI), AddressFamily::ipv6_unicast(), vec![IpAddr::V6("2001:db8::1".parse().unwrap()), IpAddr::V6("fe80::c001:bff:fe7e:0".parse().unwrap())], vec![Prefix::new("2001:db8:1:2::/64".parse().unwrap(), None), Prefix::new("2001:db8:1:1::/64".parse().unwrap(), None), Prefix::new("2001:db8:1::/64".parse().unwrap(), None)]),
            	],
            	nlri: Vec::new(),
        	},
			3,
		),
		case(
			Message::Update {
            	withdrawn_routes: Vec::new(),
            	attributes: vec![
            	    Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
            	    Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![65100]}]),
            	    Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(1, 1, 1, 1)),
            	    Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
            	],
            	nlri: vec![
            	    Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 3, 0), 24).unwrap()), None),
            	    Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 2, 0), 24).unwrap()), None),
            	    Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 1, 0), 24).unwrap()), None),
            	],
        	},
			3,
		),
	)]
    fn works_table_insert_and_remove(msg: Message, expected_received: usize) {
        let (_withdrawn_routes, attributes, nlri) = msg.to_update().unwrap();
        let mut builder = PathBuilder::builder(
            Ipv4Addr::new(1, 1, 1, 1),
            100,
            Ipv4Addr::new(2, 2, 2, 2),
            IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
            200,
        );
        for attr in attributes.into_iter() {
            builder.attr(attr).unwrap();
        }
        let paths: Vec<Path> = builder.nlri(nlri).build().unwrap();
        let mut table = Table::new();
        for path in paths.into_iter() {
            table.insert(path.prefix(), path);
        }
        assert_eq!(expected_received, table.received);
    }

    #[rstest(
		family,
		prefix,
		paths,
		best_path_ids,
		case(
			AddressFamily::ipv4_unicast(),
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:0,
					best: true,
					reason: BestPathReason::OnlyPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			vec![0],
		),
		case(
			AddressFamily::ipv4_unicast(),
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:1,
					best: true,
					reason: BestPathReason::OnlyPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:0,
					best: false,
					reason: BestPathReason::OnlyPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			vec![1],
		),
		case(
			AddressFamily::ipv4_unicast(),
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:1,
					best: true,
					reason: BestPathReason::OnlyPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:0,
					best: true,
					reason: BestPathReason::OnlyPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			vec![1, 0],
		),
	)]
    fn works_loc_rib_get_best_path(
        family: AddressFamily,
        prefix: IpNet,
        paths: Vec<Path>,
        best_path_ids: Vec<u64>,
    ) {
        let mut t = HashMap::new();
        t.insert(prefix, paths);
        let mut table = HashMap::new();
        table.insert(family.afi, t);
        let loc_rib = LocRib {
            table,
            multi_path: true,
            received: 1,
            dropped: 0,
        };
        let best = loc_rib.get_best_path(&family, &prefix);
        assert_ne!(None, best);
        let actual = best.unwrap().iter().map(|&p| p.id).collect::<Vec<u64>>();
        assert_eq!(best_path_ids, actual)
    }

    #[rstest(
		family,
		multi_path,
		prefix,
		paths,
		path,
		expect,
		best_path_ids,
		case(
			AddressFamily::ipv4_unicast(),
			false,
			"10.0.0.0/24".parse().unwrap(),
			vec![],
			Path {
				id:5,
				best: false,
				reason: BestPathReason::NotBest,
				timestamp: SystemTime::now(),
				local_id: Ipv4Addr::new(1, 1, 1, 1),
				local_asn: 65000,
				peer_id: Ipv4Addr::new(2, 2, 2, 2),
				peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
				peer_asn: 65010,
				family,
				origin: Attribute::ORIGIN_IGP,
				local_pref: 100,
				med: 0,
				as_sequence: vec![65020],
				as_set: vec![],
				next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
				propagate_attributes: vec![],
				prefix: "10.0.0.0/24".parse().unwrap(),
			},
			LocRibStatus::BestPathChanged("10.0.0.0/24".parse().unwrap(), 5),
			vec![5],
		),
		case(
			AddressFamily::ipv4_unicast(),
			false,
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:1,
					best: true,
					reason: BestPathReason::OnlyPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:0,
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65030, 65020, 65040],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			Path {
				id:2,
				best: false,
				reason: BestPathReason::OnlyPath,
				timestamp: SystemTime::now(),
				local_id: Ipv4Addr::new(1, 1, 1, 1),
				local_asn: 65000,
				peer_id: Ipv4Addr::new(2, 2, 2, 2),
				peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
				peer_asn: 65010,
				family,
				origin: Attribute::ORIGIN_IGP,
				local_pref: 100,
				med: 0,
				as_sequence: vec![65020],
				as_set: vec![],
				next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
				propagate_attributes: vec![],
				prefix: "10.0.0.0/24".parse().unwrap(),
			},
			LocRibStatus::BestPathChanged("10.0.0.0/24".parse().unwrap(), 2),
			vec![2],
		),
		case(
			AddressFamily::ipv4_unicast(),
			false,
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:1,
					best: true,
					reason: BestPathReason::ASPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:0,
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: std::ops::Sub::sub(SystemTime::now(), Duration::from_secs(5)),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(4, 4, 4, 4),
					peer_addr: IpAddr::V4(Ipv4Addr::new(4, 4, 4, 4)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65030, 65020, 65040],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			Path {
				id:2,
				best: false,
				reason: BestPathReason::NotBest,
				timestamp: SystemTime::now(),
				local_id: Ipv4Addr::new(1, 1, 1, 1),
				local_asn: 65000,
				peer_id: Ipv4Addr::new(3, 3, 3, 3),
				peer_addr: IpAddr::V4(Ipv4Addr::new(3, 3, 3, 3)),
				peer_asn: 65020,
				family,
				origin: Attribute::ORIGIN_IGP,
				local_pref: 100,
				med: 0,
				as_sequence: vec![65020, 65040, 65060],
				as_set: vec![],
				next_hops: vec![IpAddr::V4(Ipv4Addr::new(3, 3, 3, 3))],
				propagate_attributes: vec![],
				prefix: "10.0.0.0/24".parse().unwrap(),
			},
			LocRibStatus::NotChanged,
			vec![1],
		),
		case(
			AddressFamily::ipv4_unicast(),
			true,
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:1,
					best: true,
					reason: BestPathReason::OnlyPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			Path {
				id:2,
				best: false,
				reason: BestPathReason::NotBest,
				timestamp: SystemTime::now(),
				local_id: Ipv4Addr::new(1, 1, 1, 1),
				local_asn: 65000,
				peer_id: Ipv4Addr::new(3, 3, 3, 3),
				peer_addr: IpAddr::V4(Ipv4Addr::new(3, 3, 3, 3)),
				peer_asn: 65020,
				family,
				origin: Attribute::ORIGIN_IGP,
				local_pref: 100,
				med: 0,
				as_sequence: vec![65040, 65020],
				as_set: vec![],
				next_hops: vec![IpAddr::V4(Ipv4Addr::new(4, 4, 4, 4))],
				propagate_attributes: vec![],
				prefix: "10.0.0.0/24".parse().unwrap(),
			},
			LocRibStatus::MultiPathAdded,
			vec![1, 2],
		),
		case(
			AddressFamily::ipv4_unicast(),
			true,
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:1,
					best: true,
					reason: BestPathReason::OnlyPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:0,
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020, 65050],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			Path {
				id:2,
				best: false,
				reason: BestPathReason::NotBest,
				timestamp: SystemTime::now(),
				local_id: Ipv4Addr::new(1, 1, 1, 1),
				local_asn: 65000,
				peer_id: Ipv4Addr::new(3, 3, 3, 3),
				peer_addr: IpAddr::V4(Ipv4Addr::new(3, 3, 3, 3)),
				peer_asn: 65020,
				family,
				origin: Attribute::ORIGIN_IGP,
				local_pref: 100,
				med: 0,
				as_sequence: vec![65040, 65020],
				as_set: vec![],
				next_hops: vec![IpAddr::V4(Ipv4Addr::new(4, 4, 4, 4))],
				propagate_attributes: vec![],
				prefix: "10.0.0.0/24".parse().unwrap(),
			},
			LocRibStatus::MultiPathAdded,
			vec![1, 2],
		),
		case(
			AddressFamily::ipv4_unicast(),
			true,
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:1,
					best: true,
					reason: BestPathReason::EqualCostMiltiPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:2,
					best: true,
					reason: BestPathReason::EqualCostMiltiPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(3, 3, 3, 3),
					peer_addr: IpAddr::V4(Ipv4Addr::new(3, 3, 3, 3)),
					peer_asn: 65020,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65040, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(4, 4, 4, 4))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:0,
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020, 65050],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			Path {
				id:5,
				best: false,
				reason: BestPathReason::NotBest,
				timestamp: SystemTime::now(),
				local_id: Ipv4Addr::new(1, 1, 1, 1),
				local_asn: 65000,
				peer_id: Ipv4Addr::new(6, 6, 6, 6),
				peer_addr: IpAddr::V4(Ipv4Addr::new(6, 6, 6, 6)),
				peer_asn: 65060,
				family,
				origin: Attribute::ORIGIN_IGP,
				local_pref: 100,
				med: 0,
				as_sequence: vec![65060, 65020],
				as_set: vec![],
				next_hops: vec![IpAddr::V4(Ipv4Addr::new(4, 4, 4, 4))],
				propagate_attributes: vec![],
				prefix: "10.0.0.0/24".parse().unwrap(),
			},
			LocRibStatus::MultiPathAdded,
			vec![1, 2, 5],
		),
		case(
			AddressFamily::ipv4_unicast(),
			true,
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:1,
					best: true,
					reason: BestPathReason::EqualCostMiltiPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:2,
					best: true,
					reason: BestPathReason::EqualCostMiltiPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(3, 3, 3, 3),
					peer_addr: IpAddr::V4(Ipv4Addr::new(3, 3, 3, 3)),
					peer_asn: 65020,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65040, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(4, 4, 4, 4))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:0,
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020, 65050],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			Path {
				id:6,
				best: false,
				reason: BestPathReason::NotBest,
				timestamp: SystemTime::now(),
				local_id: Ipv4Addr::new(1, 1, 1, 1),
				local_asn: 65000,
				peer_id: Ipv4Addr::new(6, 6, 6, 6),
				peer_addr: IpAddr::V4(Ipv4Addr::new(6, 6, 6, 6)),
				peer_asn: 65060,
				family,
				origin: Attribute::ORIGIN_IGP,
				local_pref: 100,
				med: 0,
				as_sequence: vec![65060],
				as_set: vec![],
				next_hops: vec![IpAddr::V4(Ipv4Addr::new(4, 4, 4, 4))],
				propagate_attributes: vec![],
				prefix: "10.0.0.0/24".parse().unwrap(),
			},
			LocRibStatus::BestPathChanged("10.0.0.0/24".parse().unwrap(), 6),
			vec![6],
		),
	)]
    fn works_loc_rib_insert(
        family: AddressFamily,
        multi_path: bool,
        prefix: IpNet,
        paths: Vec<Path>,
        mut path: Path,
        expect: LocRibStatus,
        best_path_ids: Vec<u64>,
    ) {
        let l = paths.len();
        let mut t = HashMap::new();
        t.insert(prefix, paths);
        let mut table = HashMap::new();
        table.insert(family.afi, t);
        let mut loc_rib = LocRib {
            table,
            multi_path,
            received: l,
            dropped: 0,
        };
        let old_best = loc_rib.get_best_path(&family, &prefix);
        assert_ne!(None, old_best);

        let res = loc_rib.insert(&family, &mut path).unwrap();
        assert_eq!(expect, res);
        assert_eq!(l + 1, loc_rib.received);
        let best = loc_rib.get_best_path(&family, &prefix);
        assert_ne!(None, best);
        let actual = best.unwrap().iter().map(|&p| p.id).collect::<Vec<u64>>();
        assert_eq!(best_path_ids, actual);
    }

    #[rstest(
		family,
		multi_path,
		prefix,
		paths,
		id,
		expect,
		best_path_ids,
		case(
			AddressFamily::ipv4_unicast(),
			false,
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:0,
					best: true,
					reason: BestPathReason::OnlyPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			0,
			LocRibStatus::Withdrawn("10.0.0.0/24".parse().unwrap(), PathKind::External),
			vec![],
		),
		case(
			AddressFamily::ipv4_unicast(),
			false,
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:1,
					best: true,
					reason: BestPathReason::ASPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:0,
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65030, 65020, 65040],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			1,
			LocRibStatus::BestPathChanged("10.0.0.0/24".parse().unwrap(), 0),
			vec![0],
		),
		case(
			AddressFamily::ipv4_unicast(),
			false,
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:1,
					best: true,
					reason: BestPathReason::ASPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:0,
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65030, 65020, 65040],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			0,
			LocRibStatus::NotChanged,
			vec![1],
		),
		case(
			AddressFamily::ipv4_unicast(),
			true,
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:1,
					best: true,
					reason: BestPathReason::EqualCostMiltiPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:2,
					best: true,
					reason: BestPathReason::EqualCostMiltiPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(3, 3, 3, 3),
					peer_addr: IpAddr::V4(Ipv4Addr::new(3, 3, 3, 3)),
					peer_asn: 65020,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65040, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(4, 4, 4, 4))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:0,
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020, 65050],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			0,
			LocRibStatus::NotChanged,
			vec![1, 2],
		),
		case(
			AddressFamily::ipv4_unicast(),
			true,
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:1,
					best: true,
					reason: BestPathReason::EqualCostMiltiPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:2,
					best: true,
					reason: BestPathReason::EqualCostMiltiPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(3, 3, 3, 3),
					peer_addr: IpAddr::V4(Ipv4Addr::new(3, 3, 3, 3)),
					peer_asn: 65020,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65040, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(4, 4, 4, 4))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:0,
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020, 65050],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			2,
			LocRibStatus::MultiPathWithdrawn("10.0.0.0/24".parse().unwrap(), 1, Some(IpAddr::V4(Ipv4Addr::new(4, 4, 4, 4)))),
			vec![1],
		),
		case(
			AddressFamily::ipv4_unicast(),
			true,
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:1,
					best: true,
					reason: BestPathReason::EqualCostMiltiPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:2,
					best: true,
					reason: BestPathReason::EqualCostMiltiPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(3, 3, 3, 3),
					peer_addr: IpAddr::V4(Ipv4Addr::new(3, 3, 3, 3)),
					peer_asn: 65020,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65040, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(4, 4, 4, 4))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:0,
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020, 65050],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			1,
			LocRibStatus::MultiPathWithdrawn("10.0.0.0/24".parse().unwrap(), 2, Some(IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)))),
			vec![2],
		),
		case(
			AddressFamily::ipv4_unicast(),
			true,
			"10.0.0.0/24".parse().unwrap(),
			vec![
				Path {
					id:0,
					best: true,
					reason: BestPathReason::ASPath,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:1,
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:2,
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(3, 3, 3, 3),
					peer_addr: IpAddr::V4(Ipv4Addr::new(3, 3, 3, 3)),
					peer_asn: 65020,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65040, 65020],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(4, 4, 4, 4))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
				Path {
					id:5,
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: SystemTime::now(),
					local_id: Ipv4Addr::new(1, 1, 1, 1),
					local_asn: 65000,
					peer_id: Ipv4Addr::new(2, 2, 2, 2),
					peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
					peer_asn: 65010,
					family,
					origin: Attribute::ORIGIN_IGP,
					local_pref: 100,
					med: 0,
					as_sequence: vec![65010, 65020, 65050],
					as_set: vec![],
					next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
					propagate_attributes: vec![],
					prefix: "10.0.0.0/24".parse().unwrap(),
				},
			],
			0,
			LocRibStatus::BestPathChanged("10.0.0.0/24".parse().unwrap(), 1),
			vec![1, 2],
		),
	)]
    fn works_loc_rib_drop(
        family: AddressFamily,
        multi_path: bool,
        prefix: IpNet,
        paths: Vec<Path>,
        id: u64,
        expect: LocRibStatus,
        best_path_ids: Vec<u64>,
    ) {
        let l = paths.len();
        let mut t = HashMap::new();
        t.insert(prefix, paths);
        let mut table = HashMap::new();
        table.insert(family.afi, t);
        let mut loc_rib = LocRib {
            table,
            multi_path,
            received: l,
            dropped: 0,
        };
        let old_best = loc_rib.get_best_path(&family, &prefix);
        assert_ne!(None, old_best);

        let res = loc_rib.drop(&family, &prefix, id).unwrap();
        assert_eq!(expect, res);
        assert_eq!(1, loc_rib.dropped);

        let best = loc_rib.get_best_path(&family, &prefix);
        assert_ne!(None, best);

        let actual = best.unwrap().iter().map(|&p| p.id).collect::<Vec<u64>>();
        assert_eq!(best_path_ids, actual);
    }

    #[tokio::test]
    async fn rib_manager_install_drop_paths_ebgp_multi_path_disabled() {
        let (api_tx, _api_rx) = tokio::sync::mpsc::channel(10);
        let mut manager = RibManager::new(
            65000,
            Ipv4Addr::new(1, 1, 1, 1),
            None,
            Bgp::ROUTE_TABLE_MAIN,
            vec![AddressFamily::ipv4_unicast().afi],
            false,
            api_tx,
        )
        .await
        .unwrap();

        // peer1
        let peer1 = NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)), 65010);
        let (peer1_tx, mut peer1_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer1.clone(), peer1_tx).unwrap();

        // peer2
        let peer2 = NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)), 65020);
        let (peer2_tx, mut peer2_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer2.clone(), peer2_tx).unwrap();

        // install paths from peer1
        let paths = vec![Path {
            id: 0,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: SystemTime::now(),
            local_id: Ipv4Addr::new(1, 1, 1, 1),
            local_asn: 65000,
            peer_id: Ipv4Addr::new(2, 2, 2, 2),
            peer_addr: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
            peer_asn: 65010,
            family: AddressFamily::ipv4_unicast(),
            origin: Attribute::ORIGIN_IGP,
            local_pref: 100,
            med: 0,
            as_sequence: vec![65010],
            as_set: vec![],
            next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
            propagate_attributes: vec![],
            prefix: "10.0.0.0/24".parse().unwrap(),
        }];
        manager
            .install_paths(peer1.clone(), paths, false)
            .await
            .unwrap();

        // recv event
        match timeout(Duration::from_millis(10), peer2_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
            }
            _ => panic!("expected rib_event is advertise"),
        }
        // not recv
        match timeout(Duration::from_millis(10), peer1_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer1 should not receive event"),
        }
        // install paths from peer2
        let paths = vec![
            Path {
                id: 1,
                best: false,
                reason: BestPathReason::NotBest,
                timestamp: SystemTime::now(),
                local_id: Ipv4Addr::new(1, 1, 1, 1),
                local_asn: 65000,
                peer_id: Ipv4Addr::new(3, 3, 3, 3),
                peer_addr: IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)),
                peer_asn: 65020,
                family: AddressFamily::ipv4_unicast(),
                origin: Attribute::ORIGIN_IGP,
                local_pref: 100,
                med: 0,
                as_sequence: vec![65020],
                as_set: vec![],
                next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
                propagate_attributes: vec![],
                prefix: "10.0.1.0/24".parse().unwrap(),
            },
            Path {
                id: 2,
                best: false,
                reason: BestPathReason::NotBest,
                timestamp: SystemTime::now(),
                local_id: Ipv4Addr::new(1, 1, 1, 1),
                local_asn: 65000,
                peer_id: Ipv4Addr::new(3, 3, 3, 3),
                peer_addr: IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)),
                peer_asn: 65020,
                family: AddressFamily::ipv4_unicast(),
                origin: Attribute::ORIGIN_IGP,
                local_pref: 100,
                med: 0,
                as_sequence: vec![65020],
                as_set: vec![],
                next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
                propagate_attributes: vec![],
                prefix: "10.0.10.0/24".parse().unwrap(),
            },
            Path {
                id: 3,
                best: false,
                reason: BestPathReason::NotBest,
                timestamp: SystemTime::now(),
                local_id: Ipv4Addr::new(1, 1, 1, 1),
                local_asn: 65000,
                peer_id: Ipv4Addr::new(3, 3, 3, 3),
                peer_addr: IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)),
                peer_asn: 65020,
                family: AddressFamily::ipv4_unicast(),
                origin: Attribute::ORIGIN_IGP,
                local_pref: 100,
                med: 0,
                as_sequence: vec![65020, 65010],
                as_set: vec![],
                next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
                propagate_attributes: vec![],
                prefix: "10.0.0.0/24".parse().unwrap(),
            },
        ];
        manager
            .install_paths(peer2.clone(), paths, false)
            .await
            .unwrap();

        // recv event
        match timeout(Duration::from_millis(10), peer1_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(2, paths.len());
                assert_eq!(1, paths[0].id);
                assert_eq!(2, paths[1].id);
            }
            _ => panic!("expected rib_event is advertise"),
        }
        // not recv
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer2 should not receive event"),
        }

        // install paths from local
        let paths = vec![Path {
            id: 4,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: SystemTime::now(),
            local_id: Ipv4Addr::new(1, 1, 1, 1),
            local_asn: 65000,
            peer_id: Ipv4Addr::new(1, 1, 1, 1),
            peer_addr: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            peer_asn: 65000,
            family: AddressFamily::ipv4_unicast(),
            origin: Attribute::ORIGIN_IGP,
            local_pref: 100,
            med: 0,
            as_sequence: vec![],
            as_set: vec![],
            next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
            propagate_attributes: vec![],
            prefix: "10.0.100.0/24".parse().unwrap(),
        }];
        manager
            .install_paths(
                NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 65000),
                paths,
                false,
            )
            .await
            .unwrap();

        // recv event
        match timeout(Duration::from_millis(10), peer2_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
                assert_eq!(4, paths[0].id);
            }
            _ => panic!("expected rib_event is advertise"),
        }
        match timeout(Duration::from_millis(10), peer1_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
                assert_eq!(4, paths[0].id);
            }
            _ => panic!("expected rib_event is advertise"),
        }

        // drop paths
        let family = AddressFamily::ipv4_unicast();
        // drop from peer2
        let prefixes: Vec<(IpNet, u64)> = vec![("10.0.0.0/24".parse().unwrap(), 3)];
        manager
            .drop_paths(peer2.clone(), family, prefixes, false)
            .await
            .unwrap();

        // not recv
        match timeout(Duration::from_millis(10), peer1_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer1 should not receive event"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer2 should not receive event"),
        }
        // drop from peer1
        let prefixes: Vec<(IpNet, u64)> = vec![("10.0.0.0/24".parse().unwrap(), 0)];
        manager
            .drop_paths(peer1.clone(), family, prefixes, false)
            .await
            .unwrap();

        // recv
        match timeout(Duration::from_millis(10), peer2_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Withdraw(prefix) => {
                assert_eq!(1, prefix.len());
            }
            _ => panic!("expected rib_event is withdrawn"),
        }
        // not recv
        match timeout(Duration::from_millis(10), peer1_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer1 should not receive event"),
        }
        // drop from local
        let prefixes: Vec<(IpNet, u64)> = vec![("10.0.100.0/24".parse().unwrap(), 4)];
        manager
            .drop_paths(
                NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 65000),
                family,
                prefixes,
                false,
            )
            .await
            .unwrap();

        // recv
        match timeout(Duration::from_millis(10), peer1_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Withdraw(prefix) => {
                assert_eq!(1, prefix.len());
            }
            _ => panic!("expected rib_event is withdrawn"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Withdraw(prefix) => {
                assert_eq!(1, prefix.len());
            }
            _ => panic!("expected rib_event is withdrawn"),
        }
    }
    #[tokio::test]
    async fn rib_manager_install_drop_paths_ibgp_multi_path_disabled() {
        let (api_tx, _api_rx) = tokio::sync::mpsc::channel(10);
        let mut manager = RibManager::new(
            65000,
            Ipv4Addr::new(1, 1, 1, 1),
            None,
            Bgp::ROUTE_TABLE_MAIN,
            vec![AddressFamily::ipv4_unicast().afi],
            false,
            api_tx,
        )
        .await
        .unwrap();

        // peer1
        let peer1 = NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)), 65000);
        let (peer1_tx, mut peer1_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer1.clone(), peer1_tx).unwrap();

        // peer2
        let peer2 = NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)), 65000);
        let (peer2_tx, mut peer2_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer2.clone(), peer2_tx).unwrap();

        // install paths from peer1
        let paths = vec![Path {
            id: 0,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: SystemTime::now(),
            local_id: Ipv4Addr::new(1, 1, 1, 1),
            local_asn: 65000,
            peer_id: Ipv4Addr::new(2, 2, 2, 2),
            peer_addr: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
            peer_asn: 65000,
            family: AddressFamily::ipv4_unicast(),
            origin: Attribute::ORIGIN_IGP,
            local_pref: 100,
            med: 0,
            as_sequence: vec![65010],
            as_set: vec![],
            next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
            propagate_attributes: vec![],
            prefix: "10.0.0.0/24".parse().unwrap(),
        }];
        manager
            .install_paths(peer1.clone(), paths, false)
            .await
            .unwrap();

        // not recv
        match timeout(Duration::from_millis(10), peer1_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer1 should not receive event"),
        }
        // not recv
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer2 should not receive event"),
        }

        // install paths from local
        let paths = vec![Path {
            id: 4,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: SystemTime::now(),
            local_id: Ipv4Addr::new(1, 1, 1, 1),
            local_asn: 65000,
            peer_id: Ipv4Addr::new(1, 1, 1, 1),
            peer_addr: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            peer_asn: 65000,
            family: AddressFamily::ipv4_unicast(),
            origin: Attribute::ORIGIN_IGP,
            local_pref: 100,
            med: 0,
            as_sequence: vec![],
            as_set: vec![],
            next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
            propagate_attributes: vec![],
            prefix: "10.0.100.0/24".parse().unwrap(),
        }];
        manager
            .install_paths(
                NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 65000),
                paths,
                false,
            )
            .await
            .unwrap();

        // recv event
        match timeout(Duration::from_millis(10), peer2_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
                assert_eq!(4, paths[0].id);
            }
            _ => panic!("expected rib_event is advertise"),
        }
        match timeout(Duration::from_millis(10), peer1_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
                assert_eq!(4, paths[0].id);
            }
            _ => panic!("expected rib_event is advertise"),
        }

        // drop paths
        let family = AddressFamily::ipv4_unicast();
        // drop from peer1
        let prefixes: Vec<(IpNet, u64)> = vec![("10.0.0.0/24".parse().unwrap(), 0)];
        manager
            .drop_paths(peer1.clone(), family, prefixes, false)
            .await
            .unwrap();

        // not recv
        match timeout(Duration::from_millis(10), peer1_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer1 should not receive event"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer2 should not receive event"),
        }
        // drop from local
        let prefixes: Vec<(IpNet, u64)> = vec![("10.0.100.0/24".parse().unwrap(), 4)];
        manager
            .drop_paths(
                NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 65000),
                family,
                prefixes,
                false,
            )
            .await
            .unwrap();

        // recv
        match timeout(Duration::from_millis(10), peer1_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Withdraw(prefix) => {
                assert_eq!(1, prefix.len());
            }
            _ => panic!("expected rib_event is withdrawn"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Withdraw(prefix) => {
                assert_eq!(1, prefix.len());
            }
            _ => panic!("expected rib_event is withdrawn"),
        }
    }
    #[tokio::test]
    async fn rib_manager_install_drop_paths_ebgp_multi_path_enabled() {
        let (api_tx, _api_rx) = tokio::sync::mpsc::channel(10);
        let mut manager = RibManager::new(
            65000,
            Ipv4Addr::new(1, 1, 1, 1),
            None,
            Bgp::ROUTE_TABLE_MAIN,
            vec![AddressFamily::ipv4_unicast().afi],
            true,
            api_tx,
        )
        .await
        .unwrap();

        // peer1
        let peer1 = NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)), 65010);
        let (peer1_tx, mut peer1_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer1.clone(), peer1_tx).unwrap();

        // peer2
        let peer2 = NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)), 65020);
        let (peer2_tx, mut peer2_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer2.clone(), peer2_tx).unwrap();

        // install paths from peer1
        let paths = vec![Path {
            id: 0,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: SystemTime::now(),
            local_id: Ipv4Addr::new(1, 1, 1, 1),
            local_asn: 65000,
            peer_id: Ipv4Addr::new(2, 2, 2, 2),
            peer_addr: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
            peer_asn: 65010,
            family: AddressFamily::ipv4_unicast(),
            origin: Attribute::ORIGIN_IGP,
            local_pref: 100,
            med: 0,
            as_sequence: vec![65010],
            as_set: vec![],
            next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
            propagate_attributes: vec![],
            prefix: "10.0.20.0/24".parse().unwrap(),
        }];
        manager
            .install_paths(peer1.clone(), paths, false)
            .await
            .unwrap();

        // recv event
        match timeout(Duration::from_millis(10), peer2_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
            }
            _ => panic!("expected rib_event is advertise"),
        }
        // not recv
        match timeout(Duration::from_millis(10), peer1_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer1 should not receive event"),
        }
        // install paths from peer2
        let paths = vec![
            Path {
                id: 1,
                best: false,
                reason: BestPathReason::NotBest,
                timestamp: SystemTime::now(),
                local_id: Ipv4Addr::new(1, 1, 1, 1),
                local_asn: 65000,
                peer_id: Ipv4Addr::new(3, 3, 3, 3),
                peer_addr: IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)),
                peer_asn: 65020,
                family: AddressFamily::ipv4_unicast(),
                origin: Attribute::ORIGIN_IGP,
                local_pref: 100,
                med: 0,
                as_sequence: vec![65020],
                as_set: vec![],
                next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
                propagate_attributes: vec![],
                prefix: "10.0.20.0/24".parse().unwrap(),
            },
            Path {
                id: 2,
                best: false,
                reason: BestPathReason::NotBest,
                timestamp: SystemTime::now(),
                local_id: Ipv4Addr::new(1, 1, 1, 1),
                local_asn: 65000,
                peer_id: Ipv4Addr::new(3, 3, 3, 3),
                peer_addr: IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)),
                peer_asn: 65020,
                family: AddressFamily::ipv4_unicast(),
                origin: Attribute::ORIGIN_IGP,
                local_pref: 100,
                med: 0,
                as_sequence: vec![65020],
                as_set: vec![],
                next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
                propagate_attributes: vec![],
                prefix: "10.0.10.0/24".parse().unwrap(),
            },
        ];
        manager
            .install_paths(peer2.clone(), paths, false)
            .await
            .unwrap();

        // recv event
        match timeout(Duration::from_millis(10), peer1_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
                assert_eq!(2, paths[0].id);
            }
            _ => panic!("expected rib_event is advertise"),
        }
        // not recv
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer2 should not receive event"),
        }

        // install paths from local
        let paths = vec![Path {
            id: 4,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: SystemTime::now(),
            local_id: Ipv4Addr::new(1, 1, 1, 1),
            local_asn: 65000,
            peer_id: Ipv4Addr::new(1, 1, 1, 1),
            peer_addr: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            peer_asn: 65000,
            family: AddressFamily::ipv4_unicast(),
            origin: Attribute::ORIGIN_IGP,
            local_pref: 100,
            med: 0,
            as_sequence: vec![],
            as_set: vec![],
            next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
            propagate_attributes: vec![],
            prefix: "10.0.20.0/24".parse().unwrap(),
        }];
        manager
            .install_paths(
                NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 65000),
                paths,
                false,
            )
            .await
            .unwrap();

        // recv event
        match timeout(Duration::from_millis(10), peer2_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
                assert_eq!(4, paths[0].id);
            }
            _ => panic!("expected rib_event is advertise"),
        }
        match timeout(Duration::from_millis(10), peer1_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
                assert_eq!(4, paths[0].id);
            }
            _ => panic!("expected rib_event is advertise"),
        }
        // drop paths
        let family = AddressFamily::ipv4_unicast();
        // drop from peer1
        let prefixes: Vec<(IpNet, u64)> = vec![
            ("10.0.20.0/24".parse().unwrap(), 1),
            ("10.0.10.0/24".parse().unwrap(), 2),
        ];
        manager
            .drop_paths(peer2.clone(), family, prefixes, false)
            .await
            .unwrap();

        // recv
        match timeout(Duration::from_millis(10), peer1_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Withdraw(prefix) => {
                assert_eq!(1, prefix.len());
            }
            _ => panic!("expected rib_event is withdrawn"),
        }
        // not recv
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer2 should not receive event"),
        }
        // drop from local
        let prefixes: Vec<(IpNet, u64)> = vec![("10.0.20.0/24".parse().unwrap(), 4)];
        manager
            .drop_paths(
                NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 65000),
                family,
                prefixes,
                false,
            )
            .await
            .unwrap();

        // recv
        match timeout(Duration::from_millis(10), peer1_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
                assert_eq!(0, paths[0].id);
            }
            _ => panic!("expected rib_event is advertise"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
                assert_eq!(0, paths[0].id);
            }
            _ => panic!("expected rib_event is advertise"),
        }
    }
    #[tokio::test]
    async fn rib_manager_install_drop_paths_ebgp_ibgp_multi_path_enabled() {
        let (api_tx, _api_rx) = tokio::sync::mpsc::channel(10);
        let mut manager = RibManager::new(
            65000,
            Ipv4Addr::new(1, 1, 1, 1),
            None,
            Bgp::ROUTE_TABLE_MAIN,
            vec![AddressFamily::ipv4_unicast().afi],
            true,
            api_tx,
        )
        .await
        .unwrap();

        // peer1
        let peer1 = NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)), 65000);
        let (peer1_tx, mut peer1_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer1.clone(), peer1_tx).unwrap();

        // peer2
        let peer2 = NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)), 65000);
        let (peer2_tx, mut peer2_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer2.clone(), peer2_tx).unwrap();

        // peer3
        let peer3 = NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 2, 2)), 65010);
        let (peer3_tx, mut peer3_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer3.clone(), peer3_tx).unwrap();

        // install paths from peer1
        let paths = vec![Path {
            id: 0,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: SystemTime::now(),
            local_id: Ipv4Addr::new(1, 1, 1, 1),
            local_asn: 65000,
            peer_id: Ipv4Addr::new(2, 2, 2, 2),
            peer_addr: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
            peer_asn: 65000,
            family: AddressFamily::ipv4_unicast(),
            origin: Attribute::ORIGIN_IGP,
            local_pref: 100,
            med: 0,
            as_sequence: vec![],
            as_set: vec![],
            next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
            propagate_attributes: vec![],
            prefix: "10.0.20.0/24".parse().unwrap(),
        }];
        manager
            .install_paths(peer1.clone(), paths, false)
            .await
            .unwrap();

        // recv event
        match timeout(Duration::from_millis(10), peer3_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
            }
            _ => panic!("expected rib_event is advertise"),
        }
        // not recv
        match timeout(Duration::from_millis(10), peer1_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer1 should not receive event"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer2 should not receive event"),
        }
        // install paths from peer2
        let paths = vec![
            Path {
                id: 1,
                best: false,
                reason: BestPathReason::NotBest,
                timestamp: SystemTime::now(),
                local_id: Ipv4Addr::new(1, 1, 1, 1),
                local_asn: 65000,
                peer_id: Ipv4Addr::new(3, 3, 3, 3),
                peer_addr: IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)),
                peer_asn: 65000,
                family: AddressFamily::ipv4_unicast(),
                origin: Attribute::ORIGIN_IGP,
                local_pref: 100,
                med: 0,
                as_sequence: vec![],
                as_set: vec![],
                next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
                propagate_attributes: vec![],
                prefix: "10.0.20.0/24".parse().unwrap(),
            },
            Path {
                id: 2,
                best: false,
                reason: BestPathReason::NotBest,
                timestamp: SystemTime::now(),
                local_id: Ipv4Addr::new(1, 1, 1, 1),
                local_asn: 65000,
                peer_id: Ipv4Addr::new(3, 3, 3, 3),
                peer_addr: IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)),
                peer_asn: 65000,
                family: AddressFamily::ipv4_unicast(),
                origin: Attribute::ORIGIN_IGP,
                local_pref: 100,
                med: 0,
                as_sequence: vec![65100],
                as_set: vec![],
                next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
                propagate_attributes: vec![],
                prefix: "10.0.10.0/24".parse().unwrap(),
            },
        ];
        manager
            .install_paths(peer2.clone(), paths, false)
            .await
            .unwrap();

        // recv event
        match timeout(Duration::from_millis(10), peer3_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
            }
            _ => panic!("expected rib_event is advertise"),
        }
        // not recv
        match timeout(Duration::from_millis(10), peer1_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer1 should not receive event"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer2 should not receive event"),
        }

        // install paths from peer3(ebgp)
        let paths = vec![Path {
            id: 4,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: SystemTime::now(),
            local_id: Ipv4Addr::new(1, 1, 1, 1),
            local_asn: 65000,
            peer_id: Ipv4Addr::new(4, 4, 4, 4),
            peer_addr: IpAddr::V4(Ipv4Addr::new(10, 0, 2, 2)),
            peer_asn: 65010,
            family: AddressFamily::ipv4_unicast(),
            origin: Attribute::ORIGIN_IGP,
            local_pref: 100,
            med: 0,
            as_sequence: vec![],
            as_set: vec![],
            next_hops: vec![IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))],
            propagate_attributes: vec![],
            prefix: "10.0.30.0/24".parse().unwrap(),
        }];
        manager
            .install_paths(peer3.clone(), paths, false)
            .await
            .unwrap();

        // recv event
        match timeout(Duration::from_millis(10), peer2_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
                assert_eq!(4, paths[0].id);
            }
            _ => panic!("expected rib_event is advertise"),
        }
        match timeout(Duration::from_millis(10), peer1_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
                assert_eq!(4, paths[0].id);
            }
            _ => panic!("expected rib_event is advertise"),
        }
        // not recv
        match timeout(Duration::from_millis(10), peer3_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer3 should not receive event"),
        }
        // drop paths
        let family = AddressFamily::ipv4_unicast();
        // drop from peer1
        let prefixes: Vec<(IpNet, u64)> = vec![
            ("10.0.20.0/24".parse().unwrap(), 1),
            ("10.0.10.0/24".parse().unwrap(), 2),
        ];
        manager
            .drop_paths(peer2.clone(), family, prefixes, false)
            .await
            .unwrap();

        // recv
        for _ in 0..2 {
            // best path changed: 10.0.20.0/24
            // withdrawn: 10.0.10.0/24
            match timeout(Duration::from_millis(10), peer3_rx.recv())
                .await
                .unwrap()
                .unwrap()
            {
                RibEvent::Withdraw(prefix) => {
                    assert_eq!(1, prefix.len());
                }
                RibEvent::Advertise(path) => {
                    assert_eq!(1, path.len());
                    assert_eq!(0, path[0].id);
                }
                _ => panic!("expected rib_event is withdrawn"),
            }
        }
        // not recv
        match timeout(Duration::from_millis(10), peer1_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer1 should not receive event"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer2 should not receive event"),
        }
        // drop from peer1
        let prefixes: Vec<(IpNet, u64)> = vec![("10.0.20.0/24".parse().unwrap(), 0)];
        manager
            .drop_paths(peer1.clone(), family, prefixes, false)
            .await
            .unwrap();

        // recv
        match timeout(Duration::from_millis(10), peer3_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Withdraw(prefix) => {
                assert_eq!(1, prefix.len());
            }
            _ => panic!("expected rib_event is withdraw"),
        }
        // not recv
        match timeout(Duration::from_millis(10), peer1_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer1 should not receive event"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer2 should not receive event"),
        }
        // drop from peer3
        let prefixes: Vec<(IpNet, u64)> = vec![("10.0.30.0/24".parse().unwrap(), 4)];
        manager
            .drop_paths(peer3.clone(), family, prefixes, false)
            .await
            .unwrap();

        // recv
        match timeout(Duration::from_millis(10), peer1_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Withdraw(prefix) => {
                assert_eq!(1, prefix.len());
            }
            _ => panic!("expected rib_event is withdraw"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Withdraw(prefix) => {
                assert_eq!(1, prefix.len());
            }
            _ => panic!("expected rib_event is withdraw"),
        }
        // not recv
        match timeout(Duration::from_millis(10), peer3_rx.recv()).await {
            Err(_) => {}
            Ok(_) => panic!("peer3 should not receive event"),
        }
    }

    #[tokio::test]
    async fn rib_manager_add_delete_network() {
        let (api_tx, _api_rx) = tokio::sync::mpsc::channel(10);
        let mut manager = RibManager::new(
            65000,
            Ipv4Addr::new(1, 1, 1, 1),
            None,
            Bgp::ROUTE_TABLE_MAIN,
            vec![AddressFamily::ipv4_unicast().afi],
            true,
            api_tx,
        )
        .await
        .unwrap();

        // peer1
        let peer1 = NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)), 65000);
        let (peer1_tx, mut peer1_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer1.clone(), peer1_tx).unwrap();

        // peer2
        let peer2 = NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)), 65000);
        let (peer2_tx, mut peer2_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer2.clone(), peer2_tx).unwrap();

        // peer3
        let peer3 = NeighborPair::new(IpAddr::V4(Ipv4Addr::new(10, 0, 2, 2)), 65010);
        let (peer3_tx, mut peer3_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer3.clone(), peer3_tx).unwrap();

        manager
            .add_path(
                vec![
                    "10.0.0.0/24".parse().unwrap(),
                    "10.1.0.0/24".parse().unwrap(),
                ],
                vec![],
            )
            .await
            .unwrap();

        // recv
        match timeout(Duration::from_millis(10), peer1_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(2, paths.len());
            }
            _ => panic!("expected rib_event is advertise"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(2, paths.len());
            }
            _ => panic!("expected rib_event is advertise"),
        }
        match timeout(Duration::from_millis(10), peer3_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(2, paths.len());
            }
            _ => panic!("expected rib_event is advertise"),
        }

        manager
            .add_path(
                vec!["10.0.2.0/24".parse().unwrap()],
                vec![
                    Attribute::new_origin(Attribute::ORIGIN_INCOMPLETE).unwrap(),
                    Attribute::new_local_pref(200).unwrap(),
                ],
            )
            .await
            .unwrap();
        // recv
        match timeout(Duration::from_millis(10), peer1_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
                assert_eq!(Attribute::ORIGIN_INCOMPLETE, paths[0].origin);
                assert_eq!(200, paths[0].local_pref);
            }
            _ => panic!("expected rib_event is advertise"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
                assert_eq!(Attribute::ORIGIN_INCOMPLETE, paths[0].origin);
                assert_eq!(200, paths[0].local_pref);
            }
            _ => panic!("expected rib_event is advertise"),
        }
        match timeout(Duration::from_millis(10), peer3_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Advertise(paths) => {
                assert_eq!(1, paths.len());
                assert_eq!(Attribute::ORIGIN_INCOMPLETE, paths[0].origin);
                assert_eq!(200, paths[0].local_pref);
            }
            _ => panic!("expected rib_event is advertise"),
        }

        manager
            .delete_path(
                AddressFamily::ipv4_unicast(),
                vec!["10.0.2.0/24".parse().unwrap()],
            )
            .await
            .unwrap();
        //recv
        match timeout(Duration::from_millis(10), peer1_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Withdraw(prefix) => {
                assert_eq!(1, prefix.len());
            }
            _ => panic!("expected rib_event is withdraw"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Withdraw(prefix) => {
                assert_eq!(1, prefix.len());
            }
            _ => panic!("expected rib_event is withdraw"),
        }
        match timeout(Duration::from_millis(10), peer3_rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            RibEvent::Withdraw(prefix) => {
                assert_eq!(1, prefix.len());
            }
            _ => panic!("expected rib_event is withdraw"),
        }
    }
}
