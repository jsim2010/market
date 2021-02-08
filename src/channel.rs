//! Implements [`Producer`] and [`Consumer`] for various types of channels.
//!
//! A channel is the most generic implementation of a market. A channel manages the exchange of goods from one or more [`Producer`]s to one or more [`Consumer`]s.
mod error;

pub use error::{WithdrawnDemandFault, WithdrawnSupplyFault};

use {
    crate::{ConsumeFailure, Consumer, ProduceFailure, Producer},
    core::marker::PhantomData,
    fehler::throws,
    std::sync::mpsc::{channel, sync_channel, SyncSender},
};

/// Describes the style or implementation of a channel.
pub trait Style {
    /// The type that implements [`Producer`].
    type Producer: Producer;
    /// The type that implements [`Consumer`].
    type Consumer: Consumer;

    /// Returns the [`Producer`] and [`Consumer`] of a channel described by `description` with an infinite stack size.
    fn infinite(description: String) -> (Self::Producer, Self::Consumer);
    /// Returns the [`Producer`] and [`Consumer`] of a channel described by `description` with a stack size of `size`.
    fn finite(description: String, size: usize) -> (Self::Producer, Self::Consumer);
}

/// Creates a channel of `S` [`Style`] with a max stock of `size`.
///
/// If `size` matches [`Size::Infinite`], returns the [`Producer`] and [`Consumer`] returned by `S::infinite()`.
/// If `size` matches [`Size::Finite(value)`], returns the [`Producer`] and [`Consumer`] returned by `S::finite(value)`.
#[inline]
#[must_use]
pub fn create<S: Style>(description: String, size: Size) -> (S::Producer, S::Consumer) {
    match size {
        Size::Infinite => S::infinite(description),
        Size::Finite(value) => S::finite(description, value),
    }
}

/// Describes the size of the stock of a channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    /// Infinite size.
    Infinite,
    /// Finite size.
    Finite(usize),
}

/// A sender implemented by [`std::sync::mpsc`].
#[derive(Debug)]
enum StdSender<G> {
    /// The asynchronous sender.
    Async(std::sync::mpsc::Sender<G>),
    /// The synchronous sender.
    Sync(SyncSender<G>),
}

/// Implements [`Producer`] for goods of type `G` to a channel created by [`std::sync::mpsc`].
#[derive(Debug)]
pub struct StdProducer<G> {
    /// Describes `Self`.
    description: String,
    /// The [`StdSender`] of the channel.
    sender: StdSender<G>,
}

impl<G> Producer for StdProducer<G> {
    type Good = G;
    type Failure = ProduceFailure<WithdrawnDemandFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        match self.sender {
            #[allow(clippy::map_err_ignore)] // Error is not used in output.
            StdSender::Async(ref sender) => sender.send(good).map_err(|_| {
                ProduceFailure::Fault(WithdrawnDemandFault::new(self.description.clone()))
            })?,
            StdSender::Sync(ref sender) => sender.try_send(good).map_err(|error| match error {
                std::sync::mpsc::TrySendError::Full(_) => ProduceFailure::FullStock,
                std::sync::mpsc::TrySendError::Disconnected(_) => {
                    ProduceFailure::Fault(WithdrawnDemandFault::new(self.description.clone()))
                }
            })?,
        }
    }
}

/// Implements [`Consumer`] for goods of type `G` from a channel created by [`std::sync::mpsc`].
#[derive(Debug)]
pub struct StdConsumer<G> {
    /// Describes `Self`.
    description: String,
    /// The [`std::sync::mpsc::Receiver`] of the channel.
    receiver: std::sync::mpsc::Receiver<G>,
}

impl<G> Consumer for StdConsumer<G> {
    type Good = G;
    type Failure = ConsumeFailure<WithdrawnSupplyFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        self.receiver.try_recv().map_err(|error| match error {
            std::sync::mpsc::TryRecvError::Empty => ConsumeFailure::EmptyStock,
            std::sync::mpsc::TryRecvError::Disconnected => {
                ConsumeFailure::Fault(WithdrawnSupplyFault::new(self.description.clone()))
            }
        })?
    }
}

/// A channel as implemented by [`std::sync::mpsc`] with a good of `G`.
#[derive(Debug)]
pub struct Std<G> {
    /// The type of good that is exchanged on the channel.
    good: PhantomData<G>,
}

impl<G> Style for Std<G> {
    type Producer = StdProducer<G>;
    type Consumer = StdConsumer<G>;

    #[inline]
    fn infinite(description: String) -> (Self::Producer, Self::Consumer) {
        let (sender, receiver) = channel();
        (
            StdProducer {
                description: description.clone(),
                sender: StdSender::Async(sender),
            },
            StdConsumer {
                description,
                receiver,
            },
        )
    }

    #[inline]
    fn finite(description: String, size: usize) -> (Self::Producer, Self::Consumer) {
        let (sender, receiver) = sync_channel(size);
        (
            StdProducer {
                description: description.clone(),
                sender: StdSender::Sync(sender),
            },
            StdConsumer {
                description,
                receiver,
            },
        )
    }
}

/// Implements [`Producer`] for goods of type `G` to a crossbeam channel.
#[derive(Debug)]
pub struct CrossbeamProducer<G> {
    /// Describes `Self`.
    description: String,
    /// The [`crossbeam_channel::Sender`] of the channel.
    sender: crossbeam_channel::Sender<G>,
}

impl<G> Producer for CrossbeamProducer<G> {
    type Good = G;
    type Failure = ProduceFailure<WithdrawnDemandFault>;

    /// Attempts to send `good` to a crossbeam channel.
    ///
    /// If attempt fails, throws a [`ProduceFailure`] describing the failure.
    /// If source of failure is [`WithdrawnDemandFault`], provides channel description in failure.
    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        self.sender.try_send(good).map_err(|error| match error {
            crossbeam_channel::TrySendError::Full(_) => ProduceFailure::FullStock,
            crossbeam_channel::TrySendError::Disconnected(_) => {
                ProduceFailure::Fault(WithdrawnDemandFault::new(self.description.clone()))
            }
        })?
    }
}

/// Implements [`Consumer`] for goods of type `G` from a crossbeam channel.
#[derive(Debug)]
pub struct CrossbeamConsumer<G> {
    /// Describes `Self`.
    description: String,
    /// The [`crossbeam_channel::Receiver`] of the channel.
    receiver: crossbeam_channel::Receiver<G>,
}

impl<G> Consumer for CrossbeamConsumer<G> {
    type Good = G;
    type Failure = ConsumeFailure<WithdrawnSupplyFault>;

    /// Attempts to retrieve `good` from a crossbeam channel.
    ///
    /// If attempt fails, throws a [`ConsumeFailure`] describing the failure.
    /// If source of failure is [`WithdrawnSupplyFault`], provides channel description in failure.
    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        self.receiver.try_recv().map_err(|error| match error {
            crossbeam_channel::TryRecvError::Empty => ConsumeFailure::EmptyStock,
            crossbeam_channel::TryRecvError::Disconnected => {
                ConsumeFailure::Fault(WithdrawnSupplyFault::new(self.description.clone()))
            }
        })?
    }
}

/// A channel that exchanges goods of type `G` implemented by [`crossbeam_channel`].
#[derive(Debug)]
pub struct Crossbeam<G> {
    /// The type of the good that is exchanged on the channel.
    good: PhantomData<G>,
}

impl<G> Style for Crossbeam<G> {
    type Producer = CrossbeamProducer<G>;
    type Consumer = CrossbeamConsumer<G>;

    #[inline]
    fn infinite(description: String) -> (Self::Producer, Self::Consumer) {
        let (sender, receiver) = crossbeam_channel::unbounded();
        (
            CrossbeamProducer {
                description: description.clone(),
                sender,
            },
            CrossbeamConsumer {
                description,
                receiver,
            },
        )
    }

    #[inline]
    fn finite(description: String, size: usize) -> (Self::Producer, Self::Consumer) {
        let (sender, receiver) = crossbeam_channel::bounded(size);
        (
            CrossbeamProducer {
                description: description.clone(),
                sender,
            },
            CrossbeamConsumer {
                description,
                receiver,
            },
        )
    }
}
