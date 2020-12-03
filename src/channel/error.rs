//! The errors related to channels.
#[cfg(doc)]
use crate::{Consumer, Producer};

use {
    crate::{ConsumeFault, ConsumeFailure, ProduceFailure, ProduceFault},
    std::sync::mpsc::SendError,
};

/// A fault thrown when attempting to produce to a channel with no [`Consumer`]s.
#[derive(Clone, ProduceFault, Copy, Debug, thiserror::Error)]
#[error("demand is gone")]
pub struct WithdrawnDemand;

/// A fault thrown when attempting to consume from a channel with an empty stock and no [`Producer`]s.
#[derive(Clone, ConsumeFault, Copy, Debug, thiserror::Error)]
#[error("supply is gone")]
pub struct WithdrawnSupply;

impl From<std::sync::mpsc::TryRecvError> for ConsumeFailure<WithdrawnSupply> {
    #[inline]
    fn from(error: std::sync::mpsc::TryRecvError) -> Self {
        match error {
            std::sync::mpsc::TryRecvError::Empty => Self::EmptyStock,
            std::sync::mpsc::TryRecvError::Disconnected => Self::Fault(WithdrawnSupply),
        }
    }
}

impl<G> From<SendError<G>> for ProduceFailure<WithdrawnDemand> {
    #[inline]
    fn from(_: SendError<G>) -> Self {
        Self::Fault(WithdrawnDemand)
    }
}

impl<G> From<std::sync::mpsc::TrySendError<G>> for ProduceFailure<WithdrawnDemand> {
    #[inline]
    fn from(error: std::sync::mpsc::TrySendError<G>) -> Self {
        match error {
            std::sync::mpsc::TrySendError::Full(_) => Self::FullStock,
            std::sync::mpsc::TrySendError::Disconnected(_) => Self::Fault(WithdrawnDemand),
        }
    }
}

impl From<crossbeam_channel::TryRecvError> for ConsumeFailure<WithdrawnSupply> {
    #[inline]
    fn from(error: crossbeam_channel::TryRecvError) -> Self {
        match error {
            crossbeam_channel::TryRecvError::Empty => Self::EmptyStock,
            crossbeam_channel::TryRecvError::Disconnected => Self::Fault(WithdrawnSupply),
        }
    }
}

impl<G> From<crossbeam_channel::TrySendError<G>> for ProduceFailure<WithdrawnDemand> {
    #[inline]
    fn from(error: crossbeam_channel::TrySendError<G>) -> Self {
        match error {
            crossbeam_channel::TrySendError::Full(_) => Self::FullStock,
            crossbeam_channel::TrySendError::Disconnected(_) => Self::Fault(WithdrawnDemand),
        }
    }
}
