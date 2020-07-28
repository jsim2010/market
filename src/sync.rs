//! Implements synchronization items.
use {
    crate::channel::{StdConsumer, StdProducer},
    std::sync::mpsc,
};

/// Creates the items that implement a trigger for synchronizing threads.
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
