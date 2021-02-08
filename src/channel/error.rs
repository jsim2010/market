//! Implements the errors related to channels.
#[cfg(doc)]
use {
    crate::{Consumer, Producer},
    std::{convert::TryFrom, error::Error, fmt::Display},
};

use crate::{ConsumeFault, ProduceFault};

/// A fault thrown when attempting to produce to a channel with no [`Consumer`]s.
///
/// Implements [`Error`]:
///     - [`Error::source()`] returns [`None`].
///     - [`Display::fmt()`] writes "demand of `{description}` has withdrawn".
/// Implements [`TryFrom<ProduceFailure<Self>>`]:
///     - [`TryFrom::try_from()`] SHALL follow specifications given at [`ProduceFault`].
#[derive(Clone, Debug, ProduceFault, thiserror::Error)]
#[error("demand of `{description}` has withdrawn")]
pub struct WithdrawnDemandFault {
    /// Describes the channel on which fault occurred.
    description: String,
}

impl WithdrawnDemandFault {
    /// Creates a new [`WithdrawnDemandFault`].
    #[inline]
    #[must_use]
    pub const fn new(description: String) -> Self {
        Self { description }
    }
}

/// A fault thrown when attempting to consume from a channel with an empty stock and no [`Producer`]s.
///
/// Implements [`Error`]:
///     - [`Error::source()`] returns [`None`].
///     - [`Display::fmt()`] writes "supply of `{description}` has withdrawn".
/// Implements [`TryFrom<ConsumeFailure<Self>>`]:
///     - [`TryFrom::try_from()`] SHALL follow specifications given at [`ConsumeFault`].
#[derive(Clone, ConsumeFault, Debug, thiserror::Error)]
#[error("supply of `{description}` has withdrawn")]
pub struct WithdrawnSupplyFault {
    /// Describes the channel on which fault occurred.
    description: String,
}

impl WithdrawnSupplyFault {
    /// Creates a new [`WithdrawnSupplyFault`].
    #[inline]
    #[must_use]
    pub const fn new(description: String) -> Self {
        Self { description }
    }
}
