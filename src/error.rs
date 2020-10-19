//! Implements errors thrown by `market`.
//!
//! There are 2 categories of errors thrown by `market`.
//! 1) Failures: These indicate that an action was not successful, but the state of the market is still valid.
//! 2) Faults: These indicate that the market is currently in a state that no attempted action will be successful until the state is changed (if possible).
use {
    crate::Consumer,
    conventus::AssembleFailure,
    core::{convert::{Infallible, TryFrom}, fmt::{Display, Debug}},
    fehler::{throw, throws},
    std::error::Error,
    thiserror::Error as ThisError,
};

pub trait ConsumerFailure: Sized {
    type Fault: TryFrom<Self>;
}

impl<T> ConsumerFailure for ClassicalConsumerFailure<T>
where
    T: TryFrom<Self> + Error,
{
    type Fault = T;
}

pub struct InfallibleConsumerFailure;

impl TryFrom<InfallibleConsumerFailure> for Infallible {
    type Error = ();

    fn try_from(_failure: InfallibleConsumerFailure) -> Result<Self, Self::Error> {
        Err(())
    }
}

impl ConsumerFailure for InfallibleConsumerFailure {
    type Fault = Infallible;
}

pub type ConsumerFault<T> = <<T as Consumer>::Failure as ConsumerFailure>::Fault;

// TODO: Rename ClassicalConsumerFailure
// Do not derive ThisError so that ClassicalConsumerFailure can be created without requiring it impl Display.
/// A `Consumer` failed to consume a good.
#[derive(Debug, Eq, Hash, PartialEq)]
pub enum ClassicalConsumerFailure<T>
where
    T: Debug,
{
    /// The stock of the market is empty.
    EmptyStock,
    /// A fault was thrown during consumption.
    ///
    /// Indicates the [`Consumer`] is currently in a state where it will not consume any more goods.
    Fault(T),
}

#[allow(clippy::use_self)] // False positive for ClassicalConsumerFailure<T>.
impl<T> ClassicalConsumerFailure<T>
where
    T: Error,
{
    // This is done because From<ClassicalConsumerFailure<T>> for ClassicalConsumerFailure<U> cannot.
    /// Converts `ClassicalConsumerFailure<T>` into `ClassicalConsumerFailure<U>`.
    #[inline]
    pub fn map_into<U>(self) -> ClassicalConsumerFailure<U>
    where
        U: Error + From<T>,
    {
        match self {
            Self::EmptyStock => ClassicalConsumerFailure::EmptyStock,
            Self::Fault(fault) => ClassicalConsumerFailure::Fault(fault.into()),
        }
    }
}

impl<T> Display for ClassicalConsumerFailure<T>
where
    T: Debug + Display,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyStock => write!(f, "stock is empty"),
            Self::Fault(fault) => write!(f, "{}", fault),
        }
    }
}

// TODO: Ideally could do From<AssembleFailure<E>> for ClassicalConsumerFailure<F> where F: From<E>. However this is overriden by From<E> for ClassicalConsumerFailure<E> since there is no way to indicate F != AssembleFailure<E>.
impl<T> From<AssembleFailure<T>> for ClassicalConsumerFailure<T>
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

impl<T> From<T> for ClassicalConsumerFailure<T>
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

impl core::convert::TryFrom<ClassicalConsumerFailure<ClosedMarketFault>> for ClosedMarketFault {
    type Error = ();

    #[throws(Self::Error)]
    fn try_from(failure: ClassicalConsumerFailure<ClosedMarketFault>) -> Self {
        if let ClassicalConsumerFailure::Fault(fault) = failure {
            fault
        } else {
            throw!(())
        }
    }
}
