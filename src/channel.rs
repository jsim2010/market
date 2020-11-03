//! Implements [`Producer`] and [`Consumer`] for various types of channels.
use {
    crate::{ConsumeFailure, Consumer, ProduceFailure, Producer},
    core::fmt::Debug,
    fehler::throws,
    std::sync::mpsc::{Receiver, SendError, Sender, TryRecvError},
};

/// A fault caused by the other side of the channel being dropped.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("channel is disconnected")]
pub struct DisconnectedFault;

consumer_fault!(DisconnectedFault);
producer_fault!(DisconnectedFault);

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
