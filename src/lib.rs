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

pub use {
    error::{Fault, InfallibleFailure, Failure, ClosedMarketFault, ClassicalConsumerFailure, ClassicalProducerFailure},
    never::Never,
};

use {
    core::{convert::TryFrom, fmt::Debug},
    crossbeam_queue::SegQueue,
    fehler::{throw, throws},
    std::error::Error,
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

/// A [`Consumer`] that consumes goods of type `G` from multiple [`Consumer`]s.
#[derive(Default)]
pub struct Collector<G, F> 
{
    /// The [`Consumer`]s.
    consumers: Vec<Box<dyn Consumer<Good = G, Failure = F>>>,
}

impl<G, F> Collector<G, F>
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
        F: From<C::Failure> + Failure + 'static,
    {
        self.consumers.push(Box::new(map::Adapter::new(consumer)));
    }
}

impl<G, F> Consumer for Collector<G, F>
where
    F: Failure,
{
    type Good = G;
    type Failure = F;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        let mut result = Err(F::insufficient_stock());

        for consumer in &self.consumers {
            let mut is_finished = true;
            result = consumer.consume();

            if let Err(ref failure) = result {
                if failure.is_insufficient_stock() {
                    is_finished = false;
                }
            }

            if is_finished {
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
    producers: Vec<Box<dyn Producer<Good = G, Failure = ClassicalProducerFailure<T>>>>,
}

impl<G, T> Distributor<G, T>
where
    T: Eq + 'static,
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
        P: Producer<Failure = ClassicalProducerFailure<T>> + 'static,
        G: core::convert::TryInto<P::Good> + 'static,
        T: TryFrom<ClassicalProducerFailure<T>> + From<Fault<P::Failure>> + Error + 'static,
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
    T: TryFrom<ClassicalProducerFailure<T>> + Eq + Error + 'static,
    G: Clone,
{
    type Good = G;
    type Failure = ClassicalProducerFailure<T>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        for producer in &self.producers {
            producer.produce(good.clone())?;
        }
    }
}

/// Defines a queue with unlimited size that implements [`Consumer`] and [`Producer`].
///
/// An [`UnlimitedQueue`] can be closed, which prevents the [`Producer`] from producing new goods while allowing the [`Consumer`] to consume only the remaining goods on the queue.
#[derive(Debug)]
pub struct UnlimitedQueue<G> {
    /// The queue.
    queue: SegQueue<G>,
    /// A trigger to close the queue.
    closure: sync::Trigger,
}

impl<G> UnlimitedQueue<G> {
    /// Creates a new empty [`UnlimitedQueue`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Closes `self`.
    #[inline]
    pub fn close(&self) {
        #[allow(clippy::expect_used)] // Trigger::produce() will not fail.
        self.closure.produce(()).expect("triggering closure");
    }
}

impl<G> Consumer for UnlimitedQueue<G>
where
    G: Debug,
{
    type Good = G;
    type Failure = ClassicalConsumerFailure<ClosedMarketFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        if let Some(good) = self.queue.pop() {
            good
        } else if self.closure.consume().is_ok() {
            throw!(ClassicalConsumerFailure::Fault(ClosedMarketFault));
        } else {
            throw!(ClassicalConsumerFailure::EmptyStock);
        }
    }
}

impl<G> Default for UnlimitedQueue<G> {
    #[inline]
    fn default() -> Self {
        Self {
            queue: SegQueue::new(),
            closure: sync::Trigger::new(),
        }
    }
}

impl<G> Producer for UnlimitedQueue<G>
where
    G: Debug,
{
    type Good = G;
    type Failure = ClassicalProducerFailure<ClosedMarketFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        if self.closure.consume().is_ok() {
            throw!(ClassicalProducerFailure::Fault(ClosedMarketFault));
        } else {
            self.queue.push(good);
        }
    }
}

/// An unlimited queue with a producer and consumer that are always functional.
#[derive(Debug, Default)]
pub struct PermanentQueue<G> {
    /// The queue.
    queue: SegQueue<G>,
}

impl<G> PermanentQueue<G> {
    /// Creates a new [`PermanentQueue`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            queue: SegQueue::new(),
        }
    }
}

impl<G> Consumer for PermanentQueue<G>
where
    G: Debug,
{
    type Good = G;
    type Failure = InfallibleFailure;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        if let Some(good) = self.queue.pop() {
            good
        } else {
            throw!(InfallibleFailure)
        }
    }
}

impl<G> Producer for PermanentQueue<G>
where
    G: Debug,
{
    type Good = G;
    type Failure = ClassicalProducerFailure<Never>;

    // TODO: Find a way to indicate this never fails.
    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        self.queue.push(good);
    }
}
