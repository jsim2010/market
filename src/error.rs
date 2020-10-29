//! Implements errors thrown by `market`.
//!
//! There are 2 categories of errors thrown by `market`.
//! 1) Failures: These indicate that an action was not successful.
//! 2) Faults: These are a subset of Failrues that indicate that the market is currently in a state where no attempted action will be successful until the state is changed (if possible).
use {
    core::{convert::{Infallible, TryFrom}, fmt::{Display, Debug}},
    fehler::{throw, throws},
    std::error::Error,
};

/// Describes the failures that could occur during a consumption or production.
pub trait Failure: Sized {
    /// Describes the fault that could occur.
    type Fault: TryFrom<Self>;
}

/// A shortcut for referring to the fault of `F`.
pub type Fault<F> = <F as Failure>::Fault;

/// A `Consumer` failed to consume a good.
// thiserror::Error is not derived so that T is not required to impl Display. see www.github.com/dtolnay/thiserror/pull/107
#[derive(Debug, Hash)]
pub enum ConsumerFailure<T>
{
    /// The stock of the market is empty.
    EmptyStock,
    /// A fault was thrown during consumption.
    Fault(T),
}

#[allow(clippy::use_self)] // False positive for ConsumerFailure<T>.
impl<T> ConsumerFailure<T>
{
    // This is done because From<ConsumerFailure<T>> for ConsumerFailure<U> cannot.
    /// Converts `ConsumerFailure<T>` into `ConsumerFailure<U>`.
    #[inline]
    pub fn map_into<U>(self) -> ConsumerFailure<U>
    where
        U: From<T>,
    {
        match self {
            Self::EmptyStock => ConsumerFailure::EmptyStock,
            Self::Fault(fault) => ConsumerFailure::Fault(fault.into()),
        }
    }
}

// Display and Error are implemented manually due to issue with thiserror::Error described above.
impl<T> Display for ConsumerFailure<T>
where
    T: Display,
{
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::EmptyStock => write!(f, "stock is empty"),
            Self::Fault(ref fault) => write!(f, "{}", fault),
        }
    }
}

impl<T> Error for ConsumerFailure<T>
where
    T: Debug + Display,
{}

impl<T> Failure for ConsumerFailure<T>
where
    T: TryFrom<Self>,
{
    type Fault = T;
}

// TODO: Ideally could do From<conventus::AssembleFailure<E>> for ConsumerFailure<F> where F: From<E>. However this is overriden by From<E> for ConsumerFailure<E> since there is no way to indicate F != conventus::AssembleFailure<E>.
impl<T> From<conventus::AssembleFailure<T>> for ConsumerFailure<T>
{
    #[inline]
    fn from(failure: conventus::AssembleFailure<T>) -> Self {
        match failure {
            conventus::AssembleFailure::Incomplete => Self::EmptyStock,
            conventus::AssembleFailure::Error(error) => Self::Fault(error),
        }
    }
}

impl<T> From<T> for ConsumerFailure<T>
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
}

/// A `Producer` failed to produce a good.
// Do not derive thiserror::Error so that ProducerFailure can be created without requiring impl Display.
#[derive(Debug, Hash)]
pub enum ProducerFailure<T>
{
    /// The stock of the market is full.
    FullStock,
    /// A fault was thrown during production.
    ///
    /// Indicates the [`Producer`] is currently in a state where it will not produce any more goods.
    Fault(T),
}

impl<T> Failure for ProducerFailure<T>
where
    T: TryFrom<Self>,
{
    type Fault = T;
}

#[allow(clippy::use_self)] // False positive for ProducerFailure<T>.
impl<T> ProducerFailure<T>
{
    /// Converts `self` into `ProducerFailure<U>`
    #[inline]
    pub fn map_into<U>(self) -> ProducerFailure<U>
    where
        U: From<T>,
    {
        match self {
            Self::FullStock => ProducerFailure::FullStock,
            Self::Fault(fault) => ProducerFailure::Fault(fault.into()),
        }
    }
}

impl<T> Display for ProducerFailure<T>
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

impl<T> From<T> for ProducerFailure<T>
{
    #[inline]
    fn from(fault: T) -> Self {
        Self::Fault(fault)
    }
}

/// An error interacting with a market due to the market being closed.
#[derive(Clone, Copy, Debug, Eq, thiserror::Error, PartialEq)]
#[error("market is closed")]
pub struct ClosedMarketFault;

impl TryFrom<ConsumerFailure<ClosedMarketFault>> for ClosedMarketFault {
    type Error = ();

    #[inline]
    #[throws(Self::Error)]
    fn try_from(failure: ConsumerFailure<Self>) -> Self {
        if let ConsumerFailure::Fault(fault) = failure {
            fault
        } else {
            throw!(())
        }
    }
}

impl TryFrom<ProducerFailure<ClosedMarketFault>> for ClosedMarketFault {
    type Error = ();

    #[inline]
    #[throws(Self::Error)]
    fn try_from(failure: ProducerFailure<Self>) -> Self {
        if let ProducerFailure::Fault(fault) = failure {
            fault
        } else {
            throw!(())
        }
    }
}
