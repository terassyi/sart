use std::io;
use std::net::{Ipv4Addr, Ipv6Addr};

use crate::bgp::error::*;
use crate::bgp::family::{AddressFamily, Afi};

use bytes::{Buf, BufMut, BytesMut};
use ipnet::{IpNet, Ipv4Net, Ipv6Net};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Prefix {
    inner: ipnet::IpNet,
    path_id: Option<u32>,
}

impl Prefix {
    pub fn new(prefix: ipnet::IpNet, path_id: Option<u32>) -> Self {
        Self {
            inner: prefix,
            path_id,
        }
    }

    pub fn decode(
        family: &AddressFamily,
        add_path_enabled: bool,
        data: &mut BytesMut,
    ) -> Result<Self, Error> {
        let path_id = if add_path_enabled {
            Some(data.get_u32())
        } else {
            None
        };
        let length = data.get_u8();
        let prefix_len = if length % 8 != 0 {
            (length / 8) + 1
        } else {
            length / 8
        };
        let p = match family.afi {
            Afi::IPv4 => {
                let mut b = [0u8; 4];
                for i in 0..(prefix_len as usize) {
                    b[i] = data.get_u8();
                }
                let addr = Ipv4Addr::from(b);
                let pref = Ipv4Net::new(addr, length)
                    .map_err(|_| Error::UpdateMessage(UpdateMessageError::InvalidNetworkField))?;
                IpNet::V4(pref)
            }
            Afi::IPv6 => {
                let mut b = [0u8; 16];
                for i in 0..(prefix_len as usize) {
                    b[i] = data.get_u8()
                }
                let addr = Ipv6Addr::from(b);
                let pref = Ipv6Net::new(addr, length)
                    .map_err(|_| Error::UpdateMessage(UpdateMessageError::InvalidNetworkField))?;
                IpNet::V6(pref)
            }
        };
        Ok(Prefix { inner: p, path_id })
    }

    pub fn encode(&self, dst: &mut BytesMut) -> io::Result<()> {
        if let Some(id) = self.path_id {
            dst.put_u32(id)
        }
        dst.put_u8(self.inner.prefix_len());
        match self.inner {
            IpNet::V4(a) => {
                let oct = a.addr().octets();
                let (aa, _) = oct
                    .as_slice()
                    .split_at(prefix_bytes_len(self.inner.prefix_len() as usize));
                dst.put_slice(aa);
            }
            IpNet::V6(a) => {
                let oct = a.addr().octets();
                let (aa, _) = oct
                    .as_slice()
                    .split_at(prefix_bytes_len(self.inner.prefix_len() as usize));
                dst.put_slice(aa);
            }
        };
        Ok(())
    }

    pub fn len(&self) -> usize {
        let length = match self.path_id {
            Some(_) => 5,
            None => 1,
        };
        length + prefix_bytes_len(self.inner.prefix_len() as usize)
    }
}

fn prefix_bytes_len(prefix_len: usize) -> usize {
    match prefix_len % 8 {
        0 => prefix_len / 8,
        _ => 1 + (prefix_len / 8),
    }
}

impl From<IpNet> for Prefix {
    fn from(pref: IpNet) -> Self {
        Self {
            inner: pref,
            path_id: None,
        }
    }
}

impl From<Prefix> for IpNet {
    fn from(val: Prefix) -> Self {
        val.inner
    }
}

impl From<&Prefix> for IpNet {
    fn from(val: &Prefix) -> Self {
        val.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;
    use ipnet::{IpNet, Ipv4Net};
    use std::net::Ipv4Addr;

    use crate::bgp::family::{AddressFamily, Afi, Safi};
    use rstest::rstest;
    #[rstest(
		family,
		add_path_enabled,
		input,
		expected,
		case(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, false, vec![0x18, 0x0a, 0x02, 0x00], Prefix{inner: IpNet::V4(Ipv4Net::new(Ipv4Addr::new(0x0a, 0x02, 0x00, 0x00), 24).unwrap()), path_id: None}),
		case(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, false, vec![0x08, 0x1e], Prefix{inner: IpNet::V4(Ipv4Net::new(Ipv4Addr::new(0x1e, 0x00, 0x00, 0x00), 8).unwrap()), path_id: None}),
		case(AddressFamily{afi: Afi::IPv6, safi: Safi::Unicast}, false, vec![0x40, 0x20, 0x01, 0x0d, 0xb8, 0x00, 0x01, 0x00, 0x02], Prefix{inner: IpNet::V6(Ipv6Net::new(Ipv6Addr::new(0x2001, 0x0db8, 0x0001, 0x0002, 0x0000, 0x0000, 0x0000, 0x0000), 64).unwrap()), path_id: None}),
		case(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, true, vec![0x00, 0x00, 0x00, 0x01, 0x20, 0x05, 0x05, 0x05, 0x05], Prefix{inner: IpNet::V4(Ipv4Net::new(Ipv4Addr::new(0x05, 0x05, 0x05, 0x05), 32).unwrap()), path_id: Some(1)}),
	)]
    fn works_prefix_decode(
        family: AddressFamily,
        add_path_enabled: bool,
        input: Vec<u8>,
        expected: Prefix,
    ) {
        let mut buf = BytesMut::from(input.as_slice());
        match Prefix::decode(&family, add_path_enabled, &mut buf) {
            Ok(pref) => assert_eq!(expected, pref),
            Err(_) => panic!("failed"),
        }
    }
    // #[rstest(
    // 	family,
    // 	add_path_enabled,
    // 	input,
    // 	expected,
    // 	case(AddressFamily{afi: Afi::IPv6, safi: Safi::Unicast}, false, vec![0x18, 0x0a, 0x02, 0x00], UpdateMessageError::InvalidNetworkField),
    // )]
    // fn failed_prefix_decode(family: AddressFamily, add_path_enabled: bool, input: Vec<u8>, expected: UpdateMessageError) {
    //        let mut buf = BytesMut::from(input.as_slice());
    // 	match Prefix::decode(family, add_path_enabled, &mut buf) {
    // 		Ok(_) => panic!("failed"),
    // 		Err(e) => match e {
    // 			Error::UpdateMessage(ee) => match ee {
    // 				UpdateMessageError::InvalidNetworkField => assert_eq!(expected, ee),
    // 				_ => panic!("failed"),
    // 			},
    // 			_ => panic!("failed"),
    // 		}
    // 	}
    // }
}
