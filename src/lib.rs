//! Defines interfaces used by producers and consumers in a market.
//!
//! A market is a stock of goods that can have agents act upon it. An agent can be either a [`Producer`] that stores goods into the market or a [`Consumer`] that retrieves goods from the market. While agents are acting upon a market, they SHALL be immutable.
pub use market_derive::{ConsumeFault, ProduceFault};

use {
    core::{
        convert::{Infallible, TryFrom, TryInto},
        fmt::{self, Debug, Display, Formatter},
        iter::{self, Chain, Once},
        task::Poll,
    },
    fehler::{throw, throws},
    std::error::Error,
};

/// Characterizes the failure of an agent to successfully complete an action upon a market.
///
/// All errors thrown by an agent to indicate an action failed SHALL implement [`Failure`]. Each error type that implements [`Failure`] can have multiple instances and each instance SHALL match only 1 of 2 distinct categories:
///     1. Insufficient stock: Thrown when either there is no more room in the stock for the good from a [`Producer`] or the stock is empty and unable to provide a good to a [`Consumer`].
///     2. Faults: Thrown when the storage or retrieval mechanism of the market throws an error.
///
/// Another possible model for the return item of an action could be to only throw an error when a fault is caught and return an item like [`std::task::Poll`] that indicated either success or insufficient stock. The issue with this model is that it does not allow for specifying agents that are infallible.
pub trait Failure: Sized {
    /// Specifies all faults that can be thrown by `Self`.
    ///
    /// Given `failure` of type `F` that implements [`Failure`], if `F::Fault::try_from(failure)` throws an error, then `failure` must be caused by an insufficient stock.
    type Fault: TryFrom<Self>;
}

impl Failure for Infallible {
    type Fault = Self;
}

/// Multiple goods that were not produced along with the error that caused the production failure.
#[derive(Debug)]
pub struct Recall<G, I, E> {
    /// The goods that were not produced.
    goods: Chain<Once<G>, I>,
    /// The error that was caught.
    error: E,
}

impl<G, I: Iterator<Item = G>, E> Recall<G, I, E> {
    /// Creates a new [`Recall`] with `other_goods` appended after `first_good`.
    fn new<U: IntoIterator<Item = G, IntoIter = I>>(
        first_good: G,
        other_goods: U,
        error: E,
    ) -> Self {
        Self {
            goods: iter::once(first_good).chain(other_goods.into_iter()),
            error,
        }
    }

    /// Converts `self` into its chain of goods and error.
    #[inline]
    pub fn redeem(self) -> (Chain<Once<G>, I>, E) {
        (self.goods, self.error)
    }
}

impl<G, I, E: Display> Display for Recall<G, I, E> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to produce an iteration of goods due to: {}",
            self.error
        )
    }
}

impl<G: Debug, I: Debug, E: Debug + Display> Error for Recall<G, I, E> {}

/// A good that was not produced along with the error that caused the production failure.
#[derive(Debug)]
pub struct Return<G, E> {
    /// The good that was not produced.
    good: G,
    /// The error that caused the failure.
    error: E,
}

impl<G, E> Return<G, E> {
    /// Creates a new [`Return`].
    #[inline]
    pub const fn new(good: G, error: E) -> Self {
        Self { good, error }
    }

    /// Converts `self` into its good and error.
    #[allow(clippy::missing_const_for_fn)] // False negative.
    #[inline]
    pub fn redeem(self) -> (G, E) {
        (self.good, self.error)
    }

    /// Converts `self` into a [`Recall`] with the goods in `other` appended after the good in `self`.
    fn chain<I: IntoIterator<Item = G>>(self, other: I) -> Recall<G, I::IntoIter, E> {
        let (good, error) = self.redeem();
        Recall::new(good, other, error)
    }
}

impl<G: Display, E: Display> Display for Return<G, E> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to produce `{}` due to: {}",
            self.good, self.error
        )
    }
}

impl<G: Debug + Display, E: Debug + Display> Error for Return<G, E> {}

/// Characterizes an agent that stores goods into a market.
pub trait Producer {
    /// Specifies the good being produced.
    type Good;
    /// Specifies the error thrown when a production attempt fails.
    type Failure: Failure;

    /// Stores `good` into the market without blocking.
    ///
    /// # Errors
    ///
    /// All caught errors MUST be converted into a [`Self::Failure`]. Then `produce()` shall throw a [`Return`] created from `good` and the failure.
    fn produce(&self, good: Self::Good) -> Result<(), Return<Self::Good, Self::Failure>>;

    /// Stores each good in `iteration` into the market without blocking.
    ///
    /// # Errors
    ///
    /// If a [`Return`] is caught, SHALL throw a [`Recall`] created by chaining all remaining goods in `iteration` onto the good in [`Return`].
    #[inline]
    #[throws(Recall<Self::Good, I::IntoIter, Self::Failure>)]
    fn produce_all<I: IntoIterator<Item = Self::Good>>(&self, iteration: I) {
        let mut goods = iteration.into_iter();

        while let Some(good) = goods.next() {
            if let Err(r) = self.produce(good) {
                throw!(r.chain(goods));
            }
        }
    }

    /// Stores `good` into the market, blocking until space is available.
    ///
    /// # Errors
    ///
    /// If a fault is caught, SHALL throw a [`Recall`] with the fault and the good.
    #[inline]
    #[throws(Return<Self::Good, <Self::Failure as Failure>::Fault>)]
    fn force(&self, mut good: Self::Good) {
        while let Err((failed_good, error)) = self.produce(good).map_err(Return::redeem) {
            if let Ok(fault) = error.try_into() {
                throw!(Return::new(failed_good, fault,))
            }

            good = failed_good;
        }
    }

    /// Stores each good in `composite`, blocking until space is available.
    ///
    /// # Errors
    ///
    /// If a fault is caught, SHALL throw a [`Recall`] with the fault and all goods that were not successfully produced.
    #[inline]
    #[throws(Recall<Self::Good, I::IntoIter, <Self::Failure as Failure>::Fault>)]
    fn force_all<I: IntoIterator<Item = Self::Good>>(&self, composite: I) {
        let mut goods = composite.into_iter();

        while let Some(good) = goods.next() {
            if let Err(r) = self.force(good) {
                throw!(r.chain(goods));
            }
        }
    }
}

/// An [`Iterator`] of the goods consumed by a [`Consumer`].
#[derive(Debug)]
pub struct Goods<'a, C: Consumer> {
    /// The [`Consumer`].
    consumer: &'a C,
}

impl<C: Consumer> Iterator for Goods<'_, C> {
    type Item = Result<<C as Consumer>::Good, <<C as Consumer>::Failure as Failure>::Fault>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.consumer.consume() {
            Ok(good) => Some(Ok(good)),
            Err(failure) => failure.try_into().ok().map(Err),
        }
    }
}

/// Characterizes the construction of a item from a sequence of `P` items.
pub trait Builder<P> {
    /// Specifies the composite to be built.
    type Output;
    /// Specifies the error thrown when a build fails.
    type Error;

    /// Builds a composite item of type `Self::Output` from `parts`.
    ///
    /// If a composite is built, the parts used for the build are removed from `parts`. If `parts` is the start of a valid sequence of parts but requires more elements to create a composite, SHALL return [`Poll::Pending`].
    ///
    /// # Errors
    ///
    /// If `parts` are not the start of a valid sequence, throws `Self::Error`.
    #[throws(Self::Error)]
    fn build(&self, parts: &mut Vec<P>) -> Poll<Self::Output>;
}

/// Collects parts of type `P` that can be built into composites.
#[derive(Debug)]
pub struct Composer<P, B> {
    /// The parts that make up composites.
    parts: Vec<P>,
    /// The [`Builder`] of composites.
    builder: B,
}

impl<P, B: Builder<P>> Composer<P, B> {
    /// Creates a new [`Composer`].
    #[inline]
    pub fn new(builder: B) -> Self {
        Self {
            parts: Vec::new(),
            builder,
        }
    }

    /// Adds `parts` to the parts stored in `self`.
    fn append(&mut self, mut parts: Vec<P>) {
        self.parts.append(&mut parts);
    }

    /// Builds a composite from the parts stored in `self`.
    ///
    /// # Errors
    ///
    /// If error is caught during build, the error shall be thrown.
    #[throws(B::Error)]
    fn build(&mut self) -> Poll<B::Output> {
        self.builder.build(&mut self.parts)?
    }
}

/// An error thrown when a [`Consumer`] fails to build a composite from its goods.
#[allow(clippy::exhaustive_enums)] // It is intended that these shall be the only variants in ComposeError.
#[derive(Debug, PartialEq)]
pub enum ComposeError<T, E> {
    /// The [`Consumer`] failed consuming goods.
    Consume(T),
    /// The [`Composer`] failed generating a composite.
    Build(E),
}

/// Characterizes an agent that retrieves goods from a market.
///
/// The order in which goods are retrieved is defined by the implementer.
pub trait Consumer {
    /// Specifies the good being consumed.
    type Good;
    /// Specifies the [`Failure`] thrown when a consumption fails.
    type Failure: Failure;

    /// Retrieves the next good from the market without blocking.
    ///
    /// # Errors
    ///
    /// SHALL throw a [`Self::Failure`] if no good can be consumed.
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good;

    /// Returns a [`Goods`] of `self`.
    #[inline]
    fn goods(&self) -> Goods<'_, Self>
    where
        Self: Sized,
    {
        Goods { consumer: self }
    }

    /// Appends all current goods from the market to `composer` and returns the generated composite.
    ///
    /// # Errors
    ///
    /// If a fault is caught while consuming, SHALL throw a [`ComposeError::Consume`]. If an error is caught while composing, SHALL throw a [`ComposeError::Build`]. In both cases, successfully consumed goods are appended to `composer`.
    #[inline]
    #[throws(ComposeError<<Self::Failure as Failure>::Fault, B::Error>)]
    fn compose<B: Builder<Self::Good>>(
        &self,
        composer: &mut Composer<Self::Good, B>,
    ) -> Poll<B::Output> {
        let mut goods = Vec::new();

        // Consume until a failure while keeping all the successfully consumed goods.
        let failure = loop {
            match self.consume() {
                Ok(good) => {
                    goods.push(good);
                }
                Err(failure) => {
                    break failure;
                }
            }
        };
        composer.append(goods);

        if let Ok(fault) = failure.try_into() {
            throw!(ComposeError::Consume(fault));
        }

        composer.build().map_err(ComposeError::Build)?
    }

    /// Retrieves the next good from the market, blocking until one is available.
    ///
    /// # Errors
    ///
    /// If a fault is caught, SHALL throw the fault.
    #[inline]
    #[throws(<Self::Failure as Failure>::Fault)]
    fn demand(&self) -> Self::Good {
        loop {
            match self.consume() {
                Ok(good) => {
                    break good;
                }
                Err(failure) => {
                    if let Ok(fault) = failure.try_into() {
                        throw!(fault);
                    }
                }
            }
        }
    }
}

/// The [`Failure`] thrown when a [`Consumer`] can have an empty stock but cannot catch a fault.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("stock is empty")]
#[non_exhaustive]
pub struct EmptyStockFailure;

impl EmptyStockFailure {
    /// Creates a new [`EmptyStockFailure`].
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Failure for EmptyStockFailure {
    type Fault = Infallible;
}

// Required by EmptyStockFailure: Failure.
impl TryFrom<EmptyStockFailure> for Infallible {
    type Error = ();

    #[inline]
    fn try_from(_: EmptyStockFailure) -> Result<Self, Self::Error> {
        Err(())
    }
}

/// The [`Failure`] thrown when a [`Producer`] can have a full stock but cannot catch a fault.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("stock is full")]
#[non_exhaustive]
pub struct FullStockFailure;

impl FullStockFailure {
    /// Creates a new [`FullStockFailure`].
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Failure for FullStockFailure {
    type Fault = Infallible;
}

// Required by FullStockFailure: Failure.
impl TryFrom<FullStockFailure> for Infallible {
    type Error = ();

    #[inline]
    fn try_from(_: FullStockFailure) -> Result<Self, Self::Error> {
        Err(())
    }
}

/// The typical [`Failure`] thrown when a [`Consumer`] is unable to consume a good.
///
/// This SHOULD be used in all cases where a [`Consumer`] can catch a fault of type `T` and can fail due to the stock being empty.
#[allow(clippy::exhaustive_enums)] // It is intended that these shall be the only variants in ConsumeFailure.
#[derive(Clone, Debug)]
pub enum ConsumeFailure<T> {
    /// The stock of the market is empty.
    EmptyStock,
    /// Fault `T` was caught during consumption.
    Fault(T),
}

#[allow(clippy::use_self)] // False positives.
impl<T> ConsumeFailure<T> {
    /// Converts a [`ConsumeFailure<F>`] into a [`ConsumeFailure<T>`].
    #[inline]
    pub fn map_fault<F: Into<T>>(failure: ConsumeFailure<F>) -> Self {
        if let ConsumeFailure::Fault(fault) = failure {
            Self::Fault(fault.into())
        } else {
            Self::EmptyStock
        }
    }
}

impl<T: TryFrom<Self>> Failure for ConsumeFailure<T> {
    type Fault = T;
}

impl<T: Display> Display for ConsumeFailure<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match *self {
            Self::EmptyStock => write!(f, "stock is empty"),
            Self::Fault(ref fault) => write!(f, "{}", fault),
        }
    }
}

impl<T: Debug + Display> Error for ConsumeFailure<T> {}

impl<T> From<EmptyStockFailure> for ConsumeFailure<T> {
    #[inline]
    fn from(_: EmptyStockFailure) -> Self {
        Self::EmptyStock
    }
}

/// The typical [`Failure`] thrown when a [`Producer`] is unable to produce a good.
///
/// This SHOULD be used in all cases where a [`Producer`] can catch a fault of type `T` and can fail due to the stock being full.
#[allow(clippy::exhaustive_enums)] // It is intended that these shall be the only variants in ProduceFailure.
#[derive(Clone, Debug, PartialEq)]
pub enum ProduceFailure<T> {
    /// The stock of the market is full.
    FullStock,
    /// Fault `T` was caught during production.
    Fault(T),
}

#[allow(clippy::use_self)] // False positives.
impl<T> ProduceFailure<T> {
    /// Converts a [`ProduceFailure<F>`] into a [`ProduceFailure<T>`].
    #[inline]
    pub fn map_fault<F: Into<T>>(failure: ProduceFailure<F>) -> Self {
        if let ProduceFailure::Fault(fault) = failure {
            Self::Fault(fault.into())
        } else {
            Self::FullStock
        }
    }
}

impl<T> Default for ProduceFailure<T> {
    #[inline]
    fn default() -> Self {
        Self::FullStock
    }
}

impl<T: TryFrom<Self>> Failure for ProduceFailure<T> {
    type Fault = T;
}

impl<T: Display> Display for ProduceFailure<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match *self {
            Self::FullStock => write!(f, "stock is full"),
            Self::Fault(ref fault) => write!(f, "{}", fault),
        }
    }
}

impl<T: Debug + Display> Error for ProduceFailure<T> {}

impl<T> From<T> for ProduceFailure<T> {
    #[inline]
    fn from(fault: T) -> Self {
        Self::Fault(fault)
    }
}
