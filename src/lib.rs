//! Infrastructure for producers and consumers in a market.
//!
//! A market is a stock of goods that can have agents act upon it. An agent can be either a [`Producer`] that stores goods into the market or a [`Consumer`] that retrieves goods from the market. The important thing to note about agents is that they are immutable during their respective actions.

// Use of market in derive macros requires defining crate as market.
extern crate self as market;

pub mod channel;
mod error;
pub mod io;
mod map;
pub mod process;
pub mod queue;
pub mod sync;
pub mod thread;
pub mod vec;

pub use {
    error::{ConsumeFailure, Failure, InsufficientStockFailure, ProduceFailure},
    market_derive::{ConsumeFault, ProduceFault},
};

use {
    core::fmt::Debug,
    fehler::{throw, throws},
};

/// Specifies the storage of goods into a market.
#[allow(clippy::missing_inline_in_public_items)] // current issue with fehler for produce(); see https://github.com/withoutboats/fehler/issues/39
pub trait Producer {
    /// The item being produced.
    type Good;
    /// Describes a failure to successfully complete production.
    type Failure: Failure;

    /// Stores `good` in the market without blocking.
    ///
    /// SHALL only run in the calling process and return without blocking.
    ///
    /// # Errors
    ///
    /// If fault `T` is caught, SHALL throw [`Self::Failure`] `F` such that `F.fault() == Some(T)`. If `self` cannot store `good` without blocking, SHALL throw an appropriate [`Self::Failure`].
    #[allow(redundant_semicolons, unused_variables)] // current issue with fehler; see https://github.com/withoutboats/fehler/issues/39
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good);

    /// Stores each good in `goods` in the market without blocking.
    ///
    /// # Errors
    ///
    /// If [`Failure`] `F` is caught, SHALL attempt no further goods and throw `F`.
    #[throws(Self::Failure)]
    fn produce_all(&self, goods: Vec<Self::Good>) {
        for good in goods {
            self.produce(good)?
        }
    }

    /// Stores `good` in the market, blocking until space is available.
    ///
    /// # Errors
    ///
    /// If fault `T` is caught, SHALL throw `T`
    #[inline]
    #[throws(<Self::Failure as Failure>::Fault)]
    fn force(&self, good: Self::Good)
    where
        Self::Good: Clone + Debug,
    {
        while let Err(failure) = self.produce(good.clone()) {
            if let Some(fault) = failure.fault() {
                throw!(fault)
            }
        }
    }

    /// Stores every good in `goods`, blocking until space is available.
    ///
    /// # Errors
    ///
    /// If fault `T` is caught, SHALL attempt no further goods and throw `T`.
    #[throws(<Self::Failure as Failure>::Fault)]
    fn force_all(&self, goods: Vec<Self::Good>)
    where
        Self::Good: Clone + Debug,
    {
        for good in goods {
            self.force(good)?
        }
    }
}

/// Retrieves goods from a market.
///
/// The order in which goods are retrieved is defined by the implementer.
pub trait Consumer {
    /// The item being consumed.
    type Good;
    /// Describes a failure to successfully complete consumption.
    type Failure: Failure;

    /// Retrieves the next good from the market without blocking.
    ///
    /// SHALL only run in the calling process and return the next good in the market without blocking.
    ///
    /// # Errors
    ///
    /// If fault `T` is caught, SHALL throw [`Self::Failure`] `F` such that `F.fault() == Some(T)`. If `self` cannot retrieve a good without blocking, SHALL throw an appropriate [`Self::Failure`].
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good;

    /// Retrieves all goods in the market without blocking.
    ///
    /// If assembly fails, the cause of the failure SHALL be thrown and `parts` SHALL NOT be modified.
    ///
    /// # Errors
    ///
    /// If a fault `T` is caught prior to retrieving a good, SHALL throw `T`. If fault is caught after 1 or more goods have been retrieved, the fault is ignored and SHALL return the retrieved goods.
    #[inline]
    #[throws(<Self::Failure as Failure>::Fault)]
    fn consume_all(&self) -> Vec<Self::Good> {
        let mut goods = Vec::new();

        loop {
            match self.consume() {
                Ok(good) => {
                    goods.push(good);
                }
                Err(failure) => {
                    if let Some(fault) = failure.fault() {
                        if goods.is_empty() {
                            throw!(fault)
                        }
                    }

                    break goods;
                }
            }
        }
    }

    /// Retrieves the next good from the market, blocking until one is available.
    ///
    /// # Errors
    ///
    /// If fault `T` is caught, SHALL throw `T`.
    #[inline]
    #[throws(<Self::Failure as Failure>::Fault)]
    fn demand(&self) -> Self::Good {
        loop {
            match self.consume() {
                Ok(good) => {
                    break good;
                }
                Err(failure) => {
                    if let Some(fault) = failure.fault() {
                        throw!(fault);
                    }
                }
            }
        }
    }
}
