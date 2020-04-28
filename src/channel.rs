//! [`Consumer`] and [`Producer`] implementations for different channel implementations.
use {
    crate::{ClosedMarketFailure, ConsumeError, Consumer, ProduceGoodError, Producer},
    core::fmt::Debug,
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

/// A [`crossbeam_channel::Sender`] that implements [`Producer`].
#[derive(Debug)]
pub struct CrossbeamProducer<G> {
    /// The sender.
    tx: crossbeam_channel::Sender<G>,
}

impl<G> Producer for CrossbeamProducer<G> {
    type Good = G;
    type Failure = ClosedMarketFailure;

    #[inline]
    #[throws(ProduceGoodError<Self::Good, Self::Failure>)]
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
