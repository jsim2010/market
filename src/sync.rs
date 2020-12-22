//! Implements [`Producer`] and [`Consumer`] for synchronization items.
use {
    crate::{Consumer, InsufficientStockFailure, Producer},
    core::{
        convert::Infallible,
        sync::atomic::{AtomicBool, Ordering},
    },
    crossbeam_queue::ArrayQueue,
    fehler::{throw, throws},
    std::sync::Arc,
};

/// Creates the [`Trigger`] and [`Hammer`] of a lock.
///
/// A lock is a simple synchronization that exchanges a binary signal. Once the lock has been triggered, it stays triggered/active forever.
///
/// The names `lock`, `Trigger` and `Hammer` come from the ignition mechanism of a firearm.
#[inline]
#[must_use]
pub fn create_lock() -> (Trigger, Hammer) {
    let trigger_bool = Arc::new(AtomicBool::new(false));
    let hammer_bool = Arc::clone(&trigger_bool);
    (
        Trigger {
            is_activated: trigger_bool,
        },
        Hammer {
            is_activated: hammer_bool,
        },
    )
}

/// Produces a binary signal that cannot be deactivated.
#[derive(Debug)]
pub struct Trigger {
    /// If the trigger has been activated.
    is_activated: Arc<AtomicBool>,
}

impl Producer for Trigger {
    type Good = ();
    type Failure = Infallible;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, _: Self::Good) {
        self.is_activated.store(true, Ordering::Relaxed);
    }
}

/// Consumes a binary signal that cannot be deactivated.
#[derive(Debug)]
pub struct Hammer {
    /// If the hammer has been activated.
    is_activated: Arc<AtomicBool>,
}

impl Consumer for Hammer {
    type Good = ();
    type Failure = InsufficientStockFailure;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        if !self.is_activated.load(Ordering::Relaxed) {
            throw!(InsufficientStockFailure);
        }
    }
}

/// Creates a [`Deliverer`] and [`Accepter`] for an exchange with a stock of 1.
#[inline]
#[must_use]
pub fn create_delivery<G>() -> (Deliverer<G>, Accepter<G>) {
    let passer_item = Arc::new(ArrayQueue::new(1));
    let catcher_item = Arc::clone(&passer_item);
    (
        Deliverer { item: passer_item },
        Accepter { item: catcher_item },
    )
}

/// Delivers an item.
#[derive(Debug)]
pub struct Deliverer<G> {
    /// The item to be delivered.
    item: Arc<ArrayQueue<G>>,
}

impl<G> Producer for Deliverer<G> {
    type Good = G;
    type Failure = InsufficientStockFailure;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        #[allow(clippy::map_err_ignore)] // Error is ().
        self.item.push(good).map_err(|_| InsufficientStockFailure)?;
    }
}

/// Accepts an item.
#[derive(Debug)]
pub struct Accepter<G> {
    /// The item to be accepted.
    item: Arc<ArrayQueue<G>>,
}

impl<G> Consumer for Accepter<G> {
    type Good = G;
    type Failure = InsufficientStockFailure;

    #[throws(Self::Failure)]
    #[inline]
    fn consume(&self) -> Self::Good {
        self.item.pop().ok_or(InsufficientStockFailure)?
    }
}
