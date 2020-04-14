//! [`Consumer`] and [`Producer`] implementations for different channel implementations.
use {
    crate::{ClosedMarketError, Consumer, Producer},
    fehler::{throw, throws},
    std::sync::mpsc,
};

/// A [`mpsc::Receiver`] that implements [`Consumer`].
#[derive(Debug)]
pub struct MpscConsumer<G> {
    /// The receiver.
    rx: mpsc::Receiver<G>,
}

impl<G> Consumer for MpscConsumer<G> {
    type Good = G;
    type Error = ClosedMarketError;

    #[inline]
    #[throws(Self::Error)]
    fn consume(&self) -> Option<Self::Good> {
        match self.rx.try_recv() {
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => throw!(ClosedMarketError),
            Ok(good) => Some(good),
        }
    }
}

impl<G> From<mpsc::Receiver<G>> for MpscConsumer<G> {
    #[inline]
    fn from(value: mpsc::Receiver<G>) -> Self {
        Self { rx: value }
    }
}

/// A [`crossbeam_channel::Receiver`] that implements [`Consumer`].
#[derive(Debug)]
pub struct CrossbeamConsumer<G> {
    /// The [`crossbeam_channel::Recevier`].
    rx: crossbeam_channel::Receiver<G>,
}

impl<G> Consumer for CrossbeamConsumer<G> {
    type Good = G;
    type Error = ClosedMarketError;

    #[inline]
    #[throws(Self::Error)]
    fn consume(&self) -> Option<Self::Good> {
        match self.rx.try_recv() {
            Err(crossbeam_channel::TryRecvError::Empty) => None,
            Err(crossbeam_channel::TryRecvError::Disconnected) => throw!(ClosedMarketError),
            Ok(good) => Some(good),
        }
    }
}

impl<G> From<crossbeam_channel::Receiver<G>> for CrossbeamConsumer<G> {
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
    type Error = ClosedMarketError;

    #[inline]
    #[throws(Self::Error)]
    fn produce(&self, good: Self::Good) -> Option<Self::Good> {
        match self.tx.try_send(good) {
            Err(crossbeam_channel::TrySendError::Full(g)) => Some(g),
            Err(crossbeam_channel::TrySendError::Disconnected(..)) => throw!(ClosedMarketError),
            Ok(()) => None,
        }
    }
}

impl<G> From<crossbeam_channel::Sender<G>> for CrossbeamProducer<G> {
    #[inline]
    fn from(value: crossbeam_channel::Sender<G>) -> Self {
        Self { tx: value }
    }
}
