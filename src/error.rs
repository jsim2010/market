//! Implements errors thrown by `market`.
#![macro_use]
use {
    crate::Failure,
    core::{
        convert::{Infallible, TryFrom},
        fmt::{Debug, Display},
    },
    fehler::{throw, throws},
    std::error::Error,
};

// Since unable to implement TryFrom<ConsumeFailure<T>> for T due to T not being covered, this macro implements that functionality.
/// Makes type able to be T in ConsumeFailure<T>.
#[macro_export]
macro_rules! consumer_fault {
    ($ty:tt$(<$generic:tt>)?$( where $t:ty: $bounds:path)?) => {
        #[allow(unused_qualifications)] // Where this macro will be put is unknown.
        impl$(<$generic>)? core::convert::TryFrom<$crate::ConsumeFailure<$ty$(<$generic>)?>> for $ty$(<$generic>)? $(where $t: $bounds )?{
            type Error = ();

            #[inline]
            #[fehler::throws(())]
            fn try_from(failure: $crate::ConsumeFailure<Self>) -> Self {
                if let $crate::ConsumeFailure::Fault(fault) = failure {
                    fault
                } else {
                    fehler::throw!(())
                }
            }
        }
    };
}

// Since unable to implement TryFrom<ProduceFailure<T>> for T due to T not being covered, this macro implements that functionality.
/// Makes type able to be T in ProduceFailure<T>.
#[macro_export]
macro_rules! producer_fault {
    ($ty:tt$(<$generic:tt>)?$( where $t:ty: $bounds:path)?) => {
        #[allow(unused_qualifications)] // Where this macro will be put is unknown.
        impl$(<$generic>)? core::convert::TryFrom<$crate::ProduceFailure<$ty$(<$generic>)?>> for $ty$(<$generic>)? $(where $t: $bounds )?{
            type Error = ();

            #[inline]
            #[fehler::throws(())]
            fn try_from(failure: $crate::ProduceFailure<Self>) -> Self {
                if let $crate::ProduceFailure::Fault(fault) = failure {
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
pub enum ConsumeFailure<T> {
    /// The stock of the market is empty.
    EmptyStock,
    /// Fault `T` was caught during consumption.
    Fault(T),
}

#[allow(clippy::use_self)] // False positive for ConsumeFailure<U>.
impl<T> ConsumeFailure<T> {
    // From<ConsumeFailure<T>> for ConsumeFailure<U> where U: From<T> would be preferrable but this conflicts with From<T> for T due to the inability to indicate that T != U.
    /// Converts `ConsumeFailure<T>` into `ConsumeFailure<U>`.
    #[inline]
    pub fn map_into<U>(self) -> ConsumeFailure<U>
    where
        U: From<T>,
    {
        match self {
            Self::EmptyStock => ConsumeFailure::EmptyStock,
            Self::Fault(fault) => ConsumeFailure::Fault(fault.into()),
        }
    }
}

impl<T> Failure for ConsumeFailure<T>
where
    T: TryFrom<Self>,
{
    type Fault = T;
}

// Display is implemented manually due to issue with thiserror::Error described above.
impl<T> Display for ConsumeFailure<T>
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
impl<T> Error for ConsumeFailure<T> where T: Debug + Display {}

// From<conventus::AssembleFailure<E>> for ConsumeFailure<T> where T: From<E> would be preferrable but this conflicts with From<T> for ConsumeFailure<T> due to the inability to indicate T != conventus::AssembleFailure<E>.
impl<T> From<conventus::AssembleFailure<T>> for ConsumeFailure<T> {
    #[inline]
    fn from(failure: conventus::AssembleFailure<T>) -> Self {
        match failure {
            conventus::AssembleFailure::Incomplete => Self::EmptyStock,
            conventus::AssembleFailure::Error(error) => Self::Fault(error),
        }
    }
}

// From<T> is implemented manually due to issue with thiserror::Error described above.
impl<T> From<T> for ConsumeFailure<T> {
    #[inline]
    fn from(fault: T) -> Self {
        Self::Fault(fault)
    }
}

/// The [`Failure`] thrown when an action fails in a case where a fault is not possible.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("stock is insufficient")]
pub struct FaultlessFailure;

impl TryFrom<FaultlessFailure> for Infallible {
    type Error = ();

    #[inline]
    #[throws(())]
    fn try_from(_failure: FaultlessFailure) -> Self {
        throw!(());
    }
}

impl Failure for FaultlessFailure {
    type Fault = Infallible;
}

/// The typical [`Failure`] thrown when a [`Producer`] is unable to produce a good.
///
/// This should be used in all cases where the only reason the [`Producer`] can fail without a fault is due to the stock being full.
// thiserror::Error is not derived so that T is not required to impl Display. see www.github.com/dtolnay/thiserror/pull/107
#[derive(Debug, Hash)]
pub enum ProduceFailure<T> {
    /// The stock of the market is full.
    FullStock,
    /// Fault `T` was thrown during production.
    Fault(T),
}

#[allow(clippy::use_self)] // False positive for ProduceFailure<T>.
impl<T> ProduceFailure<T> {
    // From<ProduceFailure<T>> for ProduceFailure<U> where U: From<T> would be preferrable but this conflicts with From<T> for T due to the inability to indicate that T != U.
    /// Converts `ProduceFailure<T>` into `ProduceFailure<U>`.
    #[inline]
    pub fn map_into<U>(self) -> ProduceFailure<U>
    where
        U: From<T>,
    {
        match self {
            Self::FullStock => ProduceFailure::FullStock,
            Self::Fault(fault) => ProduceFailure::Fault(fault.into()),
        }
    }
}

impl<T> Failure for ProduceFailure<T>
where
    T: TryFrom<Self>,
{
    type Fault = T;
}

// Display is implemented manually due to issue with thiserror::Error described above.
impl<T> Display for ProduceFailure<T>
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
impl<T> Error for ProduceFailure<T> where T: Debug + Display {}

// From<T> is implemented manually due to issue with thiserror::Error described above.
impl<T> From<T> for ProduceFailure<T> {
    #[inline]
    fn from(fault: T) -> Self {
        Self::Fault(fault)
    }
}
