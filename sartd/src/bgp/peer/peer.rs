use std::sync::{Arc, Mutex};

use super::{fsm::FiniteStateMachine, neighbor::Neighbor};

#[derive(Debug)]
pub(crate) struct Peer {
    neighbor: Neighbor,
    fsm: Arc<Mutex<FiniteStateMachine>>,
}
