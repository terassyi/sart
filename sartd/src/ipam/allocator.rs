use std::net::IpAddr;

use ipnet::IpNet;

use super::{bitset::BitSet, error::Error};

pub(crate) trait Allocator {
    fn allocate(&mut self, addr: IpAddr) -> Result<(), Error>;
    fn allocate_next(&mut self) -> Result<(), Error>;
    fn release(&mut self, addr: IpAddr) -> Result<(), Error>;
}

#[derive(Debug)]
struct BitAllocator {
    cidr: IpNet,
    allocator: BitSet,
    allocated: usize,
}

impl Allocator for BitAllocator {
    fn allocate(&mut self, addr: IpAddr) -> Result<(), Error> {
        Ok(())
    }

    fn allocate_next(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn release(&mut self, addr: IpAddr) -> Result<(), Error> {
        Ok(())
    }
}
