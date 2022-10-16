use std::net::{IpAddr, Ipv4Addr};

use crate::bgp::capability::Capability;

#[derive(Debug)]
pub(crate) struct Neighbor {
    asn: u32,
    router_id: Ipv4Addr,
    addr: IpAddr,
    acceptable_capabilities: Vec<Capability>,
    hold_time: u16,
}

impl Neighbor {
    pub fn new(asn: u32) -> Self {
        Self {
            asn,
            router_id: Ipv4Addr::new(0, 0, 0, 0),
            addr: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            acceptable_capabilities: Vec::new(),
            hold_time: 0,
        }
    }

    pub fn get_asn(&self) -> u32 {
        self.asn
    }

    pub fn get_router_id(&self) -> Ipv4Addr {
        self.router_id
    }

    pub fn router_id(&mut self, id: Ipv4Addr) -> &mut Self {
        self.router_id = id;
        self
    }

    pub fn get_addr(&self) -> IpAddr {
        self.addr
    }

    pub fn addr(&mut self, a: IpAddr) -> &mut Self {
        self.addr = a;
        self
    }

    pub fn get_hold_time(&self) -> u16 {
        self.hold_time
    }

    pub fn hold_time(&mut self, t: u16) -> &mut Self {
        self.hold_time = t;
        self
    }
}
