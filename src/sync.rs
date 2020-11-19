//! Implements [`Producer`] and [`Consumer`] for synchronization items.
use {
    crate::{channel, ConsumeFailure, Consumer, ProduceFailure, Producer, TakenParticipant},
    core::sync::atomic::{AtomicBool, Ordering},
    fehler::throws,
};

/// The mechanism for activating an irreversible action.
///
/// The name derives from the name for the method of exploding the charge of a firearm.
#[derive(Debug)]
pub struct Lock {
    /// Provides communication between the [`Trigger`] and the [`Hammer`] of the lock.
    channel: channel::Channel<channel::Crossbeam<()>>,
}

impl Lock {
    /// Creates a new [`Lock`].
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes the [`Trigger`] from `self.
    ///
    /// If the trigger has already been taken, throws a [`TakenParticipant`].
    #[inline]
    #[throws(TakenParticipant)]
    pub fn trigger(&mut self) -> Trigger {
        self.channel.producer()?.into()
    }

    /// Takes the [`Hammer`] from `self`.
    ///
    /// If the hammer has already been taken, throws a [`TakenParticipant`].
    #[inline]
    #[throws(TakenParticipant)]
    pub fn hammer(&mut self) -> Hammer {
        self.channel.consumer()?.into()
    }
}

impl Default for Lock {
    #[inline]
    fn default() -> Self {
        Self {
            channel: channel::Channel::new(channel::Size::Finite(1)),
        }
    }
}

/// Sends a status that can be activated but not deactivated.
#[derive(Debug)]
pub struct Trigger {
    /// If the trigger has ben activated.
    is_activated: AtomicBool,
    /// The [`Producer`].
    producer: channel::KindProducer<channel::Crossbeam<()>>,
}

impl From<channel::KindProducer<channel::Crossbeam<()>>> for Trigger {
    #[inline]
    fn from(producer: channel::KindProducer<channel::Crossbeam<()>>) -> Self {
        Self {
            is_activated: false.into(),
            producer,
        }
    }
}

impl Producer for Trigger {
    type Good = ();
    type Failure = ProduceFailure<channel::DisconnectedFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) -> Self::Good {
        if !self.is_activated.load(Ordering::Relaxed) {
            self.is_activated.store(true, Ordering::Relaxed);
            self.producer.produce(good)?;
        }
    }
}

/// The [`Consumer`] of a [`Lock`].
///
/// The name derives from the hammer of a firearm, whose movement is caused by pulling the trigger.
#[derive(Debug)]
pub struct Hammer {
    /// If the hammer has been activated.
    is_activated: AtomicBool,
    /// The [`Consumer`].
    consumer: channel::CrossbeamConsumer<()>,
}

impl Consumer for Hammer {
    type Good = ();
    type Failure = ConsumeFailure<channel::DisconnectedFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        if !self.is_activated.load(Ordering::Relaxed) {
            let consumption = self.consumer.consume();

            if consumption.is_ok() {
                self.is_activated.store(true, Ordering::Relaxed);
            }

            consumption?
        }
    }
}

impl From<channel::CrossbeamConsumer<()>> for Hammer {
    #[inline]
    fn from(consumer: channel::CrossbeamConsumer<()>) -> Self {
        Self {
            is_activated: false.into(),
            consumer,
        }
    }
}
