use std::collections::HashMap;

use super::family::AddressFamily;
use super::packet;

pub(crate) type CapabilitySet = HashMap<u8, Capability>;

#[derive(Debug, Clone)]
pub(crate) enum Capability {
    MultiProtocol(MultiProtocol),
    RouteRefresh,
    ExtendedNextHop(ExtendedNextHop),
    BGPExtendedMessage,
    GracefulRestart(GracefulRestart),
    FourOctetASNumber(FourOctetASNumber),
    AddPath(AddPath),
    EnhancedRouteRefresh,
}

impl From<packet::capability::Capability> for Capability {
    fn from(cap: packet::capability::Capability) -> Self {
        match cap {
            packet::capability::Capability::MultiProtocol(family) => {
                Capability::MultiProtocol(MultiProtocol { family })
            }
            packet::capability::Capability::RouteRefresh => Capability::RouteRefresh,
            packet::capability::Capability::ExtendedNextHop(values) => {
                Capability::ExtendedNextHop(ExtendedNextHop { values })
            }
            packet::capability::Capability::BGPExtendedMessage => Capability::BGPExtendedMessage,
            packet::capability::Capability::GracefulRestart(flag, time, values) => {
                Capability::GracefulRestart(GracefulRestart { flag, time, values })
            }
            packet::capability::Capability::FourOctetASNumber(asn) => {
                Capability::FourOctetASNumber(FourOctetASNumber { inner: asn })
            }
            packet::capability::Capability::AddPath(family, sr) => {
                Capability::AddPath(AddPath { family, flag: sr })
            }
            packet::capability::Capability::EnhancedRouteRefresh => {
                Capability::EnhancedRouteRefresh
            }
            packet::capability::Capability::Unsupported(code, data) => {
                panic!("unsupported capability")
            }
        }
    }
}

impl Into<packet::capability::Capability> for &Capability {
    fn into(self) -> packet::capability::Capability {
        match self {
            Capability::MultiProtocol(m) => {
                packet::capability::Capability::MultiProtocol(m.family.clone())
            }
            Capability::RouteRefresh => packet::capability::Capability::RouteRefresh,
            Capability::ExtendedNextHop(e) => {
                packet::capability::Capability::ExtendedNextHop(e.values.clone())
            }
            Capability::BGPExtendedMessage => packet::capability::Capability::BGPExtendedMessage,
            Capability::GracefulRestart(g) => {
                packet::capability::Capability::GracefulRestart(g.flag, g.time, g.values.clone())
            }
            Capability::FourOctetASNumber(f) => {
                packet::capability::Capability::FourOctetASNumber(f.inner)
            }
            Capability::AddPath(a) => {
                packet::capability::Capability::AddPath(a.family.clone(), a.flag)
            }
            Capability::EnhancedRouteRefresh => {
                packet::capability::Capability::EnhancedRouteRefresh
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MultiProtocol {
    family: AddressFamily,
}

impl MultiProtocol {
    pub fn new(family: AddressFamily) -> Self {
        Self { family }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ExtendedNextHop {
    values: Vec<(AddressFamily, u16)>,
}

impl ExtendedNextHop {
    pub fn new(values: Vec<(AddressFamily, u16)>) -> Self {
        Self { values }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FourOctetASNumber {
    inner: u32,
}

impl FourOctetASNumber {
    pub fn new(asn: u32) -> Self {
        Self { inner: asn }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GracefulRestart {
    flag: u8,
    time: u16,
    values: Vec<(AddressFamily, u8)>,
}

impl GracefulRestart {
    pub fn new(flag: u8, time: u16, values: Vec<(AddressFamily, u8)>) -> Self {
        Self { flag, time, values }
    }
}

#[derive(Debug)]
pub(crate) struct RouteRefresh {}

#[derive(Debug)]
pub(crate) struct EnhancedRouteRefresh {}

#[derive(Debug)]
pub(crate) struct BGPExtendedMessage {}

#[derive(Debug, Clone)]
pub(crate) struct AddPath {
    family: AddressFamily,
    flag: u8,
}

impl AddPath {
    pub fn new(family: AddressFamily, flag: u8) -> Self {
        Self { family, flag }
    }
}
