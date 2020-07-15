//! [`Consumer`] and [`Producer`] implementations for different channel implementations.
use {
    crate::{ClosedMarketFailure, ConsumeError, Consumer, ProduceError, Producer},
    core::fmt::{Debug, Display},
    fehler::throws,
    std::sync::mpsc,
};

/// A [`mpsc::Receiver`] that implements [`Consumer`].
#[derive(Debug)]
pub struct StdConsumer<G> {
    /// The receiver.
    rx: mpsc::Receiver<G>,
}

impl<G> Consumer for StdConsumer<G>
where
    G: Debug,
{
    type Good = G;
    type Failure = ClosedMarketFailure;

    #[inline]
    #[throws(ConsumeError<Self::Failure>)]
    fn consume(&self) -> Self::Good {
        self.rx.try_recv()?
    }
}

impl<G> From<mpsc::Receiver<G>> for StdConsumer<G> {
    #[inline]
    fn from(value: mpsc::Receiver<G>) -> Self {
        Self { rx: value }
    }
}

impl From<mpsc::TryRecvError> for ConsumeError<ClosedMarketFailure> {
    #[inline]
    fn from(value: mpsc::TryRecvError) -> Self {
        match value {
            mpsc::TryRecvError::Empty => Self::EmptyStock,
            mpsc::TryRecvError::Disconnected => Self::Failure(ClosedMarketFailure),
        }
    }
}

/// A [`crossbeam_channel::Receiver`] that implements [`Consumer`].
#[derive(Debug)]
pub struct CrossbeamConsumer<G>
where
    G: Debug,
{
    /// The [`crossbeam_channel::Recevier`].
    rx: crossbeam_channel::Receiver<G>,
}

impl<G> Consumer for CrossbeamConsumer<G>
where
    G: Debug,
{
    type Good = G;
    type Failure = ClosedMarketFailure;

    #[inline]
    #[throws(ConsumeError<Self::Failure>)]
    fn consume(&self) -> Self::Good {
        self.rx.try_recv()?
    }
}

impl<G> From<crossbeam_channel::Receiver<G>> for CrossbeamConsumer<G>
where
    G: Debug,
{
    #[inline]
    fn from(value: crossbeam_channel::Receiver<G>) -> Self {
        Self { rx: value }
    }
}

impl From<crossbeam_channel::TryRecvError> for ConsumeError<ClosedMarketFailure> {
    #[inline]
    fn from(value: crossbeam_channel::TryRecvError) -> Self {
        match value {
            crossbeam_channel::TryRecvError::Empty => Self::EmptyStock,
            crossbeam_channel::TryRecvError::Disconnected => Self::Failure(ClosedMarketFailure),
        }
    }
}

/// A [`crossbeam_channel::Sender`] that implements [`Producer`].
#[derive(Debug)]
pub struct CrossbeamProducer<G> {
    /// The sender.
    tx: crossbeam_channel::Sender<G>,
}

impl<G> Producer for CrossbeamProducer<G>
where
    G: Debug + Display,
{
    type Good = G;
    type Failure = ClosedMarketFailure;

    #[inline]
    #[throws(ProduceError<Self::Failure>)]
    fn produce(&self, good: Self::Good) {
        self.tx.try_send(good)?
    }
}

impl<G> From<crossbeam_channel::Sender<G>> for CrossbeamProducer<G> {
    #[inline]
    fn from(value: crossbeam_channel::Sender<G>) -> Self {
        Self { tx: value }
    }
}

impl<G> From<crossbeam_channel::TrySendError<G>> for ProduceError<ClosedMarketFailure> {
    #[inline]
    fn from(value: crossbeam_channel::TrySendError<G>) -> Self {
        match value {
            crossbeam_channel::TrySendError::Full(_) => Self::FullStock,
            crossbeam_channel::TrySendError::Disconnected(_) => Self::Failure(ClosedMarketFailure),
        }
    }
}
