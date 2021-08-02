//! Defines the errors that can be thrown by an [`Agent`].
#[cfg(doc)]
use crate::{Agent, Consumer, Producer};

use {
    alloc::string::String,
    core::{
        convert::TryFrom,
        fmt::{self, Debug, Display, Formatter},
        marker::PhantomData,
    },
    fehler::{throw, throws},
    never::Never,
};

/// Characterizes the kinds of errors that can cause the action of an [`Agent`] to fail.
pub trait Flaws {
    /// Specifies the error caused by the lack of a resource such as a good or stock.
    ///
    /// An insufficiency shall be a temporary error; i.e. given that the market is operating under normal conditions, an insufficiency shall eventually be resolved without requiring any extra manipulation of the market.
    ///
    /// **NOTE** If implementing [`Display`] for this type, this will generally be displayed as part of [`Fault::Insufficiency`], which will prepend "insufficient " to the display from this type.
    type Insufficiency;
    /// Specifies the error caused by an invalid outcome during the action.
    ///
    /// A defect is a semi-permanent error; i.e. it shall not be resolved without extra manipulation of the market, if resolution is possible. It is possible that resolution is not possible for a defect.
    type Defect;
}

/// Characterizes converting an item into a `T`.
///
/// This is functionally the same as [`Into`], but does not include a generic implementation of `T: Blame<T>`. This allows more specific implementations without conflicts. This will be deprecated when specialization is stabilized.
pub trait Blame<T> {
    /// Converts `self` into a `T`.
    fn blame(self) -> T;
}

/// Characterizes attempting to convert an item into a `T`.
///
/// This is functionally the same as [`core::convert::TryInto`], but does not include a generic implementation of `T: TryBlame<T>`. This allows more specific implementations without conflicts. This will be deprecated when specialization is stabilized.
pub trait TryBlame<T> {
    /// Specifies the error thrown if conversion fails.
    type Error;

    /// Attempts to convert `self` into a `T`.
    #[throws(Self::Error)]
    fn try_blame(self) -> T;
}

/// The cause of an [`Agent`] failing to successfully complete an action upon a market.
#[non_exhaustive]
pub enum Fault<F: Flaws> {
    /// The action failed due to an insufficiency.
    Insufficiency(F::Insufficiency),
    /// The action failed due to a defect.
    Defect(F::Defect),
}

impl<F: Flaws> Fault<F> {
    /// Returns if `self` is a defect.
    fn is_defect(&self) -> bool {
        matches!(*self, Self::Defect(_))
    }
}

impl<F: Flaws, W: Flaws> Blame<Fault<W>> for Fault<F>
where
    W::Insufficiency: From<F::Insufficiency>,
    W::Defect: From<F::Defect>,
{
    fn blame(self) -> Fault<W> {
        match self {
            Fault::Insufficiency(insufficiency) => {
                Fault::Insufficiency(W::Insufficiency::from(insufficiency))
            }
            Fault::Defect(defect) => Fault::Defect(W::Defect::from(defect)),
        }
    }
}

impl<F: Flaws> Clone for Fault<F>
where
    F::Insufficiency: Clone,
    F::Defect: Clone,
{
    fn clone(&self) -> Self {
        match *self {
            Self::Insufficiency(ref insufficiency) => Self::Insufficiency(insufficiency.clone()),
            Self::Defect(ref defect) => Self::Defect(defect.clone()),
        }
    }
}

impl<F: Flaws> Copy for Fault<F>
where
    F::Insufficiency: Copy,
    F::Defect: Copy,
{
}

impl<F: Flaws> Debug for Fault<F>
where
    F::Insufficiency: Debug,
    F::Defect: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Insufficiency(ref insufficiency) => {
                write!(f, "Fault::Insufficiency({:?})", insufficiency)
            }
            Self::Defect(ref defect) => write!(f, "Fault::Defect({:?})", defect),
        }
    }
}

impl<F: Flaws> Display for Fault<F>
where
    F::Insufficiency: Display,
    F::Defect: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Insufficiency(ref insufficiency) => write!(f, "insufficient {}", insufficiency),
            Self::Defect(ref defect) => write!(f, "{}", defect),
        }
    }
}

impl<F: Flaws> PartialEq for Fault<F>
where
    F::Insufficiency: PartialEq,
    F::Defect: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match *self {
            Fault::Insufficiency(ref insufficiency) => {
                if let Fault::Insufficiency(ref other_insufficiency) = *other {
                    insufficiency == other_insufficiency
                } else {
                    false
                }
            }
            Fault::Defect(ref defect) => {
                if let Fault::Defect(ref other_defect) = *other {
                    defect == other_defect
                } else {
                    false
                }
            }
        }
    }
}

impl<F: Flaws, W: Flaws> TryBlame<Fault<W>> for Fault<F>
where
    W::Insufficiency: TryFrom<F::Insufficiency>,
    W::Defect: TryFrom<F::Defect>,
{
    type Error = FaultConversionError<W, F>;

    #[throws(Self::Error)]
    fn try_blame(self) -> Fault<W> {
        match self {
            Fault::Insufficiency(insufficiency) => Fault::Insufficiency(
                W::Insufficiency::try_from(insufficiency)
                    .map_err(FaultConversionError::Insufficiency)?,
            ),
            Fault::Defect(defect) => {
                Fault::Defect(W::Defect::try_from(defect).map_err(FaultConversionError::Defect)?)
            }
        }
    }
}

/// The error thrown when the action of an [`Agent`] fails.
pub struct Failure<F: Flaws> {
    /// The name of the [`Agent`].
    agent_name: String,
    /// The cause of the failure.
    fault: Fault<F>,
}

impl<F: Flaws> Failure<F> {
    /// Creates a new [`Failure`] with the `agent_name`and `fault` that caused the failure.
    pub(crate) fn new(fault: Fault<F>, agent_name: String) -> Self {
        Self { agent_name, fault }
    }

    /// Returns if `self` was caused by a defect.
    pub fn is_defect(&self) -> bool {
        self.fault.is_defect()
    }
}

impl<F: Flaws, W: Flaws> Blame<Failure<W>> for Failure<F>
where
    W::Insufficiency: From<F::Insufficiency>,
    W::Defect: From<F::Defect>,
{
    fn blame(self) -> Failure<W> {
        Failure::new(self.fault.blame(), self.agent_name)
    }
}

impl<F: Flaws> Debug for Failure<F>
where
    F::Insufficiency: Debug,
    F::Defect: Debug,
{
    /// Writes the default debug format for `self`.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Failure")
            .field("agent_name", &self.agent_name)
            .field("fault", &self.fault)
            .finish()
    }
}

impl<F: Flaws> Display for Failure<F>
where
    F::Insufficiency: Display,
    F::Defect: Display,
{
    /// Writes "{name}: {fault}".
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.agent_name, self.fault)
    }
}

#[cfg(feature = "std")]
#[cfg_attr(feature = "unstable-doc-cfg", doc(cfg(feature = "std")))]
impl<F: Flaws> std::error::Error for Failure<F>
where
    F::Insufficiency: Debug + Display,
    F::Defect: Debug + Display,
{
}

impl<F: Flaws> PartialEq for Failure<F>
where
    F::Insufficiency: PartialEq,
    F::Defect: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.agent_name == other.agent_name && self.fault == other.fault
    }
}

impl<F: Flaws, W: Flaws> TryBlame<Failure<W>> for Failure<F>
where
    W::Insufficiency: TryFrom<F::Insufficiency>,
    W::Defect: TryFrom<F::Defect>,
{
    type Error = FailureConversionError<W, F>;

    #[throws(Self::Error)]
    fn try_blame(self) -> Failure<W> {
        match self.fault.try_blame() {
            Ok(fault) => Failure::new(fault, self.agent_name),
            Err(error) => throw!(FailureConversionError {
                error,
                agent_name: self.agent_name
            }),
        }
    }
}

/// The error thrown when a [`Producer`] fails to produce a good.
pub struct Recall<F: Flaws, G> {
    /// The good that was not produced.
    good: G,
    /// The failure.
    failure: Failure<F>,
}

impl<F: Flaws, G> Recall<F, G> {
    /// Creates a new [`Recall`] with the `failure` and `good` that was not produced.
    pub(crate) fn new(failure: Failure<F>, good: G) -> Self {
        Self { good, failure }
    }
}

impl<F: Flaws, G, W: Flaws, T> Blame<Recall<W, T>> for Recall<F, G>
where
    T: From<G>,
    W::Insufficiency: From<F::Insufficiency>,
    W::Defect: From<F::Defect>,
{
    fn blame(self) -> Recall<W, T> {
        Recall::new(self.failure.blame(), T::from(self.good))
    }
}

impl<F: Flaws, G: Debug> Debug for Recall<F, G>
where
    F::Insufficiency: Debug,
    F::Defect: Debug,
{
    /// Writes the default debug format for `self`.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Recall")
            .field("good", &self.good)
            .field("failure", &self.failure)
            .finish()
    }
}

impl<F: Flaws, G> Display for Recall<F, G>
where
    F::Insufficiency: Display,
    F::Defect: Display,
    G: Display,
{
    /// Writes "`{}` caused recall of goods [{goods}]".
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` caused recall of good {}", self.failure, self.good)
    }
}

#[cfg(feature = "std")]
#[cfg_attr(feature = "unstable-doc-cfg", doc(cfg(feature = "std")))]
impl<F: Flaws, G> std::error::Error for Recall<F, G>
where
    F::Insufficiency: Debug + Display,
    F::Defect: Debug + Display,
    G: Debug + Display,
{
}

impl<F: Flaws, G> PartialEq for Recall<F, G>
where
    F::Insufficiency: PartialEq,
    F::Defect: PartialEq,
    G: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.failure == other.failure && self.good == other.good
    }
}

impl<F: Flaws, G, W: Flaws, T> TryBlame<Recall<W, T>> for Recall<F, G>
where
    W::Insufficiency: TryFrom<F::Insufficiency>,
    W::Defect: TryFrom<F::Defect>,
    T: From<G>,
{
    type Error = RecallConversionError<W, F, G>;

    #[throws(Self::Error)]
    fn try_blame(self) -> Recall<W, T> {
        match self.failure.try_blame() {
            Ok(failure) => Recall::new(failure, T::from(self.good)),
            Err(error) => throw!(RecallConversionError {
                error,
                good: self.good,
            }),
        }
    }
}

/// The error thrown when `Fault::blame()` fails.
#[non_exhaustive]
pub enum FaultConversionError<F: Flaws, W: Flaws>
where
    F::Insufficiency: TryFrom<W::Insufficiency>,
    F::Defect: TryFrom<W::Defect>,
{
    /// The failure to convert the insufficiency of `W` to that of `F`.
    Insufficiency(<F::Insufficiency as TryFrom<W::Insufficiency>>::Error),
    /// The failure to convert the defect of `W` to that of `F`.
    Defect(<F::Defect as TryFrom<W::Defect>>::Error),
}

impl<F: Flaws, W: Flaws> Debug for FaultConversionError<F, W>
where
    F::Insufficiency: TryFrom<W::Insufficiency>,
    <F::Insufficiency as TryFrom<W::Insufficiency>>::Error: Debug,
    F::Defect: TryFrom<W::Defect>,
    <F::Defect as TryFrom<W::Defect>>::Error: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Insufficiency(ref insufficiency_error) => {
                write!(f, "FaultConversionError({:?})", insufficiency_error)
            }
            Self::Defect(ref defect_error) => write!(f, "FaultConversionError({:?})", defect_error),
        }
    }
}

impl<F: Flaws, W: Flaws> Display for FaultConversionError<F, W>
where
    F::Insufficiency: TryFrom<W::Insufficiency>,
    <F::Insufficiency as TryFrom<W::Insufficiency>>::Error: Display,
    F::Defect: TryFrom<W::Defect>,
    <F::Defect as TryFrom<W::Defect>>::Error: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Insufficiency(ref insufficiency_error) => {
                write!(f, "insufficiency conversion - {}", insufficiency_error)
            }
            Self::Defect(ref defect_error) => {
                write!(f, "defect conversion - {}", defect_error)
            }
        }
    }
}

#[cfg(feature = "std")]
#[cfg_attr(feature = "unstable-doc-cfg", doc(cfg(feature = "std")))]
impl<F: Flaws, W: Flaws> std::error::Error for FaultConversionError<F, W>
where
    F::Insufficiency: TryFrom<W::Insufficiency>,
    <F::Insufficiency as TryFrom<W::Insufficiency>>::Error: Debug + Display,
    F::Defect: TryFrom<W::Defect>,
    <F::Defect as TryFrom<W::Defect>>::Error: Debug + Display,
{
}

/// The error thrown when `Failure::blame()` fails.
pub struct FailureConversionError<F: Flaws, W: Flaws>
where
    F::Insufficiency: TryFrom<W::Insufficiency>,
    F::Defect: TryFrom<W::Defect>,
{
    /// The error that caused the failure.
    error: FaultConversionError<F, W>,
    /// The name of the [`Agent`] that experienced the failure.
    agent_name: String,
}

impl<F: Flaws, W: Flaws> Debug for FailureConversionError<F, W>
where
    F::Insufficiency: TryFrom<W::Insufficiency>,
    <F::Insufficiency as TryFrom<W::Insufficiency>>::Error: Debug,
    F::Defect: TryFrom<W::Defect>,
    <F::Defect as TryFrom<W::Defect>>::Error: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FailureConversionError")
            .field("error", &self.error)
            .field("agent_name", &self.agent_name)
            .finish()
    }
}

impl<F: Flaws, W: Flaws> Display for FailureConversionError<F, W>
where
    F::Insufficiency: TryFrom<W::Insufficiency>,
    <F::Insufficiency as TryFrom<W::Insufficiency>>::Error: Display,
    F::Defect: TryFrom<W::Defect>,
    <F::Defect as TryFrom<W::Defect>>::Error: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "in `{}`: {}", self.agent_name, self.error)
    }
}

#[cfg(feature = "std")]
#[cfg_attr(feature = "unstable-doc-cfg", doc(cfg(feature = "std")))]
impl<F: Flaws, W: Flaws> std::error::Error for FailureConversionError<F, W>
where
    F::Insufficiency: TryFrom<W::Insufficiency>,
    <F::Insufficiency as TryFrom<W::Insufficiency>>::Error: Debug + Display,
    F::Defect: TryFrom<W::Defect>,
    <F::Defect as TryFrom<W::Defect>>::Error: Debug + Display,
{
}

/// The error thrown when `Recall::blame()` fails.
pub struct RecallConversionError<F: Flaws, W: Flaws, G>
where
    F::Insufficiency: TryFrom<W::Insufficiency>,
    F::Defect: TryFrom<W::Defect>,
{
    /// The error when converting the [`Failure`].
    error: FailureConversionError<F, W>,
    /// The good in the recall.
    good: G,
}

impl<F: Flaws, W: Flaws, G> RecallConversionError<F, W, G>
where
    F::Insufficiency: TryFrom<W::Insufficiency>,
    F::Defect: TryFrom<W::Defect>,
{
    /// Converts `self` into a `G`.
    pub fn into_good(self) -> G {
        self.good
    }
}

impl<F: Flaws, W: Flaws, G: Debug> Debug for RecallConversionError<F, W, G>
where
    F::Insufficiency: TryFrom<W::Insufficiency>,
    <F::Insufficiency as TryFrom<W::Insufficiency>>::Error: Debug,
    F::Defect: TryFrom<W::Defect>,
    <F::Defect as TryFrom<W::Defect>>::Error: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecallConversionError")
            .field("good", &self.good)
            .field("error", &self.error)
            .finish()
    }
}

impl<F: Flaws, W: Flaws, G> Display for RecallConversionError<F, W, G>
where
    F::Insufficiency: TryFrom<W::Insufficiency>,
    <F::Insufficiency as TryFrom<W::Insufficiency>>::Error: Display,
    F::Defect: TryFrom<W::Defect>,
    <F::Defect as TryFrom<W::Defect>>::Error: Display,
    G: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} while converting recall with good {}",
            self.error, self.good
        )
    }
}

#[cfg(feature = "std")]
#[cfg_attr(feature = "unstable-doc-cfg", doc(cfg(feature = "std")))]
impl<F: Flaws, W: Flaws, G> std::error::Error for RecallConversionError<F, W, G>
where
    F::Insufficiency: TryFrom<W::Insufficiency>,
    <F::Insufficiency as TryFrom<W::Insufficiency>>::Error: Debug + Display,
    F::Defect: TryFrom<W::Defect>,
    <F::Defect as TryFrom<W::Defect>>::Error: Debug + Display,
    G: Debug + Display,
{
}

/// Signifies a fault that can never occur.
pub type Flawless = Never;

/// The insufficiency thrown when a [`Producer`] attempts to produce to a market that has no stock available.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct FullStock;

impl Display for FullStock {
    /// Writes "stock".
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "stock")
    }
}

impl Flaws for FullStock {
    type Insufficiency = Self;
    type Defect = Flawless;
}

/// The insufficiency thrown when a [`Consumer`] attempts to consume from a market that has no goods available.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct EmptyStock;

impl Display for EmptyStock {
    /// Writes "goods".
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "goods")
    }
}

impl Flaws for EmptyStock {
    type Insufficiency = Self;
    type Defect = Flawless;
}

/// Specifies the [`Flaws`] of a [`Producer`] producing to a finite market with defects of type `D`.
#[derive(Debug)]
pub struct ProductionFlaws<D> {
    /// The type of the defect.
    defect: PhantomData<D>,
}

impl<D> Flaws for ProductionFlaws<D> {
    type Insufficiency = FullStock;
    type Defect = D;
}

/// Specifies the [`Flaws`] of a [`Consumer`] consuming from a market with defects of type `D`.
#[derive(Debug)]
pub struct ConsumptionFlaws<D> {
    /// The type of the defect.
    defect: PhantomData<D>,
}

impl<D> Flaws for ConsumptionFlaws<D> {
    type Insufficiency = EmptyStock;
    type Defect = D;
}

impl Flaws for Flawless {
    type Insufficiency = Self;
    type Defect = Self;
}

impl TryFrom<EmptyStock> for Flawless {
    type Error = ();

    fn try_from(_: EmptyStock) -> Result<Self, Self::Error> {
        Err(())
    }
}

impl TryFrom<FullStock> for Flawless {
    type Error = ();

    fn try_from(_: FullStock) -> Result<Self, Self::Error> {
        Err(())
    }
}
