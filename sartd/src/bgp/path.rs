use std::net::IpAddr;

#[derive(Debug)]
pub(crate) struct Path {
	next_hop: IpAddr,
}
