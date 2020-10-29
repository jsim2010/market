//! Implements errors thrown by `market`.
#![macro_use]
use {
    core::{convert::{Infallible, TryFrom}, fmt::{Display, Debug}},
    std::error::Error,
};

// TODO: Perhaps make these derive macros?
// Since unable to implement TryFrom<ConsumerFailure<T>> for T due to T not being covered, this macro implements that functionality.
macro_rules! try_from_consumer_failure {
    ($t:ty) => {
        impl core::convert::TryFrom<$crate::ConsumerFailure<$t>> for $t {
            type Error = ();

            #[fehler::throws(Self::Error)]
            fn try_from(failure: $crate::ConsumerFailure<$t>) -> Self {
                if let $crate::ConsumerFailure::Fault(fault) = failure {
                    fault
                } else {
                    fehler::throw!(())
                }
            }
        }
    };
}

// Since unable to implement TryFrom<ProducerFailure<T>> for T due to T not being covered, this macro implements that functionality.
macro_rules! try_from_producer_failure {
    ($t:ty) => {
        impl core::convert::TryFrom<$crate::error::ProducerFailure<$t>> for $t {
            type Error = ();

            #[fehler::throws(Self::Error)]
            fn try_from(failure: $crate::error::ProducerFailure<$t>) -> Self {
                if let $crate::error::ProducerFailure::Fault(fault) = failure {
                    fault
                } else {
                    fehler::throw!(())
                }
            }
        }
    };
}

/// The typical [`Failure`] thrown when a [`Consumer`] is unable to consume a good.
///
/// This should be used in all cases where the only reason the [`Consumer`] can fail without a fault is due to the stock being empty.
// thiserror::Error is not derived so that T is not required to impl Display. see www.github.com/dtolnay/thiserror/pull/107
#[derive(Debug, Hash)]
pub enum ConsumerFailure<T>
{
    /// The stock of the market is empty.
    EmptyStock,
    /// Fault `T` was caught during consumption.
    Fault(T),
}

#[allow(clippy::use_self)] // False positive for ConsumerFailure<U>.
impl<T> ConsumerFailure<T>
{
    // From<ConsumerFailure<T>> for ConsumerFailure<U> where U: From<T> would be preferrable but this conflicts with From<T> for T due to the inability to indicate that T != U.
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

impl<T> crate::Failure for ConsumerFailure<T>
where
    T: TryFrom<Self>,
{
    type Fault = T;
}

// Display is implemented manually due to issue with thiserror::Error described above.
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

// Error is implemented manually due to issue with thiserror::Error described above.
impl<T> Error for ConsumerFailure<T>
where
    T: Debug + Display,
{}

// From<conventus::AssembleFailure<E>> for ConsumerFailure<T> where T: From<E> would be preferrable but this conflicts with From<T> for ConsumerFailure<T> due to the inability to indicate T != conventus::AssembleFailure<E>.
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

// From<T> is implemented manually due to issue with thiserror::Error described above.
impl<T> From<T> for ConsumerFailure<T>
{
    #[inline]
    fn from(fault: T) -> Self {
        Self::Fault(fault)
    }
}

/// The [`Failure`] thrown when an action fails in a case where a fault is not possible.
#[derive(Clone, Copy, Debug)]
pub struct FaultlessFailure;

impl TryFrom<FaultlessFailure> for Infallible {
    type Error = ();

    #[inline]
    fn try_from(_failure: FaultlessFailure) -> Result<Self, Self::Error> {
        Err(())
    }
}

impl crate::Failure for FaultlessFailure {
    type Fault = Infallible;
}

/// The typical [`Failure`] thrown when a [`Producer`] is unable to produce a good.
///
/// This should be used in all cases where the only reason the [`Producer`] can fail without a fault is due to the stock being full.
// thiserror::Error is not derived so that T is not required to impl Display. see www.github.com/dtolnay/thiserror/pull/107
#[derive(Debug, Hash)]
pub enum ProducerFailure<T>
{
    /// The stock of the market is full.
    FullStock,
    /// Fault `T` was thrown during production.
    Fault(T),
}

#[allow(clippy::use_self)] // False positive for ProducerFailure<T>.
impl<T> ProducerFailure<T>
{
    // From<ProducerFailure<T>> for ProducerFailure<U> where U: From<T> would be preferrable but this conflicts with From<T> for T due to the inability to indicate that T != U.
    /// Converts `ProducerFailure<T>` into `ProducerFailure<U>`.
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

impl<T> crate::Failure for ProducerFailure<T>
where
    T: TryFrom<Self>,
{
    type Fault = T;
}

// Display is implemented manually due to issue with thiserror::Error described above.
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

// Error is implemented manually due to issue with thiserror::Error described above.
impl<T> Error for ProducerFailure<T>
where
    T: Debug + Display,
{}

// From<T> is implemented manually due to issue with thiserror::Error described above.
impl<T> From<T> for ProducerFailure<T>
{
    #[inline]
    fn from(fault: T) -> Self {
        Self::Fault(fault)
    }
}
