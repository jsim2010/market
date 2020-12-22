//! Implements [`Producer`] and [`Consumer`] for various types of channels.
//!
//! A channel is the most generic implementation of a market. A channel manages the exchange of goods from one or more [`Producer`]s to one or more [`Consumer`]s.
mod error;

pub use error::{WithdrawnDemand, WithdrawnSupply};

use {
    crate::{ConsumeFailure, Consumer, ProduceFailure, Producer},
    core::marker::PhantomData,
    fehler::throws,
    std::sync::mpsc::{channel, sync_channel, SyncSender},
};

/// The [`Producer`] of a crossbeam channel.
pub type CrossbeamProducer<G> = crossbeam_channel::Sender<G>;
/// The [`Consumer`] of a crossbeam channel.
pub type CrossbeamConsumer<G> = crossbeam_channel::Receiver<G>;

/// Creates a channel of `S` [`Style`] with a max stock of `size`.
#[inline]
#[must_use]
pub fn create<S: Style>(structure: Structure, size: Size) -> (S::Producer, S::Consumer) {
    match structure {
        Structure::BilateralMonopoly => match size {
            Size::Infinite => S::infinite(),
            Size::Finite(value) => S::finite(value),
        },
    }
}

/// Defines how many [`Producer`]s and [`Consumer`]s a channel has.
#[derive(Clone, Copy, Debug)]
pub enum Structure {
    /// 1 producer and 1 consumer.
    BilateralMonopoly,
}

/// The size of the stock of a channel.
#[derive(Clone, Copy, Debug)]
pub enum Size {
    /// Infinite size.
    Infinite,
    /// Finite size.
    Finite(usize),
}

/// Describes the style or implementation of a channel.
pub trait Style {
    /// The created [`Producer`] type.
    type Producer: Producer;
    /// The created [`Consumer`] type.
    type Consumer: Consumer;

    /// Returns the producer and consumer of a channel with an infinite stack size.
    fn infinite() -> (Self::Producer, Self::Consumer);
    /// Returns the producer and consumer of a channel with a stack size of `size`.
    fn finite(size: usize) -> (Self::Producer, Self::Consumer);
}

/// A channel as implemented by [`crossbeam_channel`] with a good of `G`.
#[derive(Debug)]
pub struct Crossbeam<G> {
    /// The type of the good that is exchanged on the channel.
    good: PhantomData<G>,
}

impl<G> Style for Crossbeam<G> {
    type Producer = crossbeam_channel::Sender<G>;
    type Consumer = crossbeam_channel::Receiver<G>;

    #[inline]
    fn infinite() -> (Self::Producer, Self::Consumer) {
        crossbeam_channel::unbounded()
    }

    #[inline]
    fn finite(size: usize) -> (Self::Producer, Self::Consumer) {
        crossbeam_channel::bounded(size)
    }
}

impl<G> Producer for crossbeam_channel::Sender<G> {
    type Good = G;
    type Failure = ProduceFailure<WithdrawnDemand>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        self.try_send(good)?
    }
}

impl<G> Consumer for crossbeam_channel::Receiver<G> {
    type Good = G;
    type Failure = ConsumeFailure<WithdrawnSupply>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        self.try_recv()?
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
    type Consumer = std::sync::mpsc::Receiver<G>;

    #[inline]
    fn infinite() -> (Self::Producer, Self::Consumer) {
        let (sender, receiver) = channel();
        (StdProducer::Async(sender), receiver)
    }

    #[inline]
    fn finite(size: usize) -> (Self::Producer, Self::Consumer) {
        let (sender, receiver) = sync_channel(size);
        (StdProducer::Sync(sender), receiver)
    }
}

/// One of the senders implemented by [`std::sync::mpsc`].
#[derive(Debug)]
pub enum StdProducer<G> {
    /// The asynchronous sender.
    Async(std::sync::mpsc::Sender<G>),
    /// The synchronous sender.
    Sync(SyncSender<G>),
}

impl<G> Producer for StdProducer<G> {
    type Good = G;
    type Failure = ProduceFailure<WithdrawnDemand>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        match *self {
            Self::Async(ref sender) => sender.send(good)?,
            Self::Sync(ref sender) => sender.try_send(good)?,
        }
    }
}

impl<G> Consumer for std::sync::mpsc::Receiver<G> {
    type Good = G;
    type Failure = ConsumeFailure<WithdrawnSupply>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        self.try_recv()?
    }
}
