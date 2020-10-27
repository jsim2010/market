//! Implements synchronization items.
use {
    crate::error::InfallibleFailure,
    core::sync::atomic::{AtomicBool, Ordering},
    fehler::{throw, throws},
};

/// Sends a status that can be activated but not deactivated.
#[derive(Debug)]
pub struct Trigger {
    /// Holds the status of the trigger.
    is_activated: AtomicBool,
}

impl Trigger {
    /// Creates a new `Trigger`.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            is_activated: AtomicBool::new(false),
        }
    }
}

impl crate::Consumer for Trigger {
    type Good = ();
    type Failure = InfallibleFailure;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        if !self.is_activated.load(Ordering::Relaxed) {
            throw!(InfallibleFailure)
        }
    }
}

// TODO: Change to InfallibleProducerFailure.
impl crate::Producer for Trigger {
    type Good = ();
    type Failure = crate::ClassicalProducerFailure<never::Never>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, _good: Self::Good) {
        self.is_activated.store(true, Ordering::Relaxed);
    }
}
