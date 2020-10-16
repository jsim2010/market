//! Implements errors thrown by `market`.
//!
//! There are 2 categories of errors thrown by `market`.
//! 1) Failures: These indicate that an action was not successful, but the state of the market is still valid.
//! 2) Faults: These indicate that the market is currently in a state that no attempted action will be successful until the state is changed (if possible).
use {
    conventus::AssembleFailure,
    core::fmt::Debug,
    fehler::{throw, throws},
    std::error::Error,
    thiserror::Error as ThisError,
};

/// A `Consumer` failed to consume a good.
#[derive(Debug, Eq, Hash, PartialEq, ThisError)]
pub enum ConsumeFailure<T>
where
    T: Error + 'static,
{
    /// The stock of the market is empty.
    #[error("stock is empty")]
    EmptyStock,
    /// A fault was thrown during consumption.
    ///
    /// Indicates the [`Consumer`] is currently in a state where it will not consume any more goods.
    #[error(transparent)]
    Fault(T),
}

#[allow(clippy::use_self)] // False positive for ConsumeFailure<T>.
impl<T> ConsumeFailure<T>
where
    T: Error,
{
    // This is done because From<ConsumeFailure<T>> for ConsumeFailure<U> cannot.
    /// Converts `ConsumeFailure<T>` into `ConsumeFailure<U>`.
    #[inline]
    pub fn map_into<U>(self) -> ConsumeFailure<U>
    where
        U: Error + From<T>,
    {
        match self {
            Self::EmptyStock => ConsumeFailure::EmptyStock,
            Self::Fault(fault) => ConsumeFailure::Fault(fault.into()),
        }
    }
}

// TODO: Ideally could do From<AssembleFailure<E>> for ConsumeFailure<F> where F: From<E>. However this is overriden by From<E> for ConsumeFailure<E> since there is no way to indicate F != AssembleFailure<E>.
impl<T> From<AssembleFailure<T>> for ConsumeFailure<T>
where
    T: Error,
{
    #[inline]
    fn from(failure: AssembleFailure<T>) -> Self {
        match failure {
            AssembleFailure::Incomplete => Self::EmptyStock,
            AssembleFailure::Error(error) => Self::Fault(error),
        }
    }
}

impl<T> From<T> for ConsumeFailure<T>
where
    T: Error,
{
    #[inline]
    fn from(fault: T) -> Self {
        Self::Fault(fault)
    }
}

/// A `Producer` failed to produce a good.
#[derive(Debug, Eq, Hash, PartialEq, ThisError)]
pub enum ProduceFailure<T>
where
    T: Error + 'static,
{
    /// The stock of the market is full.
    #[error("stock is full")]
    FullStock,
    /// A fault was thrown during production.
    ///
    /// Indicates the [`Producer`] is currently in a state where it will not produce any more goods.
    #[error(transparent)]
    Fault(T),
}

#[allow(clippy::use_self)] // False positive for ProduceFailure<T>.
impl<T> ProduceFailure<T>
where
    T: Error,
{
    /// Converts `self` into `ProduceFailure<U>`
    #[inline]
    pub fn map_into<U>(self) -> ProduceFailure<U>
    where
        U: Error + From<T>,
    {
        match self {
            Self::FullStock => ProduceFailure::FullStock,
            Self::Fault(fault) => ProduceFailure::Fault(fault.into()),
        }
    }
}

impl<T> From<T> for ProduceFailure<T>
where
    T: Error,
{
    #[inline]
    fn from(fault: T) -> Self {
        Self::Fault(fault)
    }
}

/// Returns a good that a `Producer` failed to produce.
#[derive(Debug, Eq, Hash, PartialEq, ThisError)]
#[error("")]
pub struct Recall<G, T>
where
    G: Debug,
    T: Error + 'static,
{
    /// The good that was not produced.
    good: G,
    /// The reason the production failed.
    failure: ProduceFailure<T>,
}

impl<G, T> Recall<G, T>
where
    G: Debug,
    T: Error,
{
    /// Creates a new [`Recall`].
    #[inline]
    pub fn new(good: G, failure: ProduceFailure<T>) -> Self {
        Self { good, failure }
    }

    /// Returns the recalled good if `self` is the result of a full stock; otherwise throws the fault.
    #[inline]
    #[throws(T)]
    pub fn overstock(self) -> G {
        match self.failure {
            ProduceFailure::FullStock => self.good,
            ProduceFailure::Fault(fault) => {
                throw!(fault);
            }
        }
    }
}

/// An error interacting with a market due to the market being closed.
#[derive(Clone, Copy, Debug, ThisError)]
#[error("market is closed")]
pub struct ClosedMarketFault;

impl core::convert::TryFrom<ConsumeFailure<ClosedMarketFault>> for ClosedMarketFault {
    type Error = ();

    #[throws(Self::Error)]
    fn try_from(failure: ConsumeFailure<ClosedMarketFault>) -> Self {
        if let ConsumeFailure::Fault(fault) = failure {
            fault
        } else {
            throw!(())
        }
    }
}
