//! Implements [`Producer`] and [`Consumer`] for various types of channels.
use {
    crate::{
        ConsumeFailure, ConsumeFault, Consumer, Participant, ProduceFailure, ProduceFault,
        Producer, TakenParticipant,
    },
    core::fmt::Debug,
    fehler::throws,
    std::sync::mpsc::{Receiver, SendError, Sender, TryRecvError},
};

/// The size of the stock for a channel.
#[derive(Clone, Copy, Debug)]
pub enum Size {
    /// Infinite size.
    Infinite,
    /// Finite size.
    Finite(usize),
}

// TODO: Instead of having a struct for each channel kind, should have a generic struct that can create any kind of channel.
/// Represents a [`crossbeam`] channel.
#[derive(Debug)]
pub struct Crossbeam<G> {
    /// The [`Producer`] of the channel.
    producer: Option<CrossbeamProducer<G>>,
    /// The [`Consumer`] of the channel.
    consumer: Option<CrossbeamConsumer<G>>,
}

impl<G> Crossbeam<G> {
    /// Creates a new [`crossbeam`] channel with a stock size equal to `size`.
    #[inline]
    #[must_use]
    pub fn new(size: Size) -> Self {
        Self::from(match size {
            Size::Infinite => crossbeam_channel::unbounded(),
            Size::Finite(value) => crossbeam_channel::bounded(value),
        })
    }

    /// Takes the [`Producer`] from `self`.
    ///
    /// If the producer has already been taken, throws [`TakenParticipant`].
    #[inline]
    #[throws(TakenParticipant)]
    pub fn producer(&mut self) -> CrossbeamProducer<G> {
        self.producer
            .take()
            .ok_or(TakenParticipant(Participant::Producer))?
    }

    /// Takes the [`Consumer`] from `self`.
    ///
    /// If the consumer has already been taken, throws [`TakenParticipant`].
    #[inline]
    #[throws(TakenParticipant)]
    pub fn consumer(&mut self) -> CrossbeamConsumer<G> {
        self.consumer
            .take()
            .ok_or(TakenParticipant(Participant::Consumer))?
    }
}

impl<G> From<(crossbeam_channel::Sender<G>, crossbeam_channel::Receiver<G>)> for Crossbeam<G> {
    #[inline]
    fn from(participants: (crossbeam_channel::Sender<G>, crossbeam_channel::Receiver<G>)) -> Self {
        Self {
            producer: Some(participants.0.into()),
            consumer: Some(participants.1.into()),
        }
    }
}

/// A fault caused by the other side of the channel being dropped.
#[derive(Clone, ConsumeFault, ProduceFault, Copy, Debug, thiserror::Error)]
#[error("channel is disconnected")]
pub struct DisconnectedFault;

impl From<TryRecvError> for ConsumeFailure<DisconnectedFault> {
    #[inline]
    fn from(error: TryRecvError) -> Self {
        match error {
            TryRecvError::Empty => Self::EmptyStock,
            TryRecvError::Disconnected => Self::Fault(DisconnectedFault),
        }
    }
}

impl<G> From<SendError<G>> for ProduceFailure<DisconnectedFault> {
    #[inline]
    fn from(_: SendError<G>) -> Self {
        Self::Fault(DisconnectedFault)
    }
}

impl From<crossbeam_channel::TryRecvError> for ConsumeFailure<DisconnectedFault> {
    #[inline]
    fn from(error: crossbeam_channel::TryRecvError) -> Self {
        match error {
            crossbeam_channel::TryRecvError::Empty => Self::EmptyStock,
            crossbeam_channel::TryRecvError::Disconnected => Self::Fault(DisconnectedFault),
        }
    }
}

impl<G> From<crossbeam_channel::TrySendError<G>> for ProduceFailure<DisconnectedFault> {
    #[inline]
    fn from(error: crossbeam_channel::TrySendError<G>) -> Self {
        match error {
            crossbeam_channel::TrySendError::Full(_) => Self::FullStock,
            crossbeam_channel::TrySendError::Disconnected(_) => Self::Fault(DisconnectedFault),
        }
    }
}

/// A [`std::sync::mpsc::Receiver`] that implements [`Consumer<Good = G>`].
#[derive(Debug)]
pub struct StdConsumer<G> {
    /// The receiver.
    rx: Receiver<G>,
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

impl<G> From<Receiver<G>> for StdConsumer<G> {
    #[inline]
    fn from(rx: Receiver<G>) -> Self {
        Self { rx }
    }
}

/// A [`std::sync::mpsc::Sender`] that implements [`Producer`].
#[derive(Debug)]
pub struct StdProducer<G> {
    /// The sender.
    tx: Sender<G>,
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

impl<G> From<Sender<G>> for StdProducer<G> {
    #[inline]
    fn from(tx: Sender<G>) -> Self {
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
