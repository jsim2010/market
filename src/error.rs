//! Implements errors thrown by `market`.
#![allow(clippy::module_name_repetitions)] // Module is not public so public items are re-exported and thus their use does not have any repetition.
use {
    conventus::AssembleFailure,
    core::fmt::{Debug, Display},
    fehler::{throw, throws},
    std::error::Error,
    thiserror::Error as ThisError,
};

/// The `Consumer` failed to consume a good.
#[derive(Debug, Eq, Hash, PartialEq, ThisError)]
pub enum ConsumeFailure<E>
where
    E: Error + 'static,
{
    /// The stock of the market is empty.
    #[error("stock is empty")]
    EmptyStock,
    /// An error was thrown during consumption.
    ///
    /// Indicates the [`Consumer`] will not consume any more goods in its current state.
    #[error(transparent)]
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

// TODO: Ideally could do From<AssembleFailure<E>> for ConsumeFailure<F> where F: From<E>. However this is overriden by From<E> for ConsumeFailure<E> since there is no way to indicate F != AssembleFailure<E>.
impl<E> From<AssembleFailure<E>> for ConsumeFailure<E>
where
    E: Error,
{
    #[inline]
    fn from(value: AssembleFailure<E>) -> Self {
        match value {
            AssembleFailure::Incomplete => Self::EmptyStock,
            AssembleFailure::Error(error) => Self::Error(error),
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
    E: Error + 'static,
{
    /// The stock of the market is full.
    #[error("stock is full")]
    FullStock,
    /// An error was thrown during production.
    ///
    /// Indicates the [`Producer`] will not produce any more goods in its current state.
    #[error(transparent)]
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
    E: Error + 'static,
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

/// Producer failed to strip and produce good.
#[derive(Debug, ThisError)]
pub enum ProducePartsError<S, P>
where
    S: Error + 'static,
    P: Error + 'static,
{
    /// Producer failed to strip good.
    // For now, unable to mark both as #[from] due to inability to indicate S != P. Chose Produce as from to ease use with map_into.
    #[error("cannot strip: {0}")]
    Strip(#[source] S),
    /// Producer failed to produce good.
    #[error("cannot produce: {0}")]
    Produce(#[from] P),
}

/// A consumer failed to compose and consume a good.
#[derive(Debug, ThisError)]
pub enum ConsumeCompositeError<M, N>
where
    M: Error + 'static,
    N: Error + 'static,
{
    /// Consumer failed to compose good.
    // For now, unable to mark both as #[from] due to inability to indicate M != N. Chose Consume as from to ease use with map_into.
    #[error("cannot compose: {0}")]
    Compose(#[source] M),
    /// Consumer failed to consume good.
    #[error("cannot consume: {0}")]
    Consume(#[from] N),
}
