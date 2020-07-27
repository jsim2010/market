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
pub mod process;

pub use error::{ClosedMarketError, ConsumeFailure, ProduceFailure, Recall};

use {
    core::{
        cell::RefCell,
        fmt::{self, Debug, Display},
        marker::PhantomData,
        sync::atomic::{AtomicBool, Ordering},
    },
    crossbeam_queue::SegQueue,
    fehler::{throw, throws},
    never::Never,
    std::error::Error,
};

/// Retrieves goods from a market.
///
/// The order in which goods are retrieved is defined by the implementer.
pub trait Consumer {
    /// The type of the item being consumed.
    type Good;
    /// The type of the error that could be thrown during consumption.
    type Error: Error;

    /// Retrieves the next good from the market without blocking.
    ///
    /// To ensure all functionality of the `Consumer` performs as specified, the implementor MUST implement `consume` such that all of the following specifications are true:
    ///
    /// 1. `consume` returns without blocking the current process.
    /// 2. If a good is available to the `Consumer`, `consume` returns the good.
    /// 3. If `{E}: Self::Error` is thrown, `consume` throws `ConsumeFailure::Error({E})`.
    /// 4. If the market is not holding any goods, `consume` throws `ConsumeFailure::EmptyStock`.
    #[throws(ConsumeFailure<Self::Error>)]
    fn consume(&self) -> Self::Good;

    /// Retrieves all goods held in the market without blocking.
    ///
    /// If an error is thrown after consuming 1 or more goods, the consumption stops and the error is ignored.
    #[inline]
    #[throws(ConsumeFailure<Self::Error>)]
    fn consume_all(&self) -> Vec<Self::Good> {
        let mut goods = Vec::new();

        loop {
            match self.consume() {
                Ok(good) => {
                    goods.push(good);
                }
                Err(error) => {
                    if goods.is_empty() {
                        throw!(error);
                    } else {
                        // Consumed 1 or more goods.
                        break goods;
                    }
                }
            }
        }
    }

    /// Retrieves the next good from the market, blocking until one is available.
    #[inline]
    #[throws(Self::Error)]
    fn demand(&self) -> Self::Good {
        loop {
            match self.consume() {
                Ok(good) => {
                    break good;
                }
                Err(error) => {
                    if let ConsumeFailure::Error(failure) = error {
                        throw!(failure);
                    }
                }
            }
        }
    }

    /// Creates an [`Adapter`] that converts each consumption by `self` to the appropriate `G` or `F`.
    #[inline]
    fn adapt<G, F>(self) -> Adapter<Self, G, F>
    where
        Self: Sized,
    {
        Adapter::new(self)
    }
}

/// Stores goods in a market.
#[allow(clippy::missing_inline_in_public_items)] // current issue with fehler for `fn produce()`; see https://github.com/withoutboats/fehler/issues/39
pub trait Producer {
    /// The type of the item being produced.
    type Good;
    /// The type of the error that could be thrown during production.
    type Error: Error;

    /// Stores `good` in the market without blocking.
    ///
    /// To ensure all functionality of the `Producer` performs as specified, the implementor MUST implement `produce` such that all of the following specifications are true:
    ///
    /// 1. `produce` returns without blocking the current process.
    /// 2. If the market has space available for `good`, `process` stores good in the market.
    /// 3. If the market has no space available for `good`, `process` throws `ProduceFailure::FullStock`.
    /// 4. If `{E}: Self::Error` is thrown, `produce` throws `ProduceFailure::Error({E})`.
    #[allow(redundant_semicolons, unused_variables)] // current issue with fehler; see https://github.com/withoutboats/fehler/issues/39
    #[throws(ProduceFailure<Self::Error>)]
    fn produce(&self, good: Self::Good);

    /// Stores `good` in the market without blocking, returning the good on failure.
    #[throws(Recall<Self::Good, Self::Error>)]
    fn produce_or_recall(&self, good: Self::Good)
    where
        // Debug and Dislay bounds required by Recall.
        Self::Good: Clone + Debug + Display,
    {
        self.produce(good.clone())
            .map_err(|error| Recall::new(good, error))?
    }

    /// Stores `good` in the market, blocking until space is available.
    #[inline]
    #[throws(Self::Error)]
    fn force(&self, mut good: Self::Good)
    where
        Self::Good: Clone + Debug + Display,
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
    #[throws(Self::Error)]
    fn force_all(&self, goods: Vec<Self::Good>)
    where
        Self::Good: Clone + Debug + Display,
    {
        for good in goods {
            self.force(good)?
        }
    }
}

/// A [`Consumer`] that maps the consumed good to a new good.
#[derive(Debug)]
pub struct Adapter<C, G, F> {
    /// The original consumer.
    consumer: C,
    /// The desired type of `Self::Good`.
    good: PhantomData<G>,
    /// The desired type of `Self::Error`.
    failure: PhantomData<F>,
}

impl<C, G, F> Adapter<C, G, F> {
    /// Creates a new [`Adapter`].
    const fn new(consumer: C) -> Self {
        Self {
            consumer,
            good: PhantomData,
            failure: PhantomData,
        }
    }
}

impl<C, G, F> Consumer for Adapter<C, G, F>
where
    C: Consumer,
    G: From<C::Good>,
    F: From<C::Error> + Error,
{
    type Good = G;
    type Error = F;

    #[inline]
    #[throws(ConsumeFailure<Self::Error>)]
    fn consume(&self) -> Self::Good {
        self.consumer
            .consume()
            .map_err(|error| match error {
                ConsumeFailure::EmptyStock => ConsumeFailure::EmptyStock,
                ConsumeFailure::Error(failure) => ConsumeFailure::Error(Self::Error::from(failure)),
            })
            .map(Self::Good::from)?
    }
}

/// A [`Consumer`] that consumes goods of type `G` from multiple [`Consumer`]s.
#[derive(Default)]
pub struct Collector<G, E> {
    /// The [`Consumer`]s.
    consumers: Vec<Box<dyn Consumer<Good = G, Error = E>>>,
}

impl<G, E> Collector<G, E> {
    /// Creates a new, empty [`Collector`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            consumers: Vec::new(),
        }
    }

    /// Converts `consumer` to an appropriate type then pushes it.
    #[inline]
    pub fn convert_into_and_push<C>(&mut self, consumer: C)
    where
        C: Consumer + 'static,
        G: From<C::Good> + 'static,
        E: From<C::Error> + Error + 'static,
    {
        self.push(consumer.adapt());
    }

    /// Adds `consumer` to the end of the [`Consumer`]s held by `self`.
    #[inline]
    pub fn push<C>(&mut self, consumer: C)
    where
        C: Consumer<Good = G, Error = E> + 'static,
        E: Error,
    {
        self.consumers.push(Box::new(consumer));
    }
}

impl<G, E> Consumer for Collector<G, E>
where
    E: Error,
{
    type Good = G;
    type Error = E;

    #[inline]
    #[throws(ConsumeFailure<Self::Error>)]
    fn consume(&self) -> Self::Good {
        let mut result = Err(ConsumeFailure::EmptyStock);

        for consumer in &self.consumers {
            result = match consumer.consume() {
                Ok(good) => Ok(good),
                Err(error) => match error {
                    ConsumeFailure::EmptyStock => continue,
                    ConsumeFailure::Error(_) => Err(error),
                },
            };

            break;
        }

        result?
    }
}

impl<G, E> Debug for Collector<G, E> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Collector {{ .. }}")
    }
}

/// Converts a single good into parts.
pub trait StripFrom<G>
where
    Self: Sized,
{
    /// Converts `good` into [`Vec`] of parts.
    fn strip_from(good: &G) -> Vec<Self>;
}

/// A [`Producer`] of type `P` that produces parts stripped from goods of type `G`.
#[derive(Debug)]
pub struct StrippingProducer<G, P>
where
    P: Producer,
    <P as Producer>::Good: Debug,
{
    #[doc(hidden)]
    phantom: PhantomData<G>,
    /// The producer of parts.
    producer: P,
    /// Parts stripped from a composite good yet to be produced.
    parts: RefCell<Vec<<P as Producer>::Good>>,
}

impl<G, P> StrippingProducer<G, P>
where
    P: Producer,
    <P as Producer>::Good: Debug,
{
    /// Creates a new [`StrippingProducer`].
    #[inline]
    pub fn new(producer: P) -> Self {
        Self {
            producer,
            phantom: PhantomData,
            parts: RefCell::new(Vec::new()),
        }
    }
}

impl<G, P> Producer for StrippingProducer<G, P>
where
    P: Producer,
    G: Debug + Display,
    <P as Producer>::Good: StripFrom<G> + Clone + Debug,
    <P as Producer>::Error: Error,
{
    type Good = G;
    type Error = <P as Producer>::Error;

    #[inline]
    #[throws(ProduceFailure<Self::Error>)]
    fn produce(&self, good: Self::Good) {
        let parts = <<P as Producer>::Good>::strip_from(&good);

        for part in parts {
            if let Err(error) = self.producer.produce(part) {
                throw!(error);
            }
        }
    }
}

/// Consumes parts from a [`Consumer`] of composite goods.
#[derive(Debug)]
pub struct StrippingConsumer<C, P> {
    /// The consumer of composite goods.
    consumer: C,
    /// The queue of stripped parts.
    parts: SegQueue<P>,
}

impl<C, P> StrippingConsumer<C, P>
where
    C: Consumer,
    P: StripFrom<<C as Consumer>::Good>,
{
    /// Creates a new [`StrippingConsumer`]
    #[inline]
    pub fn new(consumer: C) -> Self {
        Self {
            consumer,
            parts: SegQueue::new(),
        }
    }

    /// Consumes all stocked composite goods and strips them into parts.
    ///
    /// Runs until a [`ConsumerError`] is thrown.
    fn strip(&self) -> ConsumeFailure<<C as Consumer>::Error> {
        let error;

        loop {
            match self.consumer.consume() {
                Ok(composite) => {
                    for part in P::strip_from(&composite) {
                        self.parts.push(part);
                    }
                }
                Err(e) => {
                    error = e;
                    break;
                }
            }
        }

        error
    }
}

impl<C, P> Consumer for StrippingConsumer<C, P>
where
    C: Consumer,
    P: StripFrom<<C as Consumer>::Good> + Debug,
{
    type Good = P;
    type Error = <C as Consumer>::Error;

    #[inline]
    #[throws(ConsumeFailure<Self::Error>)]
    fn consume(&self) -> Self::Good {
        // Store result of strip because all stocked parts should be consumed prior to failing.
        let error = self.strip();

        if let Ok(part) = self.parts.pop() {
            part
        } else {
            throw!(error);
        }
    }
}

/// The Error thrown when failing to compose parts into a composite good.
#[derive(Copy, Clone, Debug)]
pub struct NonComposible;

impl<T> From<NonComposible> for ConsumeFailure<T>
where
    T: Error,
{
    #[inline]
    fn from(_: NonComposible) -> Self {
        Self::EmptyStock
    }
}

/// Converts an array of parts into a composite good.
pub trait ComposeFrom<G>
where
    Self: Sized,
{
    #[throws(NonComposible)]
    /// Converts `parts` into a composite good.
    fn compose_from(parts: &mut Vec<G>) -> Self;
}

/// Consumes composite goods of type `G` from a parts [`Consumer`] of type `C`.
#[derive(Debug)]
pub struct ComposingConsumer<C, G>
where
    C: Consumer,
    <C as Consumer>::Good: Debug,
{
    /// The consumer.
    consumer: C,
    /// The current buffer of parts.
    buffer: RefCell<Vec<<C as Consumer>::Good>>,
    #[doc(hidden)]
    phantom: PhantomData<G>,
}

impl<C, G> ComposingConsumer<C, G>
where
    C: Consumer,
    <C as Consumer>::Good: Debug,
{
    /// Creates a new [`ComposingConsumer`].
    #[inline]
    pub fn new(consumer: C) -> Self {
        Self {
            consumer,
            buffer: RefCell::new(Vec::new()),
            phantom: PhantomData,
        }
    }
}

impl<C, G> Consumer for ComposingConsumer<C, G>
where
    C: Consumer,
    G: ComposeFrom<<C as Consumer>::Good>,
    <C as Consumer>::Good: Debug,
{
    type Good = G;
    type Error = <C as Consumer>::Error;

    #[inline]
    #[throws(ConsumeFailure<Self::Error>)]
    fn consume(&self) -> Self::Good {
        let mut goods = self.consumer.consume_all()?;
        let mut buffer = self.buffer.borrow_mut();
        buffer.append(&mut goods);
        G::compose_from(&mut buffer)?
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
    type Error = <C as Consumer>::Error;

    #[inline]
    #[throws(ConsumeFailure<Self::Error>)]
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
    /// If the queue is closed.
    is_closed: AtomicBool,
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
        self.is_closed.store(true, Ordering::Relaxed);
    }
}

impl<G> Consumer for UnlimitedQueue<G>
where
    G: Debug,
{
    type Good = G;
    type Error = ClosedMarketError;

    #[inline]
    #[throws(ConsumeFailure<Self::Error>)]
    fn consume(&self) -> Self::Good {
        match self.queue.pop() {
            Ok(good) => good,
            Err(_) => {
                if self.is_closed.load(Ordering::Relaxed) {
                    throw!(ConsumeFailure::Error(ClosedMarketError));
                } else {
                    throw!(ConsumeFailure::EmptyStock);
                }
            }
        }
    }
}

impl<G> Default for UnlimitedQueue<G> {
    #[inline]
    fn default() -> Self {
        Self {
            queue: SegQueue::new(),
            is_closed: AtomicBool::new(false),
        }
    }
}

impl<G> Producer for UnlimitedQueue<G>
where
    G: Debug + Display,
{
    type Good = G;
    type Error = ClosedMarketError;

    #[inline]
    #[throws(ProduceFailure<Self::Error>)]
    fn produce(&self, good: Self::Good) {
        if self.is_closed.load(Ordering::Relaxed) {
            throw!(ProduceFailure::Error(ClosedMarketError));
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
    type Error = Never;

    #[inline]
    #[throws(ConsumeFailure<Self::Error>)]
    fn consume(&self) -> Self::Good {
        self.queue.pop().map_err(|_| ConsumeFailure::EmptyStock)?
    }
}

impl<G> Producer for PermanentQueue<G>
where
    G: Debug + Display,
{
    type Good = G;
    type Error = Never;

    // TODO: Find a way to indicate this never fails.
    #[inline]
    #[throws(ProduceFailure<Self::Error>)]
    fn produce(&self, good: Self::Good) {
        self.queue.push(good);
    }
}
