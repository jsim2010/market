//! Implements errors thrown by `market`.
#![allow(clippy::module_name_repetitions)]
use {
    core::fmt::{Debug, Display},
    fehler::{throw, throws},
    std::error::Error,
    thiserror::Error as ThisError,
};

/// An error consuming from a market.
#[derive(Debug, Eq, Hash, PartialEq, ThisError)]
pub enum ConsumeError<F>
where
    F: Error,
{
    /// The stock of the market is empty.
    #[error("stock is empty")]
    EmptyStock,
    /// A failure occurred.
    ///
    /// Indicates the [`Consumer`] will not consume any more goods in its current state.
    #[error("failure: {0}")]
    Failure(F),
}

#[allow(clippy::use_self)] // False positive for ConsumeError<E>.
impl<F> ConsumeError<F>
where
    F: Error,
{
    /// Converts `ConsumeError<F>` into `ConsumeError<E>`.
    #[inline]
    pub fn map_into<E>(self) -> ConsumeError<E>
    where
        E: Error + From<F>,
    {
        match self {
            Self::EmptyStock => ConsumeError::EmptyStock,
            Self::Failure(failure) => ConsumeError::Failure(failure.into()),
        }
    }
}

impl<F> From<F> for ConsumeError<F>
where
    F: Error,
{
    #[inline]
    fn from(value: F) -> Self {
        Self::Failure(value)
    }
}

/// An error producing to a market.
#[derive(Debug, Eq, Hash, PartialEq, ThisError)]
pub enum ProduceError<F: Error> {
    /// The stock of the market is full.
    #[error("stock is full")]
    FullStock,
    /// A failure to produce a good.
    #[error("failure: {0}")]
    Failure(F),
}

#[allow(clippy::use_self)] // False positive for ProduceError<E>.
impl<F> ProduceError<F>
where
    F: Error,
{
    /// Converts `self` into `ProduceError<E>`
    #[inline]
    pub fn map_into<E>(self) -> ProduceError<E>
    where
        E: Error + From<F>,
    {
        match self {
            Self::FullStock => ProduceError::FullStock,
            Self::Failure(failure) => ProduceError::Failure(failure.into()),
        }
    }
}

impl<F> From<F> for ProduceError<F>
where
    F: Error,
{
    #[inline]
    fn from(value: F) -> Self {
        Self::Failure(value)
    }
}

/// An error producing to a market that returns the failed good.
#[derive(Debug, Eq, Hash, PartialEq, ThisError)]
#[error("unable to produce good `{good}`: {error}")]
pub struct Recall<G, F>
where
    G: Debug + Display,
    F: Error,
{
    /// The good that was not produced.
    good: G,
    /// The error.
    error: ProduceError<F>,
}

impl<G, F> Recall<G, F>
where
    G: Debug + Display,
    F: Error,
{
    /// Creates a new [`Recall`].
    #[inline]
    pub fn new(good: G, error: ProduceError<F>) -> Self {
        Self { good, error }
    }

    /// Returns recalled good if recall was due to a full stock; otherwise throws failure.
    #[inline]
    #[throws(F)]
    pub fn return_good_if_full(self) -> G {
        match self.error {
            ProduceError::FullStock => self.good,
            ProduceError::Failure(failure) => {
                throw!(failure);
            }
        }
    }
}

/// A failure consuming a good due to the market being closed.
#[derive(Clone, Copy, Debug, ThisError)]
#[error("market is closed")]
pub struct ClosedMarketFailure;
