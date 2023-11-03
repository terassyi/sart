use super::allocator::Allocator;

pub(crate) struct Manager<A: Allocator> {
    allocator: A,
}
