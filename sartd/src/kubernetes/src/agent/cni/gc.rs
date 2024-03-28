use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use kube::{
    api::{DeleteParams, ListParams},
    Api, Client, ResourceExt,
};

use sartd_ipam::manager::AllocatorSet;

use crate::{
    agent::reconciler::node_bgp::ENV_HOSTNAME,
    crd::address_block::{AddressBlock, ADDRESS_BLOCK_NODE_LABEL},
};

pub struct GarbageCollector {
    interval: Duration,
    client: Client,
    allocator: Arc<AllocatorSet>,
    node: String,
    blocks: HashMap<String, GarbageCollectorMarker>,
    pods: HashMap<String, GarbageCollectorMarker>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GarbageCollectorMarker {
    Unused,
    Deleted,
}

impl GarbageCollector {
    pub fn new(
        interval: Duration,
        client: Client,
        allocator: Arc<AllocatorSet>,
    ) -> GarbageCollector {
        let node_name = std::env::var(ENV_HOSTNAME).expect("HOSTNAME environment value is not set");
        GarbageCollector {
            interval,
            client,
            allocator,
            node: node_name,
            blocks: HashMap::new(),
            pods: HashMap::new(),
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn run(&mut self) {
        let mut ticker = tokio::time::interval(self.interval);

        loop {
            ticker.tick().await;

            let address_block_api = Api::<AddressBlock>::all(self.client.clone());

            // let label_selector = form
            let list_params = ListParams::default()
                .labels(&format!("{}={}", ADDRESS_BLOCK_NODE_LABEL, self.node));
            let block_list = match address_block_api.list(&list_params).await {
                Ok(list) => list,
                Err(e) => {
                    tracing::warn!(error=?e,"Failed to list AddressBlock");
                    continue;
                }
            };

            let mut unused = HashMap::new();
            let mut used = HashSet::new();

            {
                let alloc_set = self.allocator.clone();
                let allocator = alloc_set.inner.lock().unwrap();

                for ab in block_list.iter() {
                    let block_opt = allocator.blocks.get(&ab.name_any());
                    match block_opt {
                        Some(block) => {
                            if block.allocator.is_empty() {
                                match self.blocks.get_mut(&block.name) {
                                    Some(status) => {
                                        if GarbageCollectorMarker::Unused.eq(status) {
                                            unused.insert(
                                                block.name.clone(),
                                                GarbageCollectorMarker::Deleted,
                                            );
                                            tracing::info!(
                                                block = block.name,
                                                gc_mark =? GarbageCollectorMarker::Deleted,
                                                "Update GC marker",
                                            );
                                        }
                                    }
                                    None => {
                                        unused.insert(
                                            block.name.clone(),
                                            GarbageCollectorMarker::Unused,
                                        );
                                        tracing::info!(
                                            block = block.name,
                                            gc_mark =? GarbageCollectorMarker::Unused,
                                            "Add GC marker",
                                        );
                                    }
                                }
                            } else {
                                used.insert(block.name.clone());
                            }
                        }
                        None => match self.blocks.get_mut(&ab.name_any()) {
                            Some(status) => {
                                if GarbageCollectorMarker::Unused.eq(status) {
                                    unused.insert(ab.name_any(), GarbageCollectorMarker::Deleted);
                                    tracing::info!(
                                        block = ab.name_any(),
                                        gc_mark =? GarbageCollectorMarker::Deleted,
                                        "Update GC marker",
                                    );
                                }
                            }
                            None => {
                                unused.insert(ab.name_any(), GarbageCollectorMarker::Unused);
                                tracing::info!(
                                    block = ab.name_any(),
                                    gc_mark =? GarbageCollectorMarker::Unused,
                                    "Add GC marker",
                                );
                            }
                        },
                    }
                }

                for (block_name, block) in allocator.blocks.iter() {
                    if block.allocator.is_empty() {
                        match self.blocks.get_mut(block_name) {
                            Some(status) => {
                                if GarbageCollectorMarker::Unused.eq(status) {
                                    unused.insert(
                                        block_name.clone(),
                                        GarbageCollectorMarker::Deleted,
                                    );
                                    tracing::info!(
                                        block = block_name,
                                        gc_mark =? GarbageCollectorMarker::Deleted,
                                        "Update GC marker",
                                    );
                                }
                            }
                            None => {
                                unused.insert(block_name.clone(), GarbageCollectorMarker::Unused);
                                tracing::info!(
                                    block = block_name,
                                    gc_mark =? GarbageCollectorMarker::Unused,
                                    "Add GC marker",
                                );
                            }
                        }
                    } else {
                        used.insert(block_name.clone());
                    }
                }
            }

            for (k, v) in unused.iter() {
                self.blocks.insert(k.clone(), *v);
            }

            for k in used.iter() {
                self.blocks.remove(k);
            }

            let mut deleted_keys = Vec::new();
            for (block, status) in self.blocks.iter() {
                if GarbageCollectorMarker::Deleted.eq(status) {
                    if let Ok(Some(_ab)) = address_block_api.get_opt(block).await {
                        if let Err(err) = address_block_api
                            .delete(block, &DeleteParams::default())
                            .await
                        {
                            tracing::warn!(err=?err, "Failed to delete AddressBlock by GC");
                            continue;
                        }
                        deleted_keys.push(block.clone());
                    }
                }
            }
            for k in deleted_keys.iter() {
                self.blocks.remove(k);
            }
        }
    }
}
