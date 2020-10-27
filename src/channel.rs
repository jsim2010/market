//! Implements `Consumer` and `Producer` for various types of channels.
use {
    crate::{ClosedMarketFault, ClassicalConsumerFailure, Consumer, ClassicalProducerFailure, Producer},
    core::fmt::Debug,
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
    type Failure = ClassicalConsumerFailure<ClosedMarketFault>;

    #[inline]
    #[throws(Self::Failure)]
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

impl From<mpsc::TryRecvError> for ClassicalConsumerFailure<ClosedMarketFault> {
    #[inline]
    fn from(value: mpsc::TryRecvError) -> Self {
        match value {
            mpsc::TryRecvError::Empty => Self::EmptyStock,
            mpsc::TryRecvError::Disconnected => Self::Fault(ClosedMarketFault),
        }
    }
}

/// A [`std::sync::mpsc::Sender`] that implements [`Producer`].
///
/// [`std::sync::mpsc::Sender`]: https:://doc.rust-lang.org/std/sync/mpsc/struct.Sender.html
/// [`Producer`]: ../trait.Producer.html
#[derive(Debug)]
pub struct StdProducer<G> {
    /// The sender.
    tx: mpsc::Sender<G>,
}

impl<G> Producer for StdProducer<G>
where
    G: Debug,
{
    type Good = G;
    type Failure = ClassicalProducerFailure<ClosedMarketFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        self.tx.send(good)?
    }
}

impl<G> From<mpsc::Sender<G>> for StdProducer<G> {
    #[inline]
    fn from(value: mpsc::Sender<G>) -> Self {
        Self { tx: value }
    }
}

impl<G> From<mpsc::SendError<G>> for ClassicalProducerFailure<ClosedMarketFault> {
    #[inline]
    fn from(_value: mpsc::SendError<G>) -> Self {
        Self::Fault(ClosedMarketFault)
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
    type Failure = ClassicalConsumerFailure<ClosedMarketFault>;

    #[inline]
    #[throws(Self::Failure)]
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

impl From<crossbeam_channel::TryRecvError> for ClassicalConsumerFailure<ClosedMarketFault> {
    #[inline]
    fn from(value: crossbeam_channel::TryRecvError) -> Self {
        match value {
            crossbeam_channel::TryRecvError::Empty => Self::EmptyStock,
            crossbeam_channel::TryRecvError::Disconnected => Self::Fault(ClosedMarketFault),
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
    G: Debug,
{
    type Good = G;
    type Failure = ClassicalProducerFailure<ClosedMarketFault>;

    #[inline]
    #[throws(Self::Failure)]
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

impl<G> From<crossbeam_channel::TrySendError<G>> for ClassicalProducerFailure<ClosedMarketFault> {
    #[inline]
    fn from(value: crossbeam_channel::TrySendError<G>) -> Self {
        match value {
            crossbeam_channel::TrySendError::Full(_) => Self::FullStock,
            crossbeam_channel::TrySendError::Disconnected(_) => Self::Fault(ClosedMarketFault),
        }
    }
}
