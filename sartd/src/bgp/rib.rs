use ipnet::IpNet;
use std::{cmp::Ordering, collections::HashMap, net::Ipv4Addr};
use tokio::sync::mpsc::Sender;

use crate::bgp::path::PathKind;

use super::{
    error::{ControlError, Error, RibError},
    event::RibEvent,
    family::{AddressFamily, Afi},
    path::{BestPathReason, Path},
    peer::neighbor::NeighborPair,
};

#[derive(Debug)]
pub(crate) struct Table {
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
}

#[derive(Debug)]
pub(crate) struct AdjRib {
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

    pub fn prefixes(
        &self,
        family: &AddressFamily,
    ) -> Option<std::collections::hash_map::Keys<'_, IpNet, Path>> {
        match self.table.get(family) {
            Some(table) => Some(table.inner.keys()),
            None => None,
        }
    }

    pub fn remove(&mut self, family: &AddressFamily, prefix: &IpNet) -> Option<Path> {
        match self.table.get_mut(family) {
            Some(table) => table.remove(prefix),
            None => None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct LocRib {
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

    fn set_protocol(&mut self, protocol: Afi) -> Result<(), Error> {
        match self.table.get(&protocol) {
            Some(_) => Err(Error::Rib(RibError::ProtocolIsAlreadyRegistered)),
            None => {
                self.table.insert(protocol, HashMap::new());
                Ok(())
            }
        }
    }

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
        match self.get(family, prefix) {
            Some(paths) => Some(paths.iter().filter(|p| p.best).collect::<Vec<&Path>>()),
            None => None,
        }
    }

    #[tracing::instrument(skip(self, family, path))]
    fn insert(&mut self, family: &AddressFamily, path: &mut Path) -> Result<LocRibStatus, Error> {
        let prefix = path.prefix.clone();
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
                    tracing::info!(reason=?reason,prefix=?prefix,path_id=path.id,"Best path is decided");
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
            } else {
                if idx == 0 {
                    path.best = true;
                    path.reason = reason;

                    // unmark old best path
                    paths[0].best = false;
                    paths[0].reason = BestPathReason::NotBest;

                    paths.insert(idx, path.clone());
                    return Ok(LocRibStatus::BestPathChanged(prefix, path.id));
                }
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

    fn drop(
        &mut self,
        family: &AddressFamily,
        prefix: &IpNet,
        id: u64,
    ) -> Result<LocRibStatus, Error> {
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
                        if idx == 0 {
                            // when idx is best, we don't have to update best paths.
                            if paths[idx].best {
                                // TODO: the best path selected reason may be changed
                                return Ok(LocRibStatus::MultiPathWithdrawn(*prefix, paths[0].id));
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
                        return Ok(LocRibStatus::MultiPathWithdrawn(*prefix, paths[0].id));
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
    MultiPathWithdrawn(IpNet, u64),
    AddtionalPathAdded(IpNet, u64),
    AdditionalPathWithdrawn(IpNet, u64),
}

#[derive(Debug)]
pub(crate) struct RibManager {
    asn: u32,
    router_id: Ipv4Addr,
    loc_rib: LocRib,
    endpoint: String,
    peers_tx: HashMap<NeighborPair, Sender<RibEvent>>,
}

impl RibManager {
    pub fn new(
        asn: u32,
        router_id: Ipv4Addr,
        endpoint: String,
        protocols: Vec<Afi>,
        multi_path_enabled: bool,
    ) -> Self {
        Self {
            asn,
            router_id,
            loc_rib: LocRib::new(protocols, multi_path_enabled),
            endpoint,
            peers_tx: HashMap::new(),
        }
    }

    #[tracing::instrument(skip(self, event))]
    pub async fn handle(&mut self, event: RibEvent) -> Result<(), Error> {
        tracing::info!(level="rib",event=?event);
        match event {
            RibEvent::AddPeer {
                neighbor,
                rib_event_tx,
            } => self.add_peer(neighbor, rib_event_tx),
            RibEvent::AddNetwork(networks) => self.add_network(networks),
            RibEvent::InstallPaths(neighbor, paths) => self.install_paths(neighbor, paths).await,
            RibEvent::DropPaths(neighbor, family, path_ids) => {
                self.drop_paths(neighbor, family, path_ids).await
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
    fn add_network(&mut self, networks: Vec<IpNet>) -> Result<(), Error> {
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn install_paths(
        &mut self,
        neighbor: NeighborPair,
        mut paths: Vec<Path>,
    ) -> Result<(), Error> {
        if paths.is_empty() {
            return Ok(());
        }
        let family = paths[0].family;

        let mut changed_prefixes = Vec::new();
        for path in paths.iter_mut() {
            // TODO: should send event to fib endpoint
            match self.loc_rib.insert(&family, path)? {
                LocRibStatus::BestPathChanged(prefix, id) => {
                    tracing::info!(family=?family,prefix=?prefix,path_id=id,"best path changed");
                    changed_prefixes.push((prefix, id));
                }
                LocRibStatus::MultiPathAdded => {
                    tracing::info!(family=?family,prefix=?path.prefix,"multiple best paths added");
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

    #[tracing::instrument(skip(self))]
    async fn drop_paths(
        &mut self,
        neighbor: NeighborPair,
        family: AddressFamily,
        path_ids: Vec<(IpNet, u64)>,
    ) -> Result<(), Error> {
        let mut advertise_prefixes = Vec::new();
        let mut withdraw_prefixes = Vec::new();
        for (prefix, path_id) in path_ids.iter() {
            match self.loc_rib.drop(&family, prefix, *path_id)? {
                LocRibStatus::BestPathChanged(prefix, id) => {
                    tracing::info!(family=?family,prefix=?prefix,path_id=id,"best path changed");
                    advertise_prefixes.push((prefix, id));
                }
                LocRibStatus::MultiPathWithdrawn(prefix, id) => {
                    tracing::info!(family=?family,prefix=?prefix,"multiple best paths withdrawn");
                }
                LocRibStatus::Withdrawn(prefix, kind) => {
                    withdraw_prefixes.push((prefix, kind));
                    tracing::info!(family=?family,prefix=?prefix,"all path are withdrawn");
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
}

fn get_comparison_funcs(multi_path: bool) -> Vec<(BestPathReason, ComparisonFunc)> {
    let mut funcs: Vec<(BestPathReason, ComparisonFunc)> = Vec::new();
    funcs.push((BestPathReason::Weight, compare_weight));
    funcs.push((BestPathReason::LocalPref, compare_local_pref));
    funcs.push((BestPathReason::LocalOriginated, compare_local_originated));
    funcs.push((BestPathReason::ASPath, compare_as_path));
    funcs.push((BestPathReason::Origin, compare_origin));
    funcs.push((BestPathReason::MultiExitDisc, compare_med));
    funcs.push((BestPathReason::External, compare_external));
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
    if a.local_pref > b.local_pref {
        return Ordering::Less;
    } else if a.local_pref == b.local_pref {
        return Ordering::Equal;
    }
    Ordering::Greater
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
    if a_len < b_len {
        return Ordering::Less;
    } else if a_len == b_len {
        return Ordering::Equal;
    }
    Ordering::Greater
}

fn compare_origin(a: &Path, b: &Path) -> Ordering {
    if a.origin < b.origin {
        return Ordering::Less;
    } else if a.origin == b.origin {
        return Ordering::Equal;
    }
    Ordering::Greater
}

fn compare_med(a: &Path, b: &Path) -> Ordering {
    if a.med < b.med {
        return Ordering::Less;
    } else if a.med == b.med {
        return Ordering::Equal;
    }
    Ordering::Greater
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
        return if a.timestamp < b.timestamp {
            Ordering::Less
        } else if a.timestamp.eq(&b.timestamp) {
            Ordering::Equal
        } else {
            Ordering::Greater
        };
    }
    Ordering::Equal
}

fn compare_peer_router_id(a: &Path, b: &Path) -> Ordering {
    return if a.peer_id < b.peer_id {
        Ordering::Less
    } else if a.peer_id == b.peer_id {
        Ordering::Equal
    } else {
        Ordering::Greater
    };
}

fn compare_peer_addr(a: &Path, b: &Path) -> Ordering {
    return if a.peer_addr < b.peer_addr {
        Ordering::Less
    } else if a.peer_addr == b.peer_addr {
        Ordering::Equal
    } else {
        Ordering::Greater
    };
}

#[cfg(test)]
mod tests {
    use futures::FutureExt;
    use ipnet::{IpNet, Ipv4Net};
    use rstest::rstest;
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr};
    use std::ops::Sub;
    use tokio::sync::mpsc::{channel, unbounded_channel};
    use tokio::time::{timeout, Duration, Instant};

    use crate::bgp::packet::attribute::{ASSegment, Base};
    use crate::bgp::packet::message::Message;
    use crate::bgp::packet::prefix::Prefix;
    use crate::bgp::path::BestPathReason;
    use crate::bgp::{
        event::RibEvent,
        family::{AddressFamily, Afi},
        packet::attribute::Attribute,
        path::{Path, PathKind},
        peer::neighbor::NeighborPair,
    };

    use super::{LocRib, LocRibStatus, RibManager, Table};

    #[test]
    fn works_table() {
        let mut table = Table::new();
    }

    #[test]
    fn works_rib_manager_add_peer() {
        let mut manager = RibManager::new(
            100,
            Ipv4Addr::new(1, 1, 1, 1),
            "test_endpoint".to_string(),
            vec![Afi::IPv4],
            false,
        );
        let (tx, _rx) = channel::<RibEvent>(128);
        manager
            .add_peer(
                NeighborPair::new(
                    IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
                    100,
                    Ipv4Addr::new(0, 0, 0, 0),
                ),
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
        let mut builder = crate::bgp::path::PathBuilder::builder(
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
				timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
				timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now().sub(Duration::from_secs(5)),
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
				timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
				timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
				timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
				timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
				timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
			LocRibStatus::MultiPathWithdrawn("10.0.0.0/24".parse().unwrap(), 1),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
			LocRibStatus::MultiPathWithdrawn("10.0.0.0/24".parse().unwrap(), 2),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
					timestamp: Instant::now(),
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
        let mut manager = RibManager::new(
            65000,
            Ipv4Addr::new(1, 1, 1, 1),
            String::from("dummy"),
            vec![AddressFamily::ipv4_unicast().afi],
            false,
        );

        // peer1
        let peer1 = NeighborPair::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
            65010,
            Ipv4Addr::new(2, 2, 2, 2),
        );
        let (peer1_tx, mut peer1_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer1.clone(), peer1_tx).unwrap();

        // peer2
        let peer2 = NeighborPair::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)),
            65020,
            Ipv4Addr::new(3, 3, 3, 3),
        );
        let (peer2_tx, mut peer2_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer2.clone(), peer2_tx).unwrap();

        // install paths from peer1
        let paths = vec![Path {
            id: 0,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: Instant::now(),
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
        manager.install_paths(peer1.clone(), paths).await.unwrap();

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
            Err(_) => assert!(true),
            Ok(_) => panic!("peer1 should not receive event"),
        }
        // install paths from peer2
        let paths = vec![
            Path {
                id: 1,
                best: false,
                reason: BestPathReason::NotBest,
                timestamp: Instant::now(),
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
                timestamp: Instant::now(),
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
                timestamp: Instant::now(),
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
        manager.install_paths(peer2.clone(), paths).await.unwrap();

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
            Err(_) => assert!(true),
            Ok(_) => panic!("peer2 should not receive event"),
        }

        // install paths from local
        let paths = vec![Path {
            id: 4,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: Instant::now(),
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
                NeighborPair::new(
                    IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
                    65000,
                    Ipv4Addr::new(1, 1, 1, 1),
                ),
                paths,
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
            .drop_paths(peer2.clone(), family.clone(), prefixes)
            .await
            .unwrap();

        // not recv
        match timeout(Duration::from_millis(10), peer1_rx.recv()).await {
            Err(_) => assert!(true),
            Ok(_) => panic!("peer1 should not receive event"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => assert!(true),
            Ok(_) => panic!("peer2 should not receive event"),
        }
        // drop from peer1
        let prefixes: Vec<(IpNet, u64)> = vec![("10.0.0.0/24".parse().unwrap(), 0)];
        manager
            .drop_paths(peer1.clone(), family.clone(), prefixes)
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
            Err(_) => assert!(true),
            Ok(_) => panic!("peer1 should not receive event"),
        }
        // drop from local
        let prefixes: Vec<(IpNet, u64)> = vec![("10.0.100.0/24".parse().unwrap(), 4)];
        manager
            .drop_paths(
                NeighborPair::new(
                    IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
                    65000,
                    Ipv4Addr::new(1, 1, 1, 1),
                ),
                family.clone(),
                prefixes,
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
        let mut manager = RibManager::new(
            65000,
            Ipv4Addr::new(1, 1, 1, 1),
            String::from("dummy"),
            vec![AddressFamily::ipv4_unicast().afi],
            false,
        );

        // peer1
        let peer1 = NeighborPair::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
            65000,
            Ipv4Addr::new(2, 2, 2, 2),
        );
        let (peer1_tx, mut peer1_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer1.clone(), peer1_tx).unwrap();

        // peer2
        let peer2 = NeighborPair::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)),
            65000,
            Ipv4Addr::new(3, 3, 3, 3),
        );
        let (peer2_tx, mut peer2_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer2.clone(), peer2_tx).unwrap();

        // install paths from peer1
        let paths = vec![Path {
            id: 0,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: Instant::now(),
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
        manager.install_paths(peer1.clone(), paths).await.unwrap();

        // not recv
        match timeout(Duration::from_millis(10), peer1_rx.recv()).await {
            Err(_) => assert!(true),
            Ok(_) => panic!("peer1 should not receive event"),
        }
        // not recv
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => assert!(true),
            Ok(_) => panic!("peer2 should not receive event"),
        }

        // install paths from local
        let paths = vec![Path {
            id: 4,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: Instant::now(),
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
                NeighborPair::new(
                    IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
                    65000,
                    Ipv4Addr::new(1, 1, 1, 1),
                ),
                paths,
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
            .drop_paths(peer1.clone(), family.clone(), prefixes)
            .await
            .unwrap();

        // not recv
        match timeout(Duration::from_millis(10), peer1_rx.recv()).await {
            Err(_) => assert!(true),
            Ok(_) => panic!("peer1 should not receive event"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => assert!(true),
            Ok(_) => panic!("peer2 should not receive event"),
        }
        // drop from local
        let prefixes: Vec<(IpNet, u64)> = vec![("10.0.100.0/24".parse().unwrap(), 4)];
        manager
            .drop_paths(
                NeighborPair::new(
                    IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
                    65000,
                    Ipv4Addr::new(1, 1, 1, 1),
                ),
                family.clone(),
                prefixes,
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
        let mut manager = RibManager::new(
            65000,
            Ipv4Addr::new(1, 1, 1, 1),
            String::from("dummy"),
            vec![AddressFamily::ipv4_unicast().afi],
            true,
        );

        // peer1
        let peer1 = NeighborPair::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
            65010,
            Ipv4Addr::new(2, 2, 2, 2),
        );
        let (peer1_tx, mut peer1_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer1.clone(), peer1_tx).unwrap();

        // peer2
        let peer2 = NeighborPair::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)),
            65020,
            Ipv4Addr::new(3, 3, 3, 3),
        );
        let (peer2_tx, mut peer2_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer2.clone(), peer2_tx).unwrap();

        // install paths from peer1
        let paths = vec![Path {
            id: 0,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: Instant::now(),
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
        manager.install_paths(peer1.clone(), paths).await.unwrap();

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
            Err(_) => assert!(true),
            Ok(_) => panic!("peer1 should not receive event"),
        }
        // install paths from peer2
        let paths = vec![
            Path {
                id: 1,
                best: false,
                reason: BestPathReason::NotBest,
                timestamp: Instant::now(),
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
                timestamp: Instant::now(),
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
        manager.install_paths(peer2.clone(), paths).await.unwrap();

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
            Err(_) => assert!(true),
            Ok(_) => panic!("peer2 should not receive event"),
        }

        // install paths from local
        let paths = vec![Path {
            id: 4,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: Instant::now(),
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
                NeighborPair::new(
                    IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
                    65000,
                    Ipv4Addr::new(1, 1, 1, 1),
                ),
                paths,
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
            .drop_paths(peer2.clone(), family.clone(), prefixes)
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
            Err(_) => assert!(true),
            Ok(_) => panic!("peer2 should not receive event"),
        }
        // drop from local
        let prefixes: Vec<(IpNet, u64)> = vec![("10.0.20.0/24".parse().unwrap(), 4)];
        manager
            .drop_paths(
                NeighborPair::new(
                    IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
                    65000,
                    Ipv4Addr::new(1, 1, 1, 1),
                ),
                family.clone(),
                prefixes,
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
        let mut manager = RibManager::new(
            65000,
            Ipv4Addr::new(1, 1, 1, 1),
            String::from("dummy"),
            vec![AddressFamily::ipv4_unicast().afi],
            true,
        );

        // peer1
        let peer1 = NeighborPair::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
            65000,
            Ipv4Addr::new(2, 2, 2, 2),
        );
        let (peer1_tx, mut peer1_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer1.clone(), peer1_tx).unwrap();

        // peer2
        let peer2 = NeighborPair::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2)),
            65000,
            Ipv4Addr::new(3, 3, 3, 3),
        );
        let (peer2_tx, mut peer2_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer2.clone(), peer2_tx).unwrap();

        // peer3
        let peer3 = NeighborPair::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 2, 2)),
            65010,
            Ipv4Addr::new(4, 4, 4, 4),
        );
        let (peer3_tx, mut peer3_rx) = channel::<RibEvent>(10);
        manager.add_peer(peer3.clone(), peer3_tx).unwrap();

        // install paths from peer1
        let paths = vec![Path {
            id: 0,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: Instant::now(),
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
        manager.install_paths(peer1.clone(), paths).await.unwrap();

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
            Err(_) => assert!(true),
            Ok(_) => panic!("peer1 should not receive event"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => assert!(true),
            Ok(_) => panic!("peer2 should not receive event"),
        }
        // install paths from peer2
        let paths = vec![
            Path {
                id: 1,
                best: false,
                reason: BestPathReason::NotBest,
                timestamp: Instant::now(),
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
                timestamp: Instant::now(),
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
        manager.install_paths(peer2.clone(), paths).await.unwrap();

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
            Err(_) => assert!(true),
            Ok(_) => panic!("peer1 should not receive event"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => assert!(true),
            Ok(_) => panic!("peer2 should not receive event"),
        }

        // install paths from peer3(ebgp)
        let paths = vec![Path {
            id: 4,
            best: false,
            reason: BestPathReason::NotBest,
            timestamp: Instant::now(),
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
        manager.install_paths(peer3.clone(), paths).await.unwrap();

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
            Err(_) => assert!(true),
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
            .drop_paths(peer2.clone(), family.clone(), prefixes)
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
            Err(_) => assert!(true),
            Ok(_) => panic!("peer1 should not receive event"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => assert!(true),
            Ok(_) => panic!("peer2 should not receive event"),
        }
        // drop from peer1
        let prefixes: Vec<(IpNet, u64)> = vec![("10.0.20.0/24".parse().unwrap(), 0)];
        manager
            .drop_paths(peer1.clone(), family.clone(), prefixes)
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
            Err(_) => assert!(true),
            Ok(_) => panic!("peer1 should not receive event"),
        }
        match timeout(Duration::from_millis(10), peer2_rx.recv()).await {
            Err(_) => assert!(true),
            Ok(_) => panic!("peer2 should not receive event"),
        }
        // drop from peer3
        let prefixes: Vec<(IpNet, u64)> = vec![("10.0.30.0/24".parse().unwrap(), 4)];
        manager
            .drop_paths(peer3.clone(), family.clone(), prefixes)
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
            Err(_) => assert!(true),
            Ok(_) => panic!("peer3 should not receive event"),
        }
    }
}
