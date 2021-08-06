//! Defines interfaces used by [`Agent`]s to act upon a market.
//!
//! An [`Agent`] can be either a [`Producer`] that stores goods into the market or a [`Consumer`] that retrieves goods from the market. While an [`Agent`] is acting upon a market, it is immutable.

// Add unstable feature to document when items are supported.
#![cfg_attr(feature = "unstable-doc-cfg", feature(doc_cfg))]
#![no_std]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod error;

pub use error::{
    Blame, Blockage, ConsumptionFlaws, EmptyStock, Failure, FailureConversionError, Fault,
    FaultConversionError, Flawless, Flaws, FullStock, ProductionFlaws, Recall,
    RecallConversionError, TryBlame,
};

use {
    core::{convert::TryFrom, fmt::Display},
    fehler::{throw, throws},
};

/// Characterizes an agent that interacts with a market.
// Agent does not define Flaws type because an Agent that implements both Producer and Consumer (such as a queue) may have different Flaws for each trait.
pub trait Agent {
    /// Specifies the good that is stored in the market.
    type Good;
}

/// Characterizes an agent that stores goods into a market.
pub trait Producer: Agent {
    /// Specifies the [`Flaws`] thrown when a production fails.
    type Flaws: Flaws;

    /// Returns the [`Recall`] thrown by `self` when `fault` is caught while producing `good`.
    fn recall(&self, fault: Fault<Self::Flaws>, good: Self::Good) -> Recall<Self::Flaws, Self::Good>
    where
        Self: Display,
    {
        Recall::new(Failure::new(&self, fault), good)
    }

    /// Stores `good` into the market without blocking.
    ///
    /// # Errors
    ///
    /// If `produce` fails to store `good` into the market, it shall throw a [`Recall`] containing the [`Fault`] and `good`.
    fn produce(&self, good: Self::Good) -> Result<(), Recall<Self::Flaws, Self::Good>>;

    /// Stores each good from the [`Iterator`] `goods` into the market without blocking.
    ///
    /// # Errors
    ///
    /// If the production of a good fails, shall throw a [`Recall`] and `goods` shall contain all goods whose production was not attempted.
    #[throws(Recall<Self::Flaws, Self::Good>)]
    fn produce_all<I>(&self, goods: &mut I)
    where
        // Required for Producer to be object safe: See https://doc.rust-lang.org/reference/items/traits.html#object-safety.
        Self: Sized,
        I: Iterator<Item = Self::Good>,
    {
        for good in goods {
            self.produce(good)?;
        }
    }

    /// Retrieves each good from the [`Consumer`] `consumer` and stores it into the market without blocking.
    ///
    /// # Errors
    ///
    /// If the consumption or production of a good fails, except in the case where consumption fails due to an insufficiency after at least one successful consumption, `produce_goods` shall throw a [`Blockage`] and `consumer` shall contain all goods whose production was not attempted.
    #[throws(Blockage<C::Flaws, Self::Flaws, Self::Good>)]
    fn produce_goods<C>(&self, consumer: &C)
    where
        // Required for Producer to be object safe: See https://doc.rust-lang.org/reference/items/traits.html#object-safety.
        Self: Sized,
        C: Consumer<Good = Self::Good>,
    {
        // Throw any consumer error on the first attempt; after this only throw defects.
        self.produce(consumer.consume()?)?;

        let failure = loop {
            match consumer.consume() {
                Ok(good) => self.produce(good)?,
                Err(failure) => break failure,
            }
        };

        if failure.is_defect() {
            throw!(failure);
        }
    }

    /// Stores `good` into the market, blocking until stock is available.
    ///
    /// # Errors
    ///
    /// If the production fails due to a defect, `force` shall throw a [`Recall`] containing the [`Fault`] and `good`.
    #[throws(Recall<<Self::Flaws as Flaws>::Defect, Self::Good>)]
    fn force(&self, mut good: Self::Good)
    where
        // Indicates that Self::Flaws::Defect implements Flaws with itself as the Defect.
        <Self::Flaws as Flaws>::Defect: Flaws<Defect = <Self::Flaws as Flaws>::Defect>,
        <<Self::Flaws as Flaws>::Defect as Flaws>::Insufficiency:
            TryFrom<<Self::Flaws as Flaws>::Insufficiency>,
    {
        while let Err(recall) = self.produce(good) {
            match recall.try_blame() {
                Ok(defect) => throw!(defect),
                Err(error) => {
                    good = error.into_good();
                }
            }
        }
    }

    /// Stores each good from the [`Iterator`] `goods` into the market, blocking until stock is available.
    ///
    /// # Errors
    ///
    /// If the production of a good fails, `force_all` shall throw a [`Recall`] and `goods` shall contain all goods whose production was not attempted.
    #[throws(Recall<<Self::Flaws as Flaws>::Defect, Self::Good>)]
    fn force_all<I>(&self, goods: &mut I)
    where
        // Required for Producer to be object safe: See https://doc.rust-lang.org/reference/items/traits.html#object-safety.
        Self: Sized,
        I: Iterator<Item = Self::Good>,
        <Self::Flaws as Flaws>::Defect: Flaws<Defect = <Self::Flaws as Flaws>::Defect>,
        <<Self::Flaws as Flaws>::Defect as Flaws>::Insufficiency:
            TryFrom<<Self::Flaws as Flaws>::Insufficiency>,
    {
        for good in goods {
            self.force(good)?;
        }
    }

    /// Retrieves and stores goods from `consumer` into the market, blocking both until stock is sufficient for the respective action.
    ///
    /// # Errors
    ///
    /// If the consumption or production of a good fails due to a defect, `force_all` shall throw a [`Blockage`] and `consumer` shall contain all goods whose production was not attempted.
    #[allow(unreachable_code)] // Issue with fehler (#53) which has been resolved but not released.
    #[throws(Blockage<<C::Flaws as Flaws>::Defect, <Self::Flaws as Flaws>::Defect, Self::Good>)]
    fn force_goods<C>(&self, consumer: &C)
    where
        // Required for Producer to be object safe: See https://doc.rust-lang.org/reference/items/traits.html#object-safety.
        Self: Sized,
        C: Consumer<Good = Self::Good>,
        <C::Flaws as Flaws>::Defect: Flaws<Defect = <C::Flaws as Flaws>::Defect>,
        <<C::Flaws as Flaws>::Defect as Flaws>::Insufficiency:
            TryFrom<<C::Flaws as Flaws>::Insufficiency>,
        <Self::Flaws as Flaws>::Defect: Flaws<Defect = <Self::Flaws as Flaws>::Defect>,
        <<Self::Flaws as Flaws>::Defect as Flaws>::Insufficiency:
            TryFrom<<Self::Flaws as Flaws>::Insufficiency>,
    {
        loop {
            self.force(consumer.demand()?)?;
        }
    }
}

/// Characterizes an agent that retrieves goods from a market.
///
/// The order in which goods are retrieved is defined by the implementer.
pub trait Consumer: Agent {
    /// Specifies the [`Flaws`] thrown when a consumption fails.
    type Flaws: Flaws;

    /// Returns the [`Failure`] thrown by `self` when `fault` is caught.
    fn failure(&self, fault: Fault<Self::Flaws>) -> Failure<Self::Flaws>
    where
        Self: Display,
    {
        Failure::new(&self, fault)
    }

    /// Retrieves the next good from the market without blocking.
    ///
    /// # Errors
    ///
    /// If `consume` fails to retrieve `good` from the market, it shall throw the causing [`Failure`].
    #[throws(Failure<Self::Flaws>)]
    fn consume(&self) -> Self::Good;

    /// Retrieves the next good from the market, blocking until one is available.
    ///
    /// # Errors
    ///
    /// If the consumption fails due to a defect, `demand` shall throw the appropriate [`Failure`].
    #[throws(Failure<<Self::Flaws as Flaws>::Defect>)]
    fn demand(&self) -> Self::Good
    where
        // Indicates that Self::Flaws::Defect implements Flaws with itself as the Defect.
        <Self::Flaws as Flaws>::Defect: Flaws<Defect = <Self::Flaws as Flaws>::Defect>,
        <<Self::Flaws as Flaws>::Defect as Flaws>::Insufficiency:
            TryFrom<<Self::Flaws as Flaws>::Insufficiency>,
    {
        loop {
            match self.consume() {
                Ok(good) => {
                    break good;
                }
                Err(failure) => {
                    if let Ok(defect) = failure.try_blame() {
                        throw!(defect);
                    }
                }
            }
        }
    }
}

/// Defines traits of markets for a channel.
///
/// A channel exchanges goods between [`Producer`]s and [`Consumer`]s. If either all [`Consumer`]s or all [`Producer`]s for a channel are dropped, the channel becomes invalid.
pub mod channel {
    use {
        super::{Consumer, ConsumptionFlaws, Flawless, Flaws, Producer, ProductionFlaws},
        core::fmt::{self, Display, Formatter},
    };

    /// The defect thrown when a [`Producer`] attempts to produce to a channel with no [`Consumer`]s.
    #[derive(Clone, Copy, Debug, Default)]
    #[non_exhaustive]
    pub struct WithdrawnDemand;

    impl Display for WithdrawnDemand {
        /// Writes "demand has withdrawn".
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "demand has withdrawn")
        }
    }

    #[cfg(feature = "std")]
    #[cfg_attr(feature = "unstable-doc-cfg", doc(cfg(feature = "std")))]
    impl std::error::Error for WithdrawnDemand {}

    impl Flaws for WithdrawnDemand {
        type Insufficiency = Flawless;
        type Defect = Self;
    }

    /// The defect thrown when a [`Consumer`] attempts to consume from an empty channel with no [`Producer`]s.
    #[derive(Clone, Copy, Debug, Default)]
    #[non_exhaustive]
    pub struct WithdrawnSupply;

    impl Display for WithdrawnSupply {
        /// Writes "supply has withdrawn".
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "supply has withdrawn")
        }
    }

    #[cfg(feature = "std")]
    #[cfg_attr(feature = "unstable-doc-cfg", doc(cfg(feature = "std")))]
    impl std::error::Error for WithdrawnSupply {}

    impl Flaws for WithdrawnSupply {
        type Insufficiency = Flawless;
        type Defect = Self;
    }

    /// Characterizes a channel with infinite capacity.
    pub trait InfiniteChannel<G> {
        /// Specifies the [`Producer`].
        type Producer: Producer<Good = G, Flaws = WithdrawnDemand>;
        /// Specifies the [`Consumer`].
        type Consumer: Consumer<Good = G, Flaws = ConsumptionFlaws<WithdrawnSupply>>;

        /// Creates the [`Producer`] and [`Consumer`] connected to an infinite channel.
        fn establish<S>(name_str: &S) -> (Self::Producer, Self::Consumer)
        where
            S: AsRef<str> + ?Sized;
    }

    /// Characterizes a channel with a limited capacity.
    pub trait FiniteChannel<G> {
        /// Specifies the [`Producer`].
        type Producer: Producer<Good = G, Flaws = ProductionFlaws<WithdrawnDemand>>;
        /// Specifies the [`Consumer`].
        type Consumer: Consumer<Good = G, Flaws = ConsumptionFlaws<WithdrawnSupply>>;

        /// Creates the [`Producer`] and [`Consumer`] connected to a channel with capacity of `size`.
        fn establish<S>(name_str: &S, size: usize) -> (Self::Producer, Self::Consumer)
        where
            S: AsRef<str> + ?Sized;
    }
}

/// Defines traits of markets for a queue.
///
/// A queue is a single item that implements [`Producer`] and [`Consumer`]. As a result, storing and retrieving from a queue cannot cause a defect.
pub mod queue {
    use super::{Consumer, EmptyStock, Flawless, FullStock, Producer};

    /// Characterizes a queue with infinite size.
    pub trait InfiniteQueue<G>:
        Consumer<Good = G, Flaws = EmptyStock> + Producer<Good = G, Flaws = Flawless>
    {
        /// Creates a queue with infinite size.
        fn allocate<S>(name_str: &S) -> Self
        where
            S: AsRef<str> + ?Sized;
    }

    /// Characterizes a queue with a size.
    pub trait FiniteQueue<G>:
        Consumer<Good = G, Flaws = EmptyStock> + Producer<Good = G, Flaws = FullStock>
    {
        /// Creates a queue with finite size.
        fn allocate<S>(name_str: &S, size: usize) -> Self
        where
            S: AsRef<str> + ?Sized;
    }
}
