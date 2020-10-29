//! Infrastructure for producers and consumers in a market.
//!
//! A market is a stock of goods. A producer stores goods in the market while a consumer retrieves goods from the market.
//!
//! In the rust stdlib, the primary example of a market is [`std::sync::mpsc::channel`]. [`std::sync::mpsc::Sender`] is the producer and [`std::sync::mpsc::Receiver`] is the consumer.
//!
//! [`std::sync::mpsc::channel`]: https://doc.rust-lang.org/std/sync/mpsc/fn.channel.html
//! [`std::sync::mpsc::Sender`]: https://doc.rust-lang.org/std/sync/mpsc/struct.Sender.html
//! [`std::sync::mpsc::Receiver`]: https:://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html

pub mod channel;
mod error;
pub mod io;
mod map;
pub mod process;
pub mod sync;
pub mod thread;

pub use error::{Failure, Fault};

use {
    core::{convert::{TryFrom, TryInto}, fmt::Debug},
    fehler::{throw, throws},
};

/// Retrieves goods from a market.
///
/// The order in which goods are retrieved is defined by the implementer.
pub trait Consumer {
    /// The item being consumed.
    type Good;
    /// The type of failure that could occur during consumption.
    type Failure: Failure;

    /// Retrieves the next good from the market without blocking.
    ///
    /// To ensure all functionality of a `Consumer` performs as specified, the implementor MUST implement `consume` such that all of the following specifications are true:
    ///
    /// 1. `consume` returns without blocking the current process.
    /// 2. If at least one good is in stock, `consume` returns one of those goods.
    /// 3. If fault `T` is thrown during consumption, `consume` throws failure `F` such that `Fault::<Self::Failure>::try_from(F)` returns `Ok(T)`.
    /// 4. If no good can be returned, `consume` throws an appropriate failure.
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good;

    /// Retrieves all goods held in the market without blocking.
    ///
    /// If no goods can be consumed, an empty list is returned. If a fault is thrown after consuming 1 or more goods, the fault is ignored and the current list of goods is returned.
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

/// Stores goods in a market.
#[allow(clippy::missing_inline_in_public_items)] // current issue with fehler for `fn produce()`; see https://github.com/withoutboats/fehler/issues/39
pub trait Producer {
    /// The item being produced.
    type Good;
    /// The type of failure that could occur during production.
    type Failure: Failure;

    /// Stores `good` in the market without blocking.
    ///
    /// To ensure all functionality of the `Producer` performs as specified, the implementor MUST implement `produce` such that all of the following specifications are true:
    ///
    /// 1. `produce` returns without blocking the current process.
    /// 2. If the market can store `good`, `process` stores `good` in the market.
    /// 3. If fault `T` is thrown during production, `produce` throws failure `F` such that `Fault::<Self::Failure>::try_from(F)` returns `Ok(T)`.
    /// 4. If `good` cannot be stored, `produce` throws an appropriate failure.
    #[allow(redundant_semicolons, unused_variables)] // current issue with fehler; see https://github.com/withoutboats/fehler/issues/39
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good);

    /// Stores each good in `goods` in the market without blocking.
    ///
    /// If a failure is thrown, all goods remaining to be produced are not attempted.
    #[throws(Self::Failure)]
    fn produce_all(&self, goods: Vec<Self::Good>) {
        for good in goods {
            self.produce(good)?
        }
    }

    /// Stores `good` in the market, blocking until space is available.
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
    /// If an error is thrown, all goods remaining to be produced are not attempted.
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

// TODO: Collector and Distributor will need to be generic based on how they choose the order of actors.
/// A [`Consumer`] that consumes goods of type `G` from multiple [`Consumer`]s.
#[derive(Default)]
pub struct Collector<G, T> 
{
    /// The [`Consumer`]s.
    consumers: Vec<Box<dyn Consumer<Good = G, Failure = error::ConsumerFailure<T>>>>,
}

impl<G, T> Collector<G, T>
{
    /// Creates a new, empty [`Collector`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            consumers: Vec::new(),
        }
    }

    /// Adds `consumer` to the end of the [`Consumer`]s held by `self`.
    #[inline]
    pub fn push<C>(&mut self, consumer: std::rc::Rc<C>)
    where
        C: Consumer + 'static,
        G: From<C::Good> + 'static,
        T: TryFrom<error::ConsumerFailure<T>> + 'static,
        error::ConsumerFailure<T>: From<C::Failure>,
    {
        self.consumers.push(Box::new(map::Adapter::new(consumer)));
    }
}

impl<G, T> Consumer for Collector<G, T>
where
    T: TryFrom<error::ConsumerFailure<T>>,
{
    type Good = G;
    type Failure = error::ConsumerFailure<T>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        let mut result = Err(error::ConsumerFailure::EmptyStock);

        for consumer in &self.consumers {
            result = consumer.consume();

            if let Err(error::ConsumerFailure::EmptyStock) = result {
                // Nothing good or bad was found, continue searching.
            } else {
                break;
            }
        }

        result?
    }
}

impl<G, E> Debug for Collector<G, E>
where
    E: Debug,
{
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Collector {{ .. }}")
    }
}

/// Distributes goods to multiple producers.
pub struct Distributor<G, T> {
    /// The producers.
    producers: Vec<Box<dyn Producer<Good = G, Failure = error::ProducerFailure<T>>>>,
}

impl<G, T> Distributor<G, T>
{
    /// Creates a new, empty [`Distributor`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `producer` to the end of the [`Producers`]s held by `self`.
    #[inline]
    pub fn push<P>(&mut self, producer: std::rc::Rc<P>)
    where
        P: Producer + 'static,
        G: TryInto<P::Good> + 'static,
        error::ProducerFailure<T>: From<P::Failure>,
        T: TryFrom<error::ProducerFailure<T>> + 'static,
    {
        self.producers.push(Box::new(map::Converter::new(producer)));
    }
}

impl<G, T> Debug for Distributor<G, T> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Distributor {{ .. }}")
    }
}

impl<G, T> Default for Distributor<G, T> {
    #[inline]
    fn default() -> Self {
        Self {
            producers: Vec::new(),
        }
    }
}

impl<G, T> Producer for Distributor<G, T>
where
    T: TryFrom<error::ProducerFailure<T>>,
    G: Clone,
{
    type Good = G;
    type Failure = error::ProducerFailure<T>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        for producer in &self.producers {
            producer.produce(good.clone())?;
        }
    }
}
