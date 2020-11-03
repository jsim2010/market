//! Implements [`Producer`] and [`Consumer`] for synchronization items.
use {
    crate::{Consumer, FaultlessFailure, Producer},
    core::{convert::Infallible, sync::atomic::{AtomicBool, Ordering}},
    fehler::{throw, throws},
};

/// Sends a status that can be activated but not deactivated.
#[derive(Debug)]
pub struct Trigger {
    /// Holds the status of the trigger.
    is_activated: AtomicBool,
}

impl Trigger {
    /// Creates a new `Trigger` that is unactive.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            is_activated: AtomicBool::new(false),
        }
    }
}

impl Consumer for Trigger {
    type Good = ();
    type Failure = FaultlessFailure;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        if !self.is_activated.load(Ordering::Relaxed) {
            throw!(FaultlessFailure)
        }
    }
}

impl Producer for Trigger {
    type Good = ();
    type Failure = Infallible;

    #[inline]
    fn produce(&self, _good: Self::Good) -> Result<Self::Good, Self::Failure> {
        self.is_activated.store(true, Ordering::Relaxed);
        Ok(())
    }
}
