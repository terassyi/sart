use std::collections::HashMap;

use super::family::AddressFamily;
use super::packet::capability;

pub(crate) type CapabilitySet = HashMap<u8, Capability>;

#[derive(Debug)]
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

#[derive(Debug)]
pub(crate) struct MultiProtocol {
    family: AddressFamily,
}

#[derive(Debug)]
pub(crate) struct ExtendedNextHop {
    values: Vec<(AddressFamily, u16)>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FourOctetASNumber {
    inner: u32,
}

#[derive(Debug)]
pub(crate) struct GracefulRestart {
    flag: u8,
    time: u16,
    values: Vec<(AddressFamily, u8)>,
}

#[derive(Debug)]
pub(crate) struct RouteRefresh {}

#[derive(Debug)]
pub(crate) struct EnhancedRouteRefresh {}

#[derive(Debug)]
pub(crate) struct BGPExtendedMessage {}

#[derive(Debug)]
pub(crate) struct AddPath {
    family: AddressFamily,
    flag: u8,
}
