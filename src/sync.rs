//! Implements synchronization items.
use {
    crate::channel::{StdConsumer, StdProducer},
    core::sync::atomic::{AtomicBool, Ordering},
    std::sync::mpsc,
};

// TODO: Adapt trigger so that it is a Consumer and Producer. Potentially requires adapting Consumer and Producer so that it does not have a good.
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

    /// Triggers `self`.
    #[inline]
    pub fn trigger(&self) {
        self.is_activated.store(true, Ordering::Relaxed);
    }

    /// Returns if `self` has been triggered.
    #[inline]
    pub fn is_triggered(&self) -> bool {
        self.is_activated.load(Ordering::Relaxed)
    }
}

/// Creates the items that implement a trigger for synchronizing threads.
// TODO: Remove trigger on next release.
#[deprecated]
#[must_use]
#[inline]
pub fn trigger() -> (Actuator, Releaser) {
    let (actuator, releaser) = mpsc::channel();
    (actuator.into(), releaser.into())
}

/// The Producer of a trigger.
pub type Actuator = StdProducer<()>;
/// The Consumer of a trigger.
pub type Releaser = StdConsumer<()>;
