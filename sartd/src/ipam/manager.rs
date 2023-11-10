use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use ipnet::IpNet;

use super::{
    allocator::{Allocator, AllocatorMethod},
    error::Error,
};

#[derive(Debug)]
pub(crate) struct AllocatorSet {
    pub inner: Arc<Mutex<AllocatorSetInner>>,
}

impl AllocatorSet {
    pub(crate) fn new() -> AllocatorSet {
        AllocatorSet {
            inner: Arc::new(Mutex::new(AllocatorSetInner::new())),
        }
    }
}

#[derive(Debug)]
pub(crate) struct AllocatorSetInner {
    pub blocks: HashMap<String, Block>,
    // blocks: HashMap<String, Arc<Mutex<Block>>>,
    pub auto_assigns: Vec<String>,
}
impl AllocatorSetInner {
    pub(crate) fn new() -> AllocatorSetInner {
        AllocatorSetInner {
            blocks: HashMap::new(),
            auto_assigns: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Block {
    pub name: String,
    pub pool_name: String,
    // pub address_type: AddressType,
    pub allocator: Allocator,
}

impl Block {
    pub(crate) fn new(name: String, pool_name: String, cidr: IpNet) -> Result<Block, Error> {
        Ok(Block {
            name,
            pool_name,
            allocator: Allocator::new(cidr, AllocatorMethod::Bit)?,
        })
    }
}
