use std::collections::HashMap;

use super::family::{AddressFamily, Afi, Safi};
use super::packet::capability::Cap;

#[derive(Debug, Clone)]
pub struct CapabilitySet {
    inner: HashMap<u8, Capability>,
}

impl CapabilitySet {
    pub fn default(asn: u32) -> Self {
        let mut inner = HashMap::new();
        inner.insert(
            Cap::FOUR_OCTET_AS_NUMBER,
            Capability::FourOctetASNumber(FourOctetASNumber::new(asn)),
        );
        inner.insert(Cap::ROUTE_REFRESH, Capability::RouteRefresh);
        inner.insert(
            Cap::MULTI_PROTOCOL,
            Capability::MultiProtocol(MultiProtocol {
                family: AddressFamily {
                    afi: Afi::IPv4,
                    safi: Safi::Unicast,
                },
            }),
        );
        Self { inner }
    }

    pub fn with_empty() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn iter(&self) -> Iter {
        Iter {
            base: self.inner.iter(),
        }
    }

    pub fn keys(&self) -> std::collections::hash_map::Keys<u8, Capability> {
        self.inner.keys()
    }

    pub fn values(&self) -> std::collections::hash_map::Values<u8, Capability> {
        self.inner.values()
    }

    pub fn get(&self, k: &u8) -> Option<&Capability> {
        self.inner.get(k)
    }

    pub fn get_mut(&mut self, k: &u8) -> Option<&mut Capability> {
        self.inner.get_mut(k)
    }

    pub fn insert(&mut self, k: u8, v: Capability) -> Option<Capability> {
        self.inner.insert(k, v)
    }

    pub fn remove(&mut self, k: &u8) -> Option<Capability> {
        self.inner.remove(k)
    }

    pub fn get_key_value(&self, k: &u8) -> Option<(&u8, &Capability)> {
        self.inner.get_key_value(k)
    }
}

pub struct Iter<'a> {
    base: std::collections::hash_map::Iter<'a, u8, Capability>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a u8, &'a Capability);
    fn next(&mut self) -> Option<Self::Item> {
        self.base.next()
    }
}

#[derive(Debug, Clone)]
pub enum Capability {
    MultiProtocol(MultiProtocol),
    RouteRefresh,
    ExtendedNextHop(ExtendedNextHop),
    BGPExtendedMessage,
    GracefulRestart(GracefulRestart),
    FourOctetASNumber(FourOctetASNumber),
    AddPath(AddPath),
    EnhancedRouteRefresh,
}

impl From<Cap> for Capability {
    fn from(cap: Cap) -> Self {
        match cap {
            Cap::MultiProtocol(family) => Capability::MultiProtocol(MultiProtocol { family }),
            Cap::RouteRefresh => Capability::RouteRefresh,
            Cap::ExtendedNextHop(values) => Capability::ExtendedNextHop(ExtendedNextHop { values }),
            Cap::BGPExtendedMessage => Capability::BGPExtendedMessage,
            Cap::GracefulRestart(flag, time, values) => {
                Capability::GracefulRestart(GracefulRestart { flag, time, values })
            }
            Cap::FourOctetASNumber(asn) => {
                Capability::FourOctetASNumber(FourOctetASNumber { inner: asn })
            }
            Cap::AddPath(family, sr) => Capability::AddPath(AddPath { family, flag: sr }),
            Cap::EnhancedRouteRefresh => Capability::EnhancedRouteRefresh,
            Cap::Unsupported(_code, _data) => {
                panic!("unsupported capability")
            }
        }
    }
}

impl From<&Capability> for Cap {
    fn from(val: &Capability) -> Self {
        match val {
            Capability::MultiProtocol(m) => Cap::MultiProtocol(m.family),
            Capability::RouteRefresh => Cap::RouteRefresh,
            Capability::ExtendedNextHop(e) => Cap::ExtendedNextHop(e.values.clone()),
            Capability::BGPExtendedMessage => Cap::BGPExtendedMessage,
            Capability::GracefulRestart(g) => {
                Cap::GracefulRestart(g.flag, g.time, g.values.clone())
            }
            Capability::FourOctetASNumber(f) => Cap::FourOctetASNumber(f.inner),
            Capability::AddPath(a) => Cap::AddPath(a.family, a.flag),
            Capability::EnhancedRouteRefresh => Cap::EnhancedRouteRefresh,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MultiProtocol {
    family: AddressFamily,
}

impl MultiProtocol {
    pub fn new(family: AddressFamily) -> Self {
        Self { family }
    }
}

#[derive(Debug, Clone)]
pub struct ExtendedNextHop {
    values: Vec<(AddressFamily, u16)>,
}

impl ExtendedNextHop {
    pub fn new(values: Vec<(AddressFamily, u16)>) -> Self {
        Self { values }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FourOctetASNumber {
    inner: u32,
}

impl FourOctetASNumber {
    pub fn new(asn: u32) -> Self {
        Self { inner: asn }
    }

    pub fn set(&mut self, asn: u32) {
        self.inner = asn;
    }
}

#[derive(Debug, Clone)]
pub struct GracefulRestart {
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
pub struct RouteRefresh {}

#[derive(Debug)]
pub struct EnhancedRouteRefresh {}

#[derive(Debug)]
pub struct BGPExtendedMessage {}

#[derive(Debug, Clone)]
pub struct AddPath {
    family: AddressFamily,
    flag: u8,
}

impl AddPath {
    pub fn new(family: AddressFamily, flag: u8) -> Self {
        Self { family, flag }
    }
}

#[cfg(test)]
mod tests {
    use super::CapabilitySet;
    use crate::packet::capability::Cap;

    #[test]
    fn works_capability_set_collection_methods() {
        let capset = CapabilitySet::default(100);
        let a = capset
            .iter()
            .filter(|(&k, _)| k == Cap::FOUR_OCTET_AS_NUMBER);
        assert_eq!(1, a.count());
        let keys = capset.keys();
        assert_eq!(3, keys.len());
    }
}
