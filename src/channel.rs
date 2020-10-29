//! Implements [`Producer`] and [`Consumer`] for various types of channels.
use {
    core::fmt::Debug,
    fehler::throws,
    std::sync::mpsc::{Receiver, Sender, TryRecvError, SendError},
};

/// A fault caused by the other side of the channel being dropped.
#[derive(Debug, thiserror::Error)]
#[error("channel is closed")]
pub struct ClosedChannelFault;

try_from_consumer_failure!(ClosedChannelFault);
try_from_producer_failure!(ClosedChannelFault);

impl From<TryRecvError> for crate::ConsumerFailure<ClosedChannelFault> {
    #[inline]
    fn from(error: TryRecvError) -> Self {
        match error {
            TryRecvError::Empty => Self::EmptyStock,
            TryRecvError::Disconnected => Self::Fault(ClosedChannelFault),
        }
    }
}

impl<G> From<SendError<G>> for crate::ProducerFailure<ClosedChannelFault> {
    #[inline]
    fn from(_: SendError<G>) -> Self {
        Self::Fault(ClosedChannelFault)
    }
}

impl From<crossbeam_channel::TryRecvError> for crate::ConsumerFailure<ClosedChannelFault> {
    #[inline]
    fn from(error: crossbeam_channel::TryRecvError) -> Self {
        match error {
            crossbeam_channel::TryRecvError::Empty => Self::EmptyStock,
            crossbeam_channel::TryRecvError::Disconnected => Self::Fault(ClosedChannelFault),
        }
    }
}

impl<G> From<crossbeam_channel::TrySendError<G>> for crate::ProducerFailure<ClosedChannelFault> {
    #[inline]
    fn from(error: crossbeam_channel::TrySendError<G>) -> Self {
        match error {
            crossbeam_channel::TrySendError::Full(_) => Self::FullStock,
            crossbeam_channel::TrySendError::Disconnected(_) => Self::Fault(ClosedChannelFault),
        }
    }
}

/// A [`std::sync::mpsc::Receiver`] that implements [`Consumer<Good = G>`].
#[derive(Debug)]
pub struct StdConsumer<G> {
    /// The receiver.
    rx: Receiver<G>,
}

impl<G> crate::Consumer for StdConsumer<G>
{
    type Good = G;
    type Failure = crate::ConsumerFailure<ClosedChannelFault>;

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

impl<G> crate::Producer for StdProducer<G>
{
    type Good = G;
    type Failure = crate::ProducerFailure<ClosedChannelFault>;

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
pub struct CrossbeamConsumer<G>
{
    /// The receiver.
    rx: crossbeam_channel::Receiver<G>,
}

impl<G> crate::Consumer for CrossbeamConsumer<G>
{
    type Good = G;
    type Failure = crate::ConsumerFailure<ClosedChannelFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        self.rx.try_recv()?
    }
}

impl<G> From<crossbeam_channel::Receiver<G>> for CrossbeamConsumer<G>
{
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

impl<G> crate::Producer for CrossbeamProducer<G>
{
    type Good = G;
    type Failure = crate::ProducerFailure<ClosedChannelFault>;

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
