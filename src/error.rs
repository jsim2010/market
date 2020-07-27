//! Implements errors thrown by `market`.
#![allow(clippy::module_name_repetitions)] // Module is not public so public items are re-exported and thus their use does not have any repetition.
use {
    core::fmt::{Debug, Display},
    fehler::{throw, throws},
    std::error::Error,
    thiserror::Error as ThisError,
};

/// The `Consumer` failed to consume a good.
#[derive(Debug, Eq, Hash, PartialEq, ThisError)]
pub enum ConsumeFailure<E>
where
    E: Error,
{
    /// The stock of the market is empty.
    #[error("stock is empty")]
    EmptyStock,
    /// An error was thrown during consumption.
    ///
    /// Indicates the [`Consumer`] will not consume any more goods in its current state.
    // Using #[error(transparent)] here would require adding explicit lifetime bounds to E.
    #[error("{0}")]
    Error(E),
}

#[allow(clippy::use_self)] // False positive for ConsumeFailure<E>.
impl<E> ConsumeFailure<E>
where
    E: Error,
{
    /// Converts `ConsumeFailure<E>` into `ConsumeFailure<F>`.
    #[inline]
    pub fn map_into<F>(self) -> ConsumeFailure<F>
    where
        F: Error + From<E>,
    {
        match self {
            Self::EmptyStock => ConsumeFailure::EmptyStock,
            Self::Error(error) => ConsumeFailure::Error(error.into()),
        }
    }
}

impl<E> From<E> for ConsumeFailure<E>
where
    E: Error,
{
    #[inline]
    fn from(value: E) -> Self {
        Self::Error(value)
    }
}

/// The `Producer` failed to produce a good.
#[derive(Debug, Eq, Hash, PartialEq, ThisError)]
pub enum ProduceFailure<E>
where
    E: Error,
{
    /// The stock of the market is full.
    #[error("stock is full")]
    FullStock,
    /// An error was thrown during production.
    ///
    /// Indicates the [`Producer`] will not produce any more goods in its current state.
    // Using #[error(transparent)] here would require adding explicit lifetime bounds to E.
    #[error("{0}")]
    Error(E),
}

#[allow(clippy::use_self)] // False positive for ProduceFailure<E>.
impl<E> ProduceFailure<E>
where
    E: Error,
{
    /// Converts `self` into `ProduceFailure<F>`
    #[inline]
    pub fn map_into<F>(self) -> ProduceFailure<F>
    where
        F: Error + From<E>,
    {
        match self {
            Self::FullStock => ProduceFailure::FullStock,
            Self::Error(failure) => ProduceFailure::Error(failure.into()),
        }
    }
}

impl<E> From<E> for ProduceFailure<E>
where
    E: Error,
{
    #[inline]
    fn from(value: E) -> Self {
        Self::Error(value)
    }
}

/// Returns a good that a `Producer` failed to produce.
#[derive(Debug, Eq, Hash, PartialEq, ThisError)]
#[error("failed to produce good `{good}`: {failure}")]
pub struct Recall<G, E>
where
    G: Debug + Display,
    E: Error,
{
    /// The good that was not produced.
    good: G,
    /// The reason the production failed.
    failure: ProduceFailure<E>,
}

impl<G, E> Recall<G, E>
where
    G: Debug + Display,
    E: Error,
{
    /// Creates a new [`Recall`].
    #[inline]
    pub fn new(good: G, failure: ProduceFailure<E>) -> Self {
        Self { good, failure }
    }

    /// Returns the recalled good if recall was due to a full stock; otherwise throws failure.
    #[inline]
    #[throws(E)]
    pub fn overstock(self) -> G {
        match self.failure {
            ProduceFailure::FullStock => self.good,
            ProduceFailure::Error(failure) => {
                throw!(failure);
            }
        }
    }
}

/// An error interacting with a market due to the market being closed.
#[derive(Clone, Copy, Debug, ThisError)]
#[error("market is closed")]
pub struct ClosedMarketError;
