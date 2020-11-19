//! Implements [`Producer`] and [`Consumer`] for various types of channels.
mod error;

pub use error::DisconnectedFault;

use {
    crate::{ConsumeFailure, Consumer, Participant, ProduceFailure, Producer, TakenParticipant},
    core::{fmt::Debug, marker::PhantomData},
    fehler::throws,
};

/// The [`Kind::Producer`] of `K`.
pub type KindProducer<K> = <K as Kind>::Producer;

/// The [`Kind::Consumer`] of `K`.
pub type KindConsumer<K> = <K as Kind>::Consumer;

/// The size of the stock for a channel.
#[derive(Clone, Copy, Debug)]
pub enum Size {
    /// Infinite size.
    Infinite,
    /// Finite size.
    Finite(usize),
}

/// Describes a kind of channel.
pub trait Kind {
    /// The [`Producer`] created by this channel kind.
    type Producer: Producer;
    /// The [`Consumer`] created by this channel kind.
    type Consumer: Consumer;

    /// Returns the producer and consumer of a channel with an infinite stack size.
    fn infinite() -> (Self::Producer, Self::Consumer);
    /// Returns the producer and consumer of a channel with a stack size of `size`.
    fn finite(size: usize) -> (Self::Producer, Self::Consumer);
}

/// Manages a channel as defined by `K`.
#[derive(Debug)]
pub struct Channel<K: Kind> {
    /// The producer of the channel.
    producer: Option<K::Producer>,
    /// The consumer of the channel.
    consumer: Option<K::Consumer>,
}

impl<K: Kind> Channel<K> {
    /// Creates a new channel with a stock size of `size`.
    #[inline]
    #[must_use]
    pub fn new(size: Size) -> Self {
        let (producer, consumer) = match size {
            Size::Infinite => K::infinite(),
            Size::Finite(value) => K::finite(value),
        };

        Self {
            producer: Some(producer),
            consumer: Some(consumer),
        }
    }

    /// Takes the [`Producer`] from `self`.
    #[inline]
    #[throws(TakenParticipant)]
    pub fn producer(&mut self) -> K::Producer {
        self.producer
            .take()
            .ok_or(TakenParticipant(Participant::Producer))?
    }

    /// Takes the [`Consumer`] from `self`.
    #[inline]
    #[throws(TakenParticipant)]
    pub fn consumer(&mut self) -> K::Consumer {
        self.consumer
            .take()
            .ok_or(TakenParticipant(Participant::Consumer))?
    }
}

/// Implements a crossbeam channel with a good of `G`.
#[derive(Debug)]
pub struct Crossbeam<G> {
    /// The good that is stored on the channel.
    good: PhantomData<G>,
}

impl<G> Kind for Crossbeam<G> {
    type Producer = CrossbeamProducer<G>;
    type Consumer = CrossbeamConsumer<G>;

    #[inline]
    fn infinite() -> (Self::Producer, Self::Consumer) {
        let (sender, receiver) = crossbeam_channel::unbounded();
        (sender.into(), receiver.into())
    }

    #[inline]
    fn finite(size: usize) -> (Self::Producer, Self::Consumer) {
        let (sender, receiver) = crossbeam_channel::bounded(size);
        (sender.into(), receiver.into())
    }
}

/// A [`std::sync::mpsc::Receiver`] that implements [`Consumer<Good = G>`].
#[derive(Debug)]
pub struct StdConsumer<G> {
    /// The receiver.
    rx: std::sync::mpsc::Receiver<G>,
}

impl<G> Consumer for StdConsumer<G> {
    type Good = G;
    type Failure = ConsumeFailure<DisconnectedFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        self.rx.try_recv()?
    }
}

impl<G> From<std::sync::mpsc::Receiver<G>> for StdConsumer<G> {
    #[inline]
    fn from(rx: std::sync::mpsc::Receiver<G>) -> Self {
        Self { rx }
    }
}

/// A [`std::sync::mpsc::Sender`] that implements [`Producer`].
#[derive(Debug)]
pub struct StdProducer<G> {
    /// The sender.
    tx: std::sync::mpsc::Sender<G>,
}

impl<G> Producer for StdProducer<G> {
    type Good = G;
    type Failure = ProduceFailure<DisconnectedFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        self.tx.send(good)?
    }
}

impl<G> From<std::sync::mpsc::Sender<G>> for StdProducer<G> {
    #[inline]
    fn from(tx: std::sync::mpsc::Sender<G>) -> Self {
        Self { tx }
    }
}

/// A [`crossbeam_channel::Receiver`] that implements [`Consumer`].
#[derive(Debug)]
pub struct CrossbeamConsumer<G> {
    /// The receiver.
    rx: crossbeam_channel::Receiver<G>,
}

impl<G> Consumer for CrossbeamConsumer<G> {
    type Good = G;
    type Failure = ConsumeFailure<DisconnectedFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        self.rx.try_recv()?
    }
}

impl<G> From<crossbeam_channel::Receiver<G>> for CrossbeamConsumer<G> {
    #[inline]
    fn from(rx: crossbeam_channel::Receiver<G>) -> Self {
        Self { rx }
    }
}

/// A [`crossbeam_channel::Sender`] that implements [`Producer`].
#[derive(Debug)]
pub struct CrossbeamProducer<G> {
    /// The sender.
    tx: crossbeam_channel::Sender<G>,
}

impl<G> Producer for CrossbeamProducer<G> {
    type Good = G;
    type Failure = ProduceFailure<DisconnectedFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        self.tx.try_send(good)?
    }
}

impl<G> From<crossbeam_channel::Sender<G>> for CrossbeamProducer<G> {
    #[inline]
    fn from(tx: crossbeam_channel::Sender<G>) -> Self {
        Self { tx }
    }
}
