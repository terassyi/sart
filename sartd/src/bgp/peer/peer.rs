use std::net::IpAddr;
use std::sync::Mutex;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::bgp::config::NeighborConfig;
use crate::bgp::event::{AdministrativeEvent, Event};

use super::{fsm::FiniteStateMachine, neighbor::Neighbor};

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct PeerKey {
    addr: IpAddr,
    asn: u32,
}

impl PeerKey {
    pub fn new(addr: IpAddr, asn: u32) -> Self {
        Self { addr, asn }
    }
}

#[derive(Debug)]
pub(crate) struct Peer {
    neighbor: Neighbor,
    fsm: Mutex<FiniteStateMachine>,
    admin_rx: UnboundedReceiver<Event>,
}

impl Peer {
    pub fn new(neighbor: NeighborConfig, rx: UnboundedReceiver<Event>) -> Self {
        Self {
            neighbor: Neighbor::from(neighbor),
            fsm: Mutex::new(FiniteStateMachine::new()),
            admin_rx: rx,
        }
    }

    pub async fn handle(&mut self) {
        // handle event
        while let Some(event) = self.admin_rx.recv().await {
            match event {
                Event::Admin(event) => match event {
                    AdministrativeEvent::ManualStart => {
                        println!("AdminEvent::ManualStart");
                        self.fsm.lock().unwrap().mv(Event::Admin(event));
                    }
                    _ => unimplemented!(),
                },
                _ => panic!("unimplemented"),
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct PeerManager {
    pub event_queue: UnboundedSender<Event>,
}

impl PeerManager {
    pub fn new(queue: UnboundedSender<Event>) -> Self {
        Self { event_queue: queue }
    }
}
