//! Implements [`Producer`]s and [`Consumer`]s for queues.
//!
//! Queues function similarly to channels with the only difference being that queues share access to a single memory source so there is no ['Failure'] if the other participant is dropped.
use {
    crate::{Producer, Consumer, FaultlessFailure},
    core::convert::Infallible,
    crossbeam_queue::SegQueue,
    fehler::throws,
    std::sync::Arc,
};

/// Creates a [`Producer`] and [`Consumer`] for a queue.
#[inline]
#[must_use]
pub fn create_supply_chain<G>() -> (Supplier<G>, Procurer<G>) {
    let supplier_stock = Arc::new(SegQueue::new());
    let procurer_stock = Arc::clone(&supplier_stock);
    (Supplier{stock: supplier_stock}, Procurer{stock: procurer_stock})
}

/// Consumes goods of type `G` from a queue.
#[derive(Debug)]
pub struct Supplier<G> {
    /// The stock from which [`Self`] consumes.
    stock: Arc<SegQueue<G>>,
}

impl<G> Producer for Supplier<G> {
    type Good = G;
    type Failure = Infallible;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        self.stock.push(good);
    }
}

/// Produces goods of type `G` to a queue.
#[derive(Debug)]
pub struct Procurer<G> {
    /// The stock to which [`Self`] produces.
    stock: Arc<SegQueue<G>>,
}

impl<G> Consumer for Procurer<G> {
    type Good = G;
    type Failure = FaultlessFailure;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        self.stock.pop().ok_or(FaultlessFailure)?
    }
}
