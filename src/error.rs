//! Implements errors thrown by `market`.
//!
//! There are 2 categories of errors thrown by `market`.
//! 1) Failures: These indicate that an action was not successful.
//! 2) Faults: These are a subset of Failrues that indicate that the market is currently in a state where no attempted action will be successful until the state is changed (if possible).
use {
    conventus::AssembleFailure,
    core::{convert::{Infallible, TryFrom}, fmt::{Display, Debug}},
    never::Never,
    fehler::{throw, throws},
    std::error::Error,
    thiserror::Error as ThisError,
};

/// Describes the failures that could occur during a consumption or production.
pub trait Failure: Sized {
    /// Describes the fault that could occur.
    type Fault: TryFrom<Self>;

    fn insufficient_stock() -> Self;

    fn is_insufficient_stock(&self) -> bool;
}

/// A shortcut for referring to the fault of `F`.
pub type Fault<F> = <F as Failure>::Fault;

/// A `Consumer` failed to consume a good.
// Do not derive ThisError so that ClassicalConsumerFailure can be created without requiring it impl Display.
#[derive(Debug, Hash)]
pub enum ClassicalConsumerFailure<T>
{
    /// The stock of the market is empty.
    EmptyStock,
    /// A fault was thrown during consumption.
    ///
    /// Indicates the [`Consumer`] is currently in a state where it will not consume any more goods.
    Fault(T),
}

impl<T> Failure for ClassicalConsumerFailure<T>
where
    T: TryFrom<Self>,
{
    type Fault = T;

    fn insufficient_stock() -> Self {
        Self::EmptyStock
    }

    fn is_insufficient_stock(&self) -> bool {
        if let Self::EmptyStock = self {
            true
        } else {
            false
        }
    }
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
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::EmptyStock => write!(f, "stock is empty"),
            Self::Fault(ref fault) => write!(f, "{}", fault),
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

/// A `Failure` in the case where a fault is not possible.
#[derive(Clone, Copy, Debug)]
pub struct InfallibleFailure;

impl TryFrom<InfallibleFailure> for Infallible {
    type Error = ();

    #[inline]
    fn try_from(_failure: InfallibleFailure) -> Result<Self, Self::Error> {
        Err(())
    }
}

impl Failure for InfallibleFailure {
    type Fault = Infallible;

    fn insufficient_stock() -> Self {
        Self
    }

    fn is_insufficient_stock(&self) -> bool {
        true
    }
}

/// A `Producer` failed to produce a good.
// Do not derive ThisError so that ClassicalProducerFailure can be created without requiring impl Display.
#[derive(Debug, Hash)]
pub enum ClassicalProducerFailure<T>
{
    /// The stock of the market is full.
    FullStock,
    /// A fault was thrown during production.
    ///
    /// Indicates the [`Producer`] is currently in a state where it will not produce any more goods.
    Fault(T),
}

impl<T> Failure for ClassicalProducerFailure<T>
where
    T: TryFrom<Self>,
{
    type Fault = T;

    fn insufficient_stock() -> Self {
        Self::FullStock
    }

    fn is_insufficient_stock(&self) -> bool {
        if let Self::FullStock = self {
            true
        } else {
            false
        }
    }
}

#[allow(clippy::use_self)] // False positive for ClassicalProducerFailure<T>.
impl<T> ClassicalProducerFailure<T>
where
    T: Error,
{
    /// Converts `self` into `ClassicalProducerFailure<U>`
    #[inline]
    pub fn map_into<U>(self) -> ClassicalProducerFailure<U>
    where
        U: Error + From<T>,
    {
        match self {
            Self::FullStock => ClassicalProducerFailure::FullStock,
            Self::Fault(fault) => ClassicalProducerFailure::Fault(fault.into()),
        }
    }
}

impl<T> Display for ClassicalProducerFailure<T>
where
    T: Display,
{
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::FullStock => write!(f, "stock is full"),
            Self::Fault(ref fault) => write!(f, "{}", fault),
        }
    }
}

impl<T> From<T> for ClassicalProducerFailure<T>
where
    T: Error,
{
    #[inline]
    fn from(fault: T) -> Self {
        Self::Fault(fault)
    }
}

/// An error interacting with a market due to the market being closed.
#[derive(Clone, Copy, Debug, Eq, ThisError, PartialEq)]
#[error("market is closed")]
pub struct ClosedMarketFault;

impl TryFrom<ClassicalConsumerFailure<ClosedMarketFault>> for ClosedMarketFault {
    type Error = ();

    #[inline]
    #[throws(Self::Error)]
    fn try_from(failure: ClassicalConsumerFailure<Self>) -> Self {
        if let ClassicalConsumerFailure::Fault(fault) = failure {
            fault
        } else {
            throw!(())
        }
    }
}

impl TryFrom<ClassicalProducerFailure<ClosedMarketFault>> for ClosedMarketFault {
    type Error = ();

    #[inline]
    #[throws(Self::Error)]
    fn try_from(failure: ClassicalProducerFailure<Self>) -> Self {
        if let ClassicalProducerFailure::Fault(fault) = failure {
            fault
        } else {
            throw!(())
        }
    }
}

impl TryFrom<ClassicalProducerFailure<Never>> for Never {
    type Error = ();

    #[inline]
    #[throws(Self::Error)]
    fn try_from(failure: ClassicalProducerFailure<Self>) -> Self {
        if let ClassicalProducerFailure::Fault(fault) = failure {
            fault
        } else {
            throw!(())
        }
    }
}
