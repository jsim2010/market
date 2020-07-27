//! Implements `Consumer` and `Producer` for various types of channels.
use {
    crate::{ClosedMarketError, ConsumeFailure, Consumer, ProduceFailure, Producer},
    core::fmt::{Debug, Display},
    fehler::throws,
    std::sync::mpsc,
};

/// A [`std::sync::mpsc::Receiver`] that implements [`Consumer`].
///
/// [`std::sync::mpsc::Receiver`]: https:://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html
/// [`Consumer`]: ../trait.Consumer.html
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
    type Error = ClosedMarketError;

    #[inline]
    #[throws(ConsumeFailure<Self::Error>)]
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

impl From<mpsc::TryRecvError> for ConsumeFailure<ClosedMarketError> {
    #[inline]
    fn from(value: mpsc::TryRecvError) -> Self {
        match value {
            mpsc::TryRecvError::Empty => Self::EmptyStock,
            mpsc::TryRecvError::Disconnected => Self::Error(ClosedMarketError),
        }
    }
}

/// A `crossbeam_channel::Receiver` that implements [`Consumer`].
///
/// [`Consumer`]: ../trait.Consumer.html
#[derive(Debug)]
pub struct CrossbeamConsumer<G>
where
    G: Debug,
{
    /// The receiver.
    rx: crossbeam_channel::Receiver<G>,
}

impl<G> Consumer for CrossbeamConsumer<G>
where
    G: Debug,
{
    type Good = G;
    type Error = ClosedMarketError;

    #[inline]
    #[throws(ConsumeFailure<Self::Error>)]
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

impl From<crossbeam_channel::TryRecvError> for ConsumeFailure<ClosedMarketError> {
    #[inline]
    fn from(value: crossbeam_channel::TryRecvError) -> Self {
        match value {
            crossbeam_channel::TryRecvError::Empty => Self::EmptyStock,
            crossbeam_channel::TryRecvError::Disconnected => Self::Error(ClosedMarketError),
        }
    }
}

/// A `crossbeam_channel::Sender` that implements [`Producer`].
///
/// [`Producer`]: ../trait.Producer.html
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
    type Error = ClosedMarketError;

    #[inline]
    #[throws(ProduceFailure<Self::Error>)]
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

impl<G> From<crossbeam_channel::TrySendError<G>> for ProduceFailure<ClosedMarketError> {
    #[inline]
    fn from(value: crossbeam_channel::TrySendError<G>) -> Self {
        match value {
            crossbeam_channel::TrySendError::Full(_) => Self::FullStock,
            crossbeam_channel::TrySendError::Disconnected(_) => Self::Error(ClosedMarketError),
        }
    }
}
