//! Defines the errors that can be thrown by an [`Agent`].
#[cfg(doc)]
use crate::{Agent, Consumer, Producer};

use {
    alloc::string::{String, ToString},
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
pub enum Fault<F>
where
    F: Flaws,
{
    /// The action failed due to an insufficiency.
    Insufficiency(F::Insufficiency),
    /// The action failed due to a defect.
    Defect(F::Defect),
}

impl<F> Fault<F>
where
    F: Flaws,
{
    /// Returns if `self` is a defect.
    fn is_defect(&self) -> bool {
        matches!(*self, Self::Defect(_))
    }

    /// If `self` is a defect, converts the defect into `W::Defect`; otherwise returns `self`.
    fn map_defect<M, W>(self, mut m: M) -> Fault<W>
    where
        M: FnMut(F::Defect) -> W::Defect,
        W: Flaws<Insufficiency = F::Insufficiency>,
    {
        match self {
            Self::Insufficiency(insufficiency) => Fault::Insufficiency(insufficiency),
            Self::Defect(defect) => Fault::Defect(m(defect)),
        }
    }
}

impl<F, W> Blame<Fault<W>> for Fault<F>
where
    F: Flaws,
    W: Flaws,
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

impl<F> Clone for Fault<F>
where
    F: Flaws,
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

impl<F> Copy for Fault<F>
where
    F: Flaws,
    F::Insufficiency: Copy,
    F::Defect: Copy,
{
}

impl<F> Debug for Fault<F>
where
    F: Flaws,
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

impl<F> Display for Fault<F>
where
    F: Flaws,
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

impl<F> PartialEq for Fault<F>
where
    F: Flaws,
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

impl<F, W> TryBlame<Fault<W>> for Fault<F>
where
    F: Flaws,
    W: Flaws,
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
    /// The description of the [`Agent`].
    agent_description: String,
    /// The cause of the failure.
    fault: Fault<F>,
}

impl<F> Failure<F>
where
    F: Flaws,
{
    /// Creates a new [`Failure`] with the description of `agent`and `fault` that caused the failure.
    pub(crate) fn new<A>(agent: &A, fault: Fault<F>) -> Self
    where
        A: Display,
    {
        Self {
            agent_description: agent.to_string(),
            fault,
        }
    }

    /// Returns if `self` was caused by a defect.
    pub fn is_defect(&self) -> bool {
        self.fault.is_defect()
    }

    /// If `self` is a defect, converts the defect into `W::Defect`; otherwise returns `self`.
    pub fn map_defect<M, W>(self, m: M) -> Failure<W>
    where
        M: FnMut(F::Defect) -> W::Defect,
        W: Flaws<Insufficiency = F::Insufficiency>,
    {
        Failure {
            agent_description: self.agent_description,
            fault: self.fault.map_defect(m),
        }
    }
}

impl<F, W> Blame<Failure<W>> for Failure<F>
where
    F: Flaws,
    W: Flaws,
    W::Insufficiency: From<F::Insufficiency>,
    W::Defect: From<F::Defect>,
{
    fn blame(self) -> Failure<W> {
        Failure {
            agent_description: self.agent_description,
            fault: self.fault.blame(),
        }
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
            .field("agent_description", &self.agent_description)
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
        write!(f, "{}: {}", self.agent_description, self.fault)
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
        self.agent_description == other.agent_description && self.fault == other.fault
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
            Ok(fault) => Failure {
                agent_description: self.agent_description,
                fault,
            },
            Err(error) => throw!(FailureConversionError {
                error,
                agent_description: self.agent_description
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

/// The error thrown when a chain from a [`Consumer`] to a [`Producer`] fails to produce a good.
#[non_exhaustive]
pub enum Blockage<C, P, G>
where
    C: Flaws,
    P: Flaws,
{
    /// The action failed due to a failure during consumption.
    Consumption(Failure<C>),
    /// The action failed due to a failure during production.
    Production(Recall<P, G>),
}

impl<C, P, G> Debug for Blockage<C, P, G>
where
    C: Flaws,
    C::Insufficiency: Debug,
    C::Defect: Debug,
    P: Flaws,
    P::Insufficiency: Debug,
    P::Defect: Debug,
    G: Debug,
{
    /// Writes the default debug format for `self`.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Consumption(ref failure) => {
                write!(f, "Blockage::Consumption({:?})", failure)
            }
            Self::Production(ref recall) => write!(f, "Blockage::Production({:?})", recall),
        }
    }
}

impl<C, P, G> From<Failure<C>> for Blockage<C, P, G>
where
    C: Flaws,
    P: Flaws,
{
    fn from(failure: Failure<C>) -> Self {
        Self::Consumption(failure)
    }
}

impl<C, P, G> From<Recall<P, G>> for Blockage<C, P, G>
where
    C: Flaws,
    P: Flaws,
{
    fn from(recall: Recall<P, G>) -> Self {
        Self::Production(recall)
    }
}

impl<C, P, G> PartialEq for Blockage<C, P, G>
where
    C: Flaws,
    C::Insufficiency: PartialEq,
    C::Defect: PartialEq,
    P: Flaws,
    P::Insufficiency: PartialEq,
    P::Defect: PartialEq,
    G: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match *self {
            Self::Consumption(ref my_failure) => {
                if let Blockage::Consumption(ref their_failure) = *other {
                    my_failure == their_failure
                } else {
                    false
                }
            }
            Self::Production(ref my_recall) => {
                if let Blockage::Production(ref their_recall) = *other {
                    my_recall == their_recall
                } else {
                    false
                }
            }
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
    agent_description: String,
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
            .field("agent_description", &self.agent_description)
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
        write!(f, "in `{}`: {}", self.agent_description, self.error)
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
