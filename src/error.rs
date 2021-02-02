//! Describes errors that market agents may throw during their respective actions.
//!
//! All errors that market agents may throw to indicate an action failed are [`Failure`]s. A subset of [`Failure`]s are caused by an insufficient stock; the stock does not have enough space for the good from a [`Producer`] or the stock is empty and unable to provide a good to a [`Consumer`]. All other [`Failure`]s are faults.
use {
    core::{
        convert::{Infallible, TryFrom},
        fmt::{Debug, Display},
    },
    fehler::{throw, throws},
    std::error::Error,
};

#[cfg(doc)]
use crate::{Consumer, Producer};

/// Specifies a failure to successfully complete an action.
pub trait Failure: Sized {
    /// Describes the possible faults of `Self`.
    type Fault: TryFrom<Self>;

    /// Attempts to convert `self` into a [`Self::Fault`].
    ///
    /// If `self` cannot be converted into a fault, SHALL return [`None`].
    #[inline]
    fn fault(self) -> Option<Self::Fault> {
        Self::Fault::try_from(self).ok()
    }
}

/// [`Infallible`] SHALL implement [`Failure`].
impl Failure for Infallible {
    type Fault = Self;
}

/// The typical [`Failure`] thrown when a [`Consumer`] is unable to consume a good.
///
/// This SHOULD be used in all cases where the only possible [`Failure`] thrown by a [`Consumer`] that is not a fault is due to the stock being empty.
///
/// Implements [`Debug`], [`Failure`] and [`From<T>`]. Implements [`Display`] and [`Error`] when feasible.
///
/// BLOCKED: Implements `From<InsufficientStockFailure>`
/// CAUSE: This conflicts with `ConsumeFailure<T>: From<T>`.
// thiserror::Error is not derived so that T is not required to impl Display. see www.github.com/dtolnay/thiserror/pull/107
#[derive(Debug)]
pub enum ConsumeFailure<T> {
    /// The stock of the market is empty.
    EmptyStock,
    /// Fault `T` was caught during consumption.
    Fault(T),
}

#[allow(clippy::use_self)] // False positive.
impl<T> ConsumeFailure<T> {
    /// Converts a [`ConsumeFailure<F>`] into a [`ConsumeFailure<T>`].  
    ///
    /// BLOCKED: Implements `ConsumeFailure<T>: From<ConsumeFailure<F>>`.
    /// CAUSE: This conflicts with `ConsumeFailure<T>: From<T>`.
    #[inline]
    pub fn map_fault<F>(failure: ConsumeFailure<F>) -> Self
    where
        T: From<F>,
    {
        if let ConsumeFailure::Fault(fault) = failure {
            Self::Fault(T::from(fault))
        } else {
            Self::EmptyStock
        }
    }
}

impl<T: TryFrom<Self>> Failure for ConsumeFailure<T> {
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

// From<T> is implemented manually due to issue with thiserror::Error described above.
impl<T> From<T> for ConsumeFailure<T> {
    #[inline]
    fn from(fault: T) -> Self {
        Self::Fault(fault)
    }
}

/// The typical [`Failure`] thrown when a [`Producer`] is unable to produce a good.
///
/// This SHOULD be used in all cases where the only possible [`Failure`] thrown by a [`Producer`] that is not a fault is due to the stock being full.
///
/// Implements [`Debug`], [`Failure`] and [`From<T>`]. Implements [`Display`] and [`Error`] when feasible.
///
/// BLOCKED: Implements `From<InsufficientStockFailure>`
/// CAUSE: This conflicts with `ProduceFailure<T>: From<T>`.
// thiserror::Error is not derived so that T is not required to impl Display. see www.github.com/dtolnay/thiserror/pull/107
#[derive(Debug, Hash)]
pub enum ProduceFailure<T> {
    /// The stock of the market is full.
    FullStock,
    /// Fault `T` was caught during production.
    Fault(T),
}

#[allow(clippy::use_self)] // False positive.
impl<T> ProduceFailure<T> {
    /// Converts a [`ProduceFailure<F>`] into a [`ProduceFailure<T>`].  
    ///
    /// BLOCKED: Implements `ProduceFailure<T>: From<ProduceFailure<F>>`.
    /// CAUSE: This conflicts with `ProduceFailure<T>: From<T>`.
    #[inline]
    pub fn map_fault<F>(failure: ProduceFailure<F>) -> Self
    where
        T: From<F>,
    {
        if let ProduceFailure::Fault(fault) = failure {
            Self::Fault(T::from(fault))
        } else {
            Self::FullStock
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

/// The [`Failure`] thrown when a faultless agent fails to complete an action.
///
/// A faultless agent is an agent that does not throw a [`Failure`] that can be considered a fault.
///
/// Implements [`Clone`], [`Copy`], [`Debug`], [`Failure`], [`Error`] and [`Display`].
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("stock is insufficient")]
pub struct InsufficientStockFailure;

// Required by `InsufficientStockFailure: Failure`.
impl TryFrom<InsufficientStockFailure> for Infallible {
    type Error = ();

    #[inline]
    #[throws(())]
    fn try_from(_failure: InsufficientStockFailure) -> Self {
        throw!(());
    }
}

impl Failure for InsufficientStockFailure {
    type Fault = Infallible;
}
