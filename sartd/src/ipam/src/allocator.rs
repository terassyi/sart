use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use ipnet::IpNet;

use super::{bitset::{BitSet, BitSetError}, error::Error};

#[derive(Debug, Copy, Clone)]
pub enum AllocatorMethod {
    Bit,
}

#[derive(Debug)]
pub enum Allocator {
    Bit(BitAllocator),
}

impl Allocator {
    const MAX_SIZE: u8 = 32;
    pub fn new(cidr: IpNet, method: AllocatorMethod) -> Result<Allocator, Error> {
        if let IpNet::V6(c) = cidr {
            if 128 - c.prefix_len() > Allocator::MAX_SIZE {
                return Err(Error::CIDRTooLarge(c.prefix_len()))
            }
        }
        match method {
            AllocatorMethod::Bit => Ok(Allocator::Bit(BitAllocator::new(cidr))),
        }
    }

    pub fn allocate(&mut self, addr: &IpAddr) -> Result<IpAddr, Error> {
        match self {
            Allocator::Bit(a) => a.allocate(addr),
        }
    }

    pub fn allocate_next(&mut self) -> Result<IpAddr, Error> {
        match self {
            Allocator::Bit(a) => a.allocate_next(),
        }
    }

    pub fn release(&mut self, addr: &IpAddr) -> Result<IpAddr, Error> {
        match self {
            Allocator::Bit(a) => a.release(addr),
        }
    }

    pub fn is_allocated(&self, addr: &IpAddr) -> bool {
        match self {
            Allocator::Bit(a) => a.is_allocated(addr),
        }
    }

    pub fn cidr(&self) -> &IpNet {
        match self {
            Allocator::Bit(a) => &a.cidr,
        }
    }
}

#[derive(Debug)]
pub struct BitAllocator {
    cidr: IpNet,
    allocator: BitSet,
    allocated: usize,
}

impl BitAllocator {
    fn new(cidr: IpNet) -> BitAllocator {
        let size = match cidr {
            IpNet::V4(c) => 32 - c.prefix_len(),
            IpNet::V6(c) => 128 - c.prefix_len(),
        };
        BitAllocator {
            cidr,
            allocator: BitSet::new(2_u128.pow(size as u32)),
            allocated: 0,
        }
    }

    fn size(&self) -> u128 {
        self.allocator.size()
    }

    fn allocate(&mut self, addr: &IpAddr) -> Result<IpAddr, Error> {
        let idx = addr_to_index(addr, &self.cidr)? as u128;

        self.allocator.set_true(idx).map_err(Error::BitSet)?;
        self.allocated += 1;
        Ok(*addr)
    }

    fn allocate_next(&mut self) -> Result<IpAddr, Error> {

        let index = self.allocator.set_next()
            .map_err(|e| match e {
                BitSetError::Full => Error::Full,
                _ => Error::BitSet(e)
            })?;
        self.allocated += 1;

        Ok(index_to_addr(index, &self.cidr))
    }

    fn is_allocated(&self, addr: &IpAddr) -> bool {
        let idx = match addr_to_index(addr, &self.cidr) {
            Ok(i) => i,
            Err(_) => return false,
        };

        self.allocator.is_set(idx)
    }

    fn release(&mut self, addr: &IpAddr) -> Result<IpAddr, Error> {
        if self.allocated == 0 {
            return Err(Error::NoReleasableAddress);
        }
        let idx = addr_to_index(addr, &self.cidr)? as u128;

        self.allocator.set(idx, false).map_err(Error::BitSet)?;

        self.allocated -= 1;

        Ok(*addr)
    }
}

fn addr_to_index(addr: &IpAddr, cidr: &IpNet) -> Result<u128, Error> {
    // https://github.com/rust-lang/rust/issues/113744
    // We cannot convert Ipv{4/6}Addr to bits directly
    match cidr {
        IpNet::V4(c) => {
            let min_octs = c.network().octets();
            let min_bits: u32 = min_octs[3] as u32
                + ((min_octs[2] as u32) << 8)
                + ((min_octs[1] as u32) << 16)
                + ((min_octs[0] as u32) << 24);
            match addr {
                IpAddr::V4(a) => {
                    if !cidr.contains(addr) {
                        return Err(Error::NotContains);
                    }
                    let a_octs = a.octets();
                    let a_bits: u32 = (a_octs[3] as u32)
                        + ((a_octs[2] as u32) << 8)
                        + ((a_octs[1] as u32) << 16)
                        + ((a_octs[0] as u32) << 24);
                    Ok((a_bits - min_bits) as u128)
                }
                IpAddr::V6(_) => Err(Error::ProtocolMistmatch),
            }
        }
        IpNet::V6(c) => {
            let min_octs = c.network().octets();
            match addr {
                IpAddr::V4(_) => Err(Error::ProtocolMistmatch),
                IpAddr::V6(a) => {
                    if !cidr.contains(addr) {
                        return Err(Error::NotContains);
                    }
                    let a_octs = a.octets();
                    let mut min_bits = 0u128;
                    let mut a_bits = 0u128;
                    for i in 0..16 {
                        min_bits += (min_octs[15 - i] as u128) << (i * 8);
                        a_bits += (a_octs[15 - i] as u128) << (i * 8);
                    }
                    Ok(a_bits - min_bits)
                }
            }
        }
    }
}

fn index_to_addr(index: u128, cidr: &IpNet) -> IpAddr {
    match cidr {
        IpNet::V4(c) => {
            let a_octs = c.network().octets();
            let a_bits: u32 = (a_octs[3] as u32)
                + ((a_octs[2] as u32) << 8)
                + ((a_octs[1] as u32) << 16)
                + ((a_octs[0] as u32) << 24);
            let new_bits = a_bits + index as u32;
            IpAddr::V4(Ipv4Addr::from(new_bits.to_be_bytes()))
        },
        IpNet::V6(c) => {
            let a_octs = c.network().octets();
            let mut a_bits = 0u128;
            for i in 0..16 {
                a_bits += (a_octs[15 - i] as u128) << (i * 8);
            }
            let new_bits = a_bits + index as u128;
            IpAddr::V6(Ipv6Addr::from(new_bits.to_be_bytes()))
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use ipnet::{IpNet, Ipv4Net, Ipv6Net};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use std::str::FromStr;
    use rstest::rstest;

    #[rstest(
        cidr,
        expected,
        case(IpNet::V4(Ipv4Net::from_str("10.0.0.0/24").unwrap()), 2_u128.pow(8)),
        case(IpNet::V4(Ipv4Net::from_str("10.0.0.0/32").unwrap()), 1),
        case(IpNet::V4(Ipv4Net::from_str("10.0.0.0/31").unwrap()), 2),
        case(IpNet::V4(Ipv4Net::from_str("10.0.0.0/15").unwrap()), 131072),
    )]
    fn works_bit_allocator_size(cidr: IpNet, expected: u128) {
        let alloc = BitAllocator::new(cidr);
        assert_eq!(alloc.size(), expected);
    }

    #[rstest(
        addr, 
        cidr, 
        expected, 
        case(IpAddr::V4(Ipv4Addr::from_str("10.0.0.0").unwrap()), IpNet::V4(Ipv4Net::from_str("10.0.0.0/24").unwrap()), 0),
        case(IpAddr::V4(Ipv4Addr::from_str("10.0.0.1").unwrap()), IpNet::V4(Ipv4Net::from_str("10.0.0.0/24").unwrap()), 1),
        case(IpAddr::V4(Ipv4Addr::from_str("10.0.0.100").unwrap()), IpNet::V4(Ipv4Net::from_str("10.0.0.0/24").unwrap()), 100),
        case(IpAddr::V4(Ipv4Addr::from_str("153.130.0.149").unwrap()), IpNet::V4(Ipv4Net::from_str("153.128.0.0/13").unwrap()), 131221),
        case(IpAddr::V6(Ipv6Addr::from_str("2001:db8::").unwrap()), IpNet::V6(Ipv6Net::from_str("2001:db8::/32").unwrap()), 0),
        case(IpAddr::V6(Ipv6Addr::from_str("2001:db8::1").unwrap()), IpNet::V6(Ipv6Net::from_str("2001:db8::/32").unwrap()), 1),
        case(IpAddr::V6(Ipv6Addr::from_str("2001:db8::80").unwrap()), IpNet::V6(Ipv6Net::from_str("2001:db8::/32").unwrap()), 128),
    )]
    fn works_addr_to_index(addr: IpAddr, cidr: IpNet, expected: u128) {
        let idx = addr_to_index(&addr, &cidr).unwrap();
        assert_eq!(idx, expected);
    }

    #[rstest(
        addr, 
        cidr, 
        expected, 
        case(IpAddr::V6(Ipv6Addr::from_str("2001:db8::").unwrap()), IpNet::V4(Ipv4Net::from_str("10.0.0.0/24").unwrap()), Error::ProtocolMistmatch),
        case(IpAddr::V4(Ipv4Addr::from_str("10.0.1.0").unwrap()), IpNet::V4(Ipv4Net::from_str("10.0.0.0/24").unwrap()), Error::NotContains),
    )]
    fn fails_addr_to_index(addr: IpAddr, cidr: IpNet, expected: Error) {
        let res = addr_to_index(&addr, &cidr);
        match res {
            Ok(_) => panic!("this test should not be pass here"),
            Err(e) => assert_eq!(e, expected),
        }
    }

    #[rstest(
        index,
        cidr, 
        expected, 
        case(0, IpNet::V4(Ipv4Net::from_str("10.0.0.0/24").unwrap()), IpAddr::V4(Ipv4Addr::from_str("10.0.0.0").unwrap())),
        case(1, IpNet::V4(Ipv4Net::from_str("10.0.0.0/24").unwrap()), IpAddr::V4(Ipv4Addr::from_str("10.0.0.1").unwrap())),
        case(100, IpNet::V4(Ipv4Net::from_str("10.0.0.0/24").unwrap()), IpAddr::V4(Ipv4Addr::from_str("10.0.0.100").unwrap())),
        case(131221, IpNet::V4(Ipv4Net::from_str("153.128.0.0/13").unwrap()), IpAddr::V4(Ipv4Addr::from_str("153.130.0.149").unwrap())),
        case(0, IpNet::V6(Ipv6Net::from_str("2001:db8::/32").unwrap()), IpAddr::V6(Ipv6Addr::from_str("2001:db8::").unwrap())),
        case(1, IpNet::V6(Ipv6Net::from_str("2001:db8::/32").unwrap()), IpAddr::V6(Ipv6Addr::from_str("2001:db8::1").unwrap())),
        case(128, IpNet::V6(Ipv6Net::from_str("2001:db8::/32").unwrap()), IpAddr::V6(Ipv6Addr::from_str("2001:db8::80").unwrap())),
    )]
    fn works_index_to_addr(index: u128, cidr: IpNet, expected: IpAddr) {
        let addr = index_to_addr(index, &cidr);
        assert_eq!(addr, expected);
    }

    #[rstest(
        cidr,
        addr,
        index,
        case(IpNet::V4(Ipv4Net::from_str("10.0.0.0/24").unwrap()), IpAddr::V4(Ipv4Addr::from_str("10.0.0.1").unwrap()), 1),
        case(IpNet::V4(Ipv4Net::from_str("10.0.0.0/24").unwrap()), IpAddr::V4(Ipv4Addr::from_str("10.0.0.139").unwrap()), 139),
        case(IpNet::V6(Ipv6Net::from_str("2001:db8::/96").unwrap()), IpAddr::V6(Ipv6Addr::from_str("2001:db8::1").unwrap()), 1),
        case(IpNet::V6(Ipv6Net::from_str("2001:db8::/96").unwrap()), IpAddr::V6(Ipv6Addr::from_str("2001:db8::80").unwrap()), 128),
    )]
    fn works_bit_allocator_allocate(cidr: IpNet, addr: IpAddr, index: u128) {
        let mut alloc = BitAllocator::new(cidr);
        alloc.allocate(&addr).unwrap();
        assert!(alloc.allocator.is_set(index));
    }
}
