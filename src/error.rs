//! Implements errors thrown by `market`.
#![allow(clippy::pattern_type_mismatch)] // False positive.
use {
    core::{
        convert::{Infallible, TryFrom},
        fmt::{Debug, Display},
    },
    fehler::{throw, throws},
    std::error::Error,
};

/// Describes the failures that could occur during a given action.
pub trait Failure: Sized {
    /// Describes the fault that could occur.
    type Fault: TryFrom<Self>;

    /// Converts failure `F` into `Self`.
    fn map_from<F: Failure>(failure: F) -> Self
    where
        Fault<Self>: From<Fault<F>>;
}

impl Failure for Infallible {
    type Fault = Self;

    #[inline]
    fn map_from<F: Failure>(failure: F) -> Self
    where
        Fault<Self>: From<Fault<F>>,
    {
        #[allow(clippy::unreachable)]
        // Required until unwrap_infallible is stabilized; see https://github.com/rust-lang/rust/issues/61695.
        if let Ok(fault) = Fault::<F>::try_from(failure) {
            fault.into()
        } else {
            unreachable!("Attempted to convert a failure into `Infallible`");
        }
    }
}

/// The type of [`Failure::Fault`] defined by the [`Failure`] `F`.
pub type Fault<F> = <F as Failure>::Fault;

/// The kinds of participants in a market.
#[derive(Clone, Copy, Debug, parse_display::Display)]
#[display(style = "CamelCase")]
pub enum Participant {
    /// A producer.
    Producer,
    /// A consumer.
    Consumer,
}

/// An error thrown when attempting to access a participant that was already taken.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("{0} was already taken")]
pub struct TakenParticipant(pub Participant);

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

impl<T: TryFrom<Self>> Failure for ConsumeFailure<T> {
    type Fault = T;

    #[inline]
    fn map_from<F: Failure>(failure: F) -> Self
    where
        Fault<Self>: From<Fault<F>>,
    {
        if let Ok(fault) = Fault::<F>::try_from(failure) {
            Self::Fault(fault.into())
        } else {
            Self::EmptyStock
        }
    }
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

    #[inline]
    fn map_from<F: Failure>(failure: F) -> Self
    where
        Fault<Self>: From<Fault<F>>,
    {
        #[allow(clippy::unreachable)]
        // Required until unwrap_infallible is stabilized; see https://github.com/rust-lang/rust/issues/61695.
        if Fault::<F>::try_from(failure).is_ok() {
            unreachable!("Attempted to convert a fault into `FaultlessFailure`");
        } else {
            Self
        }
    }
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
    // From<ProduceFailure<T>> for ProduceFailure<U> where U: From<T> would be preferrable but this conflicts with From<T> for T due to the inability to specify that T != U.
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

    #[inline]
    fn map_from<F: Failure>(failure: F) -> Self
    where
        Fault<Self>: From<Fault<F>>,
    {
        if let Ok(fault) = Fault::<F>::try_from(failure) {
            Self::Fault(fault.into())
        } else {
            Self::FullStock
        }
    }
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
