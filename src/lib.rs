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
    Blame, ConsumptionFlaws, EmptyStock, Failure, FailureConversionError, Fault,
    FaultConversionError, Flawless, Flaws, FullStock, ProductionFlaws, Recall,
    RecallConversionError, TryBlame,
};

use {
    alloc::string::String,
    core::convert::TryFrom,
    fehler::{throw, throws},
};

/// Characterizes an agent that interacts with a market.
pub trait Agent {
    /// Specifies the good that is stored in the market.
    type Good;

    /// Returns a [`String`] that identifies `self`.
    fn name(&self) -> String;

    /// Returns the [`Failure`] thrown by `self` when `fault` is caught.
    fn failure<F>(&self, fault: Fault<F>) -> Failure<F>
    where
        F: Flaws,
    {
        Failure::new(fault, self.name())
    }
}

/// Characterizes an agent that stores goods into a market.
pub trait Producer: Agent {
    /// Specifies the [`Flaws`] thrown when a production fails.
    type Flaws: Flaws;

    /// Returns the [`Recall`] thrown by `self` when `fault` is caught while producing `good`.
    fn recall(
        &self,
        fault: Fault<Self::Flaws>,
        good: Self::Good,
    ) -> Recall<Self::Flaws, Self::Good> {
        Recall::new(self.failure(fault), good)
    }

    /// Stores `good` into the market without blocking.
    ///
    /// # Errors
    ///
    /// If `produce` fails to store `good` into the market, it shall throw a [`Recall`] containing the [`Fault`] and `good`.
    fn produce(&self, good: Self::Good) -> Result<(), Recall<Self::Flaws, Self::Good>>;

    /// Retrieves and stores goods from `consumer` into the market without blocking.
    ///
    /// # Errors
    ///
    /// If the production of a good fails, `produce_consumptions` shall throw a [`Recall`] with all goods  in `goods` that were not produced.
    #[throws(Recall<Self::Flaws, Self::Good>)]
    fn produce_consumptions<C>(&self, consumer: &C)
    where
        // Required for Producer to be object safe: See https://doc.rust-lang.org/reference/items/traits.html#object-safety.
        Self: Sized,
        C: Consumer<Good = Self::Good>,
    {
        while let Ok(good) = consumer.consume() {
            self.produce(good)?;
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

    /// Retrieves and stores goods from `consumer` into the market, blocking storage until stock is available.
    ///
    /// # Errors
    ///
    /// If the production of a good fails due to a defect, `force_consumptions` shall throw a [`Recall`] with all goods  in `goods` that were not produced.
    #[throws(Recall<<Self::Flaws as Flaws>::Defect, Self::Good>)]
    fn force_consumptions<C>(&self, consumer: &C)
    where
        // Required for Producer to be object safe: See https://doc.rust-lang.org/reference/items/traits.html#object-safety.
        Self: Sized,
        C: Consumer<Good = Self::Good>,
        // Indicates that Self::Flaws::Defect implements Flaws with itself as the Defect.
        <Self::Flaws as Flaws>::Defect: Flaws<Defect = <Self::Flaws as Flaws>::Defect>,
        <<Self::Flaws as Flaws>::Defect as Flaws>::Insufficiency:
            TryFrom<<Self::Flaws as Flaws>::Insufficiency>,
    {
        while let Ok(good) = consumer.consume() {
            self.force(good)?;
        }
    }
}

/// Characterizes an agent that retrieves goods from a market.
///
/// The order in which goods are retrieved is defined by the implementer.
pub trait Consumer: Agent {
    /// Specifies the [`Flaws`] thrown when a consumption fails.
    type Flaws: Flaws;

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
        /// Specifies the arguments used for creating the channel.
        type Args;

        /// Creates the [`Producer`] and [`Consumer`] connected to an infinite channel.
        fn establish(args: Self::Args) -> (Self::Producer, Self::Consumer);
    }

    /// Characterizes a channel with a limited capacity.
    pub trait FiniteChannel<G> {
        /// Specifies the [`Producer`].
        type Producer: Producer<Good = G, Flaws = ProductionFlaws<WithdrawnDemand>>;
        /// Specifies the [`Consumer`].
        type Consumer: Consumer<Good = G, Flaws = ConsumptionFlaws<WithdrawnSupply>>;
        /// Specifies the arguments used for creating the channel.
        type Args;

        /// Creates the [`Producer`] and [`Consumer`] connected to a channel with capacity of `size`.
        fn establish(args: Self::Args, size: usize) -> (Self::Producer, Self::Consumer);
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
        /// Specifies the arguments used for creating the queue.
        type Args;

        /// Creates a queue with infinite size.
        fn allocate(args: Self::Args) -> Self;
    }

    /// Characterizes a queue with a size.
    pub trait FiniteQueue<G>:
        Consumer<Good = G, Flaws = EmptyStock> + Producer<Good = G, Flaws = FullStock>
    {
        /// Specifies the arguments used for creating the queue.
        type Args;

        /// Creates a queue with finite size.
        fn allocate(args: Self::Args, size: usize) -> Self;
    }
}
