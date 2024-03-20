use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    vec,
};

use ipnet::IpNet;

use super::{
    allocator::{Allocator, AllocatorMethod},
    error::Error,
};

#[derive(Debug)]
pub struct AllocatorSet {
    pub inner: Arc<Mutex<AllocatorSetInner>>,
}

impl AllocatorSet {
    pub fn new() -> AllocatorSet {
        AllocatorSet {
            inner: Arc::new(Mutex::new(AllocatorSetInner::new())),
        }
    }
}

impl Default for AllocatorSet {
    fn default() -> AllocatorSet {
        AllocatorSet::new()
    }
}

#[derive(Debug)]
pub struct AllocatorSetInner {
    pub blocks: HashMap<String, Block>,
    pub alloc_info: HashMap<String, AllocationInfo>,
    pub auto_assign: Option<String>,
}
impl AllocatorSetInner {
    pub fn new() -> AllocatorSetInner {
        AllocatorSetInner {
            blocks: HashMap::new(),
            alloc_info: HashMap::new(),
            auto_assign: None,
        }
    }

    pub fn insert(&mut self, block: Block, auto_assign: bool) -> Result<Option<Block>, Error> {
        if auto_assign {
            if self.auto_assign.is_some() {
                return Err(Error::AutoAssignableBlockAlreadyExists);
            }
            self.auto_assign = Some(block.name.clone());
        }
        let res = self.blocks.insert(block.name.clone(), block);
        Ok(res)
    }

    pub fn remove(&mut self, name: &str) -> Option<Block> {
        let res = self.blocks.remove(name);
        if let Some(a) = &self.auto_assign {
            if a.eq(name) {
                self.auto_assign = None;
            }
        }
        res
    }

    pub fn get(&self, name: &str) -> Option<&Block> {
        self.blocks.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Block> {
        self.blocks.get_mut(name)
    }

    // This returns pairs of block name and addresses which are allocated from its block.
    // From given addresses, iterate blocks(not alloc_info) hash map and collect wanted pairs.
    // This doesn't care given addresses are actually allocated.
    // Because this is used to recover the allocation information from desired resources(such as Service and Pod).
    // For example, when being restarted the controller, it has to recover the allocation ifnormation from stored resources.
    pub fn get_blocks_from_addrs(&self, addrs: &[IpAddr]) -> HashMap<String, Vec<IpAddr>> {
        let mut res: HashMap<String, Vec<IpAddr>> = HashMap::new();
        for addr in addrs.iter() {
            for (name, block) in self.blocks.iter() {
                if block.allocator.cidr().contains(addr) {
                    if let Some(a) = res.get_mut(name) {
                        a.push(*addr);
                    } else {
                        res.insert(name.to_string(), vec![*addr]);
                    }
                }
            }
        }
        res
    }
}

impl Default for AllocatorSetInner {
    fn default() -> AllocatorSetInner {
        AllocatorSetInner::new()
    }
}

#[derive(Debug)]
pub struct Block {
    pub name: String,
    pub pool_name: String,
    // pub address_type: AddressType,
    pub allocator: Allocator,
}

impl Block {
    pub fn new(name: String, pool_name: String, cidr: IpNet) -> Result<Block, Error> {
        Ok(Block {
            name,
            pool_name,
            allocator: Allocator::new(cidr, AllocatorMethod::Bit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AllocationInfo {
    // We can allocate multiple addresses from one block to a service or pod
    pub blocks: HashMap<String, Vec<IpAddr>>,
}

impl AllocationInfo {
    pub fn insert(&mut self, block: &str, addr: IpAddr) {
        match self.blocks.get_mut(block) {
            Some(b) => {
                if !b.contains(&addr) {
                    b.push(addr);
                }
            }
            None => {
                self.blocks.insert(block.to_string(), vec![addr]);
            }
        }
    }

    pub fn remove(&mut self, block: &str, addr: IpAddr) {
        let mut remove = false;
        if let Some(b) = self.blocks.get_mut(block) {
            b.retain(|a| addr.ne(a));
            if b.is_empty() {
                remove = true;
            }
        }
        if remove {
            self.blocks.remove(block);
        }
    }

    pub fn get(&self, block: &str) -> Option<&Vec<IpAddr>> {
        self.blocks.get(block)
    }
}
