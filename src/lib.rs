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
    error::{ClosedMarketFault, ConsumeFailure, ProduceFailure, Recall},
    never::Never,
};

use {
    core::fmt::Debug,
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
    /// The fault that could be thrown during consumption.
    type Fault: Error + 'static;

    /// Retrieves the next good from the market without blocking.
    ///
    /// To ensure all functionality of a `Consumer` performs as specified, the implementor MUST implement `consume` such that all of the following specifications are true:
    ///
    /// 1. `consume` returns without blocking the current process.
    /// 2. If at least one good is in stock, `consume` returns one of those goods.
    /// 3. If fault `T` is thrown during consumption, `consume` throws `ConsumeFailure::Fault(T)`.
    /// 4. If there are no goods in stock, `consume` throws `ConsumeFailure::EmptyStock`.
    #[throws(ConsumeFailure<Self::Fault>)]
    fn consume(&self) -> Self::Good;

    /// Retrieves all goods held in the market without blocking.
    ///
    /// If the stock of the market is empty, an empty list is returned. If a fault is thrown after consuming 1 or more goods, the consumption stops and the fault is ignored.
    #[inline]
    #[throws(Self::Fault)]
    fn consume_all(&self) -> Vec<Self::Good> {
        let mut goods = Vec::new();

        loop {
            match self.consume() {
                Ok(good) => {
                    goods.push(good);
                }
                Err(failure) => {
                    if let ConsumeFailure::Fault(fault) = failure {
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
    #[throws(Self::Fault)]
    fn demand(&self) -> Self::Good {
        loop {
            match self.consume() {
                Ok(good) => {
                    break good;
                }
                Err(failure) => {
                    if let ConsumeFailure::Fault(fault) = failure {
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
    /// The fault that could be thrown during production.
    type Fault: Error;

    /// Stores `good` in the market without blocking.
    ///
    /// To ensure all functionality of the `Producer` performs as specified, the implementor MUST implement `produce` such that all of the following specifications are true:
    ///
    /// 1. `produce` returns without blocking the current process.
    /// 2. If the market has space available for `good`, `process` stores `good` in the market.
    /// 3. If the market has no space available for `good`, `process` throws `ProduceFailure::FullStock`.
    /// 4. If fault `T` is thrown, `produce` throws `ProduceFailure::Fault(T)`.
    #[allow(redundant_semicolons, unused_variables)] // current issue with fehler; see https://github.com/withoutboats/fehler/issues/39
    #[throws(ProduceFailure<Self::Fault>)]
    fn produce(&self, good: Self::Good);

    /// Stores `good` in the market without blocking, returning `good` on failure.
    #[throws(Recall<Self::Good, Self::Fault>)]
    fn produce_or_recall(&self, good: Self::Good)
    where
        // Debug and Dislay bounds required by Recall.
        Self::Good: Clone + Debug,
    {
        self.produce(good.clone())
            .map_err(|failure| Recall::new(good, failure))?
    }

    /// Stores each good in `goods` in the market without blocking.
    ///
    /// If a failure is thrown, all goods remaining to be produced are not attempted.
    #[throws(ProduceFailure<Self::Fault>)]
    fn produce_all(&self, goods: Vec<Self::Good>) {
        for good in goods {
            self.produce(good)?
        }
    }

    /// Stores `good` in the market, blocking until space is available.
    #[inline]
    #[throws(Self::Fault)]
    fn force(&self, mut good: Self::Good)
    where
        Self::Good: Clone + Debug,
        Self::Fault: 'static,
    {
        loop {
            match self.produce_or_recall(good) {
                Ok(()) => break,
                Err(recall) => {
                    good = recall.overstock()?;
                }
            }
        }
    }

    /// Stores every good in `goods`, blocking until space is available.
    ///
    /// If an error is thrown, all goods remaining to be produced are not attempted.
    #[throws(Self::Fault)]
    fn force_all(&self, goods: Vec<Self::Good>)
    where
        Self::Good: Clone + Debug,
        Self::Fault: 'static,
    {
        for good in goods {
            self.force(good)?
        }
    }
}

/// A [`Consumer`] that consumes goods of type `G` from multiple [`Consumer`]s.
#[derive(Default)]
pub struct Collector<G, T> {
    /// The [`Consumer`]s.
    consumers: Vec<Box<dyn Consumer<Good = G, Fault = T>>>,
}

impl<G, T> Collector<G, T>
where
    T: 'static,
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
        T: From<C::Fault> + Error + 'static,
    {
        self.consumers.push(Box::new(map::Adapter::new(consumer)));
    }
}

impl<G, T> Consumer for Collector<G, T>
where
    T: Error + 'static,
{
    type Good = G;
    type Fault = T;

    #[inline]
    #[throws(ConsumeFailure<Self::Fault>)]
    fn consume(&self) -> Self::Good {
        let mut result = Err(ConsumeFailure::EmptyStock);

        for consumer in &self.consumers {
            result = consumer.consume();

            if let Err(ConsumeFailure::EmptyStock) = result {
            } else {
                break;
            }
        }

        result?
    }
}

impl<G, E> Debug for Collector<G, E> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Collector {{ .. }}")
    }
}

/// Distributes goods to multiple producers.
pub struct Distributor<G, T> {
    /// The producers.
    producers: Vec<Box<dyn Producer<Good = G, Fault = T>>>,
}

impl<G, T> Distributor<G, T>
where
    T: 'static,
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
        G: core::convert::TryInto<P::Good> + 'static,
        T: From<P::Fault> + Error + 'static,
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
    T: Error + 'static,
    G: Clone,
{
    type Good = G;
    type Fault = T;

    #[inline]
    #[throws(ProduceFailure<Self::Fault>)]
    fn produce(&self, good: Self::Good) {
        for producer in &self.producers {
            producer.produce(good.clone())?;
        }
    }
}

/// Inspects if goods meet defined requirements.
pub trait Inspector {
    /// The good to be inspected.
    type Good;

    /// Returns if `good` meets requirements.
    fn allows(&self, good: &Self::Good) -> bool;
}

/// Consumes goods that an [`Inspector`] has allowed.
#[derive(Debug)]
pub struct VigilantConsumer<C, I> {
    /// The consumer.
    consumer: C,
    /// The inspector.
    inspector: I,
}

impl<C, I> VigilantConsumer<C, I> {
    /// Creates a new [`VigilantConsumer`].
    #[inline]
    pub const fn new(consumer: C, inspector: I) -> Self {
        Self {
            consumer,
            inspector,
        }
    }
}

impl<C, I> Consumer for VigilantConsumer<C, I>
where
    C: Consumer,
    I: Inspector<Good = <C as Consumer>::Good> + Debug,
{
    type Good = <C as Consumer>::Good;
    type Fault = <C as Consumer>::Fault;

    #[inline]
    #[throws(ConsumeFailure<Self::Fault>)]
    fn consume(&self) -> Self::Good {
        let mut input;

        loop {
            input = self.consumer.consume()?;

            if self.inspector.allows(&input) {
                break;
            }
        }

        input
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
        self.closure.trigger();
    }
}

impl<G> Consumer for UnlimitedQueue<G>
where
    G: Debug,
{
    type Good = G;
    type Fault = ClosedMarketFault;

    #[inline]
    #[throws(ConsumeFailure<Self::Fault>)]
    fn consume(&self) -> Self::Good {
        if let Some(good) = self.queue.pop() {
            good
        } else if self.closure.is_triggered() {
            throw!(ConsumeFailure::Fault(ClosedMarketFault));
        } else {
            throw!(ConsumeFailure::EmptyStock);
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
    type Fault = ClosedMarketFault;

    #[inline]
    #[throws(ProduceFailure<Self::Fault>)]
    fn produce(&self, good: Self::Good) {
        if self.closure.is_triggered() {
            throw!(ProduceFailure::Fault(ClosedMarketFault));
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
    type Fault = Never;

    #[inline]
    #[throws(ConsumeFailure<Self::Fault>)]
    fn consume(&self) -> Self::Good {
        if let Some(good) = self.queue.pop() {
            good
        } else {
            throw!(ConsumeFailure::EmptyStock)
        }
    }
}

impl<G> Producer for PermanentQueue<G>
where
    G: Debug,
{
    type Good = G;
    type Fault = Never;

    // TODO: Find a way to indicate this never fails.
    #[inline]
    #[throws(ProduceFailure<Self::Fault>)]
    fn produce(&self, good: Self::Good) {
        self.queue.push(good);
    }
}
