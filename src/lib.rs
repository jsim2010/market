//! Infrastructure for producers and consumers in a market.
//!
//! A market is a stock of goods that can have actors act upon it. An actor can either be a producer that stores goods in the market or a consumer that retrieves goods from the market. The important thing to note about actors is that they are not mutated when they perform actions. In other words, a function definition will look like `fn action(&self)` rather than `fn action(&mut self)`.
//!
//! In the rust stdlib, the primary example of a market is implemented by [`std::sync::mpsc::channel()`]. [`std::sync::mpsc::Sender`] is the producer and [`std::sync::mpsc::Receiver`] is the consumer.
//!
//! There are 2 categories of errors that actors may throw:
//! 1. [`Failure`]: Indicates that an action was not successful.
//! 2. [`Fault`]: A subset of [`Failure`]s that indicate the market is currently in a state where no attempted action will be successful until the state is changed (if possible).

// error must be first to ensure consumer_fault! is defined before being used.
mod error;
pub mod channel;
pub mod io;
mod map;
pub mod process;
pub mod sync;
pub mod thread;
pub mod vec;

pub use error::{ConsumerFailure, FaultlessFailure, ProducerFailure};

use {
    core::{convert::{Infallible, TryFrom}, fmt::Debug},
    fehler::{throw, throws},
};

/// Describes the failures that could occur during a given action.
pub trait Failure: Sized {
    /// Describes the fault that could occur.
    type Fault: TryFrom<Self>;
}

impl Failure for Infallible {
    type Fault = Infallible;
}

/// The type of [`Failure::Fault`] defined by the [`Failure`] `F`.
pub type Fault<F> = <F as Failure>::Fault;

/// Stores goods in a market.
///
/// Actions that can be performed have the following name convention:
/// 
/// 1. An action shall have the following components: desire and quantity.
/// 2. If an action has multiple non-empty components, they shall be split by an underscore `_`.
///
/// Desire
/// 1. `produce`: The action shall not block the current process.
/// 2. `force`: The action may block the current process if necessary.
///
/// Quantity
/// 1. (empty): The action shall attempt a single good.
/// 2. `all`: The action shall attempt multiple goods.
#[allow(clippy::missing_inline_in_public_items)] // current issue with fehler for produce(); see https://github.com/withoutboats/fehler/issues/39
pub trait Producer {
    /// The item being produced.
    type Good;
    /// The type of [`Failure`] that could occur during production.
    type Failure: Failure;

    /// Stores `good` in the market without blocking.
    ///
    /// To ensure all functionality of [`Self`] performs as specified, [`produce()`] must be implemented such that all of the following are true:
    ///
    /// 1. [`produce()`] only runs in the current process and returns without blocking.
    /// 2. If possible, `self` stores `good` in the market.
    /// 3. If [`produce()`] catches fault `T`, it shall throw [`Failure`] `F` such that `Fault::<Self::Failure>::try_from(F)` returns `Ok(T)`.
    /// 4. If `self` cannot store `good` immediately, [`produce()`] shall throw an appropriate [`Failure`].
    #[allow(redundant_semicolons, unused_variables)] // current issue with fehler; see https://github.com/withoutboats/fehler/issues/39
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good);

    /// Stores each good in `goods` in the market without blocking.
    ///
    /// If [`produce_all()`] catches [`Failure`] `F`, it shall attempt no further goods and throw `F`.
    #[throws(Self::Failure)]
    fn produce_all(&self, goods: Vec<Self::Good>) {
        for good in goods {
            self.produce(good)?
        }
    }

    /// Stores `good` in the market, blocking until space is available.
    ///
    /// If [`force()`] catches fault `T`, it shall throw `T`
    #[inline]
    #[throws(Fault<Self::Failure>)]
    fn force(&self, good: Self::Good)
    where
        Self::Good: Clone + Debug,
    {
        while let Err(failure) = self.produce(good.clone()) {
            if let Ok(fault) = Fault::<Self::Failure>::try_from(failure) {
                throw!(fault)
            }
        }
    }

    /// Stores every good in `goods`, blocking until space is available.
    ///
    /// If [`force_all()`] catches fault `T`, it shall attempt no further goods and throw `T`.
    #[throws(Fault<Self::Failure>)]
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
    /// The type of [`Failure`] that could occur during consumption.
    type Failure: Failure;

    /// Retrieves the next good from the market without blocking.
    ///
    /// To ensure all functionality of [`Self`] performs as specified, [`consume()`] must be implemented such that all of the following are true:
    ///
    /// 1. [`consume()`] only runs in the current process and returns without blocking.
    /// 2. If possible, `self` returns the next good in stock.
    /// 3. If [`consume()`] catches fault `T`, it shall throw [`Failure`] `F` such that `Fault::<Self::Failure>::try_from(F)` returns `Ok(T)`.
    /// 4. If `self` cannot immediately return a good, [`consume()`] shall throw an appropriate [`Failure`].
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good;

    /// Retrieves all goods in the market without blocking.
    ///
    /// If `self` cannot immediately consume any goods, [`consume_all()`] shall return an empty [`Vec`]. If a fault is thrown after 1 or more goods have been consumed, the fault is ignored and [`consume_all()`] returns the consumed goods.
    #[inline]
    #[throws(Fault<Self::Failure>)]
    fn consume_all(&self) -> Vec<Self::Good> {
        let mut goods = Vec::new();

        loop {
            match self.consume() {
                Ok(good) => {
                    goods.push(good);
                }
                Err(failure) => {
                    if let Ok(fault) = Fault::<Self::Failure>::try_from(failure) {
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
    #[inline]
    #[throws(Fault<Self::Failure>)]
    fn demand(&self) -> Self::Good {
        loop {
            match self.consume() {
                Ok(good) => {
                    break good;
                }
                Err(failure) => {
                    if let Ok(fault) = Fault::<Self::Failure>::try_from(failure) {
                        throw!(fault);
                    }
                }
            }
        }
    }
}
