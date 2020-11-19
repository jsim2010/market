//! The errors related to channels.

/// A fault caused by the other side of the channel being dropped.
#[derive(Clone, crate::ConsumeFault, crate::ProduceFault, Copy, Debug, thiserror::Error)]
#[error("channel is disconnected")]
pub struct DisconnectedFault;

impl From<std::sync::mpsc::TryRecvError> for crate::ConsumeFailure<DisconnectedFault> {
    #[inline]
    fn from(error: std::sync::mpsc::TryRecvError) -> Self {
        match error {
            std::sync::mpsc::TryRecvError::Empty => Self::EmptyStock,
            std::sync::mpsc::TryRecvError::Disconnected => Self::Fault(DisconnectedFault),
        }
    }
}

impl<G> From<std::sync::mpsc::SendError<G>> for crate::ProduceFailure<DisconnectedFault> {
    #[inline]
    fn from(_: std::sync::mpsc::SendError<G>) -> Self {
        Self::Fault(DisconnectedFault)
    }
}

impl From<crossbeam_channel::TryRecvError> for crate::ConsumeFailure<DisconnectedFault> {
    #[inline]
    fn from(error: crossbeam_channel::TryRecvError) -> Self {
        match error {
            crossbeam_channel::TryRecvError::Empty => Self::EmptyStock,
            crossbeam_channel::TryRecvError::Disconnected => Self::Fault(DisconnectedFault),
        }
    }
}

impl<G> From<crossbeam_channel::TrySendError<G>> for crate::ProduceFailure<DisconnectedFault> {
    #[inline]
    fn from(error: crossbeam_channel::TrySendError<G>) -> Self {
        match error {
            crossbeam_channel::TrySendError::Full(_) => Self::FullStock,
            crossbeam_channel::TrySendError::Disconnected(_) => Self::Fault(DisconnectedFault),
        }
    }
}
