//! A library to standardize the traits of producing and consuming.
//!
//! The core purpose of this library is to define the traits of items that interact with markets. A market stores goods in its stock. Producers add goods to the stock while consumers remove goods from the stock. One important characteristic of producers and consumers is that they are not modified by their respective action (producing or consuming). In other words, the signature of the action function starts with `fn action(&self`, not `fn action(&mut self`.

pub mod channel;
pub mod io;

use {
    core::{
        cell::RefCell,
        fmt::{self, Debug},
        marker::PhantomData,
        sync::atomic::{AtomicBool, Ordering},
    },
    crossbeam_queue::SegQueue,
    fehler::{throw, throws},
    std::{error::Error, sync::Mutex},
    thiserror::Error as ThisError,
};

/// Consumes goods from the stock of a market.
///
/// The order in which goods are consumed is defined by the implementation of the [`Consumer`].
pub trait Consumer {
    /// The type of the item being consumed.
    type Good;
    /// The type of the error when `Self` has failed while consuming.
    ///
    /// The failure of `Self` can be caused by any of the following:
    ///
    /// 1. `Self` is in an invalid state
    /// 2. the market has no goods in stock and no functional producers
    type Error;

    /// Attempts to consume the next good from the market without blocking.
    ///
    /// Returns `Ok(None)` to indicate the stock is empty.
    #[throws(Self::Error)]
    fn consume(&self) -> Option<Self::Good>;

    /// Consumes the next good from the market, blocking until a good is available.
    #[inline]
    #[throws(Self::Error)]
    fn demand(&self) -> Self::Good {
        let good;

        loop {
            if let Some(g) = self.consume()? {
                good = g;
                break;
            }
        }

        good
    }

    /// Creates a [`Consumer`] that calls `map` or `err_map` on each consume attempt.
    #[inline]
    fn map<M, F>(self, map: M, err_map: F) -> MappedConsumer<Self, M, F>
    where
        Self: Sized,
    {
        MappedConsumer {
            consumer: self,
            map,
            err_map,
        }
    }
}

/// Produces goods to be stored in the stock of a market.
///
/// The decision tree to determine if it is desirable that a given good should be added to the market is defined by the implementation.
pub trait Producer {
    /// The type of the item being produced.
    type Good;
    /// The type of the error when `Self` has failed while producing.
    ///
    /// The failure of `Self` can be caused by any of the following:
    ///
    /// 1. `Self` is in an invalid state
    /// 2. the market has no functional consumers
    type Error;

    /// Attempts to produce `good` to the market without blocking.
    ///
    /// Returns `Some(good)` if `self` desired to add it to its stock but the stock was full. Otherwise returns `None`.
    #[throws(Self::Error)]
    fn produce(&self, good: Self::Good) -> Option<Self::Good>;

    /// Attempts `num_retries` + 1 times to produce `good to the market without blocking.
    ///
    /// Returns an error if `good` was not added for any reason.
    #[inline]
    #[throws(OneShotError<Self::Error>)]
    fn attempt(&self, mut good: Self::Good, mut num_retries: u64)
    where
        Self::Error: Error,
    {
        while num_retries > 0 {
            if let Some(failed_good) = self.produce(good)? {
                num_retries = num_retries.saturating_sub(1);
                good = failed_good;
            } else {
                return;
            }
        }

        if self.produce(good)?.is_some() {
            throw!(OneShotError::Full);
        }
    }

    /// Attempts to produce each good in `goods` to the market without blocking.
    ///
    /// Returns the goods that `self` desired to add to the stock but the stock was full. Once the stock is found to be full, `self` no longer attempts to add a good.
    #[inline]
    #[throws(Self::Error)]
    fn produce_all(&self, goods: Vec<Self::Good>) -> Vec<Self::Good> {
        let mut failed_goods = Vec::new();

        for good in goods {
            if failed_goods.is_empty() {
                if let Some(new_good) = self.produce(good)? {
                    failed_goods.push(new_good);
                }
            } else {
                failed_goods.push(good);
            }
        }

        failed_goods
    }

    /// Produces `good` to the market, blocking if needed.
    #[inline]
    #[throws(Self::Error)]
    fn force(&self, mut good: Self::Good) {
        while let Some(failed_good) = self.produce(good)? {
            good = failed_good;
        }
    }

    /// Produces each good in `goods` to the market, blocking if needed.
    #[inline]
    #[throws(Self::Error)]
    fn force_all(&self, goods: Vec<Self::Good>) {
        for good in goods {
            self.force(good)?;
        }
    }
}

/// A [`Consumer`] that maps the consumed good to a new good.
pub struct MappedConsumer<C, M, F> {
    /// The original consumer.
    consumer: C,
    /// The [`Fn`] to map from `C::Good` to `Self::Good`.
    map: M,
    /// The [`Fn`] to map from `C::Error` to `Self::Error`.
    err_map: F,
}

impl<G, E, C, M, F> Consumer for MappedConsumer<C, M, F>
where
    C: Consumer,
    M: Fn(C::Good) -> G,
    F: Fn(C::Error) -> E,
{
    type Good = G;
    type Error = E;

    #[inline]
    #[throws(Self::Error)]
    fn consume(&self) -> Option<Self::Good> {
        self.consumer
            .consume()
            .map_err(|error| (self.err_map)(error))?
            .map(|good| (self.map)(good))
    }
}

impl<C, M, F> Debug for MappedConsumer<C, M, F>
where
    C: Debug,
{
    #[allow(clippy::use_debug)] // Okay to use debug in Debug impl.
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MappedConsumer {{ consumer: {:?} }}", self.consumer)
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
        E: From<C::Error> + 'static,
    {
        self.push(consumer.map(G::from, E::from));
    }

    /// Adds `consumer` to the end of the [`Consumer`]s held by `self`.
    #[inline]
    pub fn push<C>(&mut self, consumer: C)
    where
        C: Consumer<Good = G, Error = E> + 'static,
    {
        self.consumers.push(Box::new(consumer));
    }
}

impl<G, E> Consumer for Collector<G, E> {
    type Good = G;
    type Error = E;

    #[inline]
    #[throws(Self::Error)]
    fn consume(&self) -> Option<Self::Good> {
        let mut good = None;

        for consumer in &self.consumers {
            good = consumer.consume()?;

            if good.is_some() {
                break;
            }
        }

        good
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

/// An error when a [`Producer`] is trying to produce in a single shot.
#[derive(Debug, ThisError)]
pub enum OneShotError<E>
where
    E: Error + 'static,
{
    /// The [`Producer`] is invalid.
    #[error(transparent)]
    Error(#[from] E),
    /// The stock was full.
    #[error("stock was full")]
    Full,
}

/// An error producing parts.
#[derive(Debug, ThisError)]
pub enum StripError<T>
where
    T: Debug,
{
    /// An error locking the mutex.
    #[error("")]
    Lock,
    /// An error producing the part.
    #[error("")]
    Error(T),
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
    parts: Mutex<Vec<<P as Producer>::Good>>,
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
            parts: Mutex::new(Vec::new()),
        }
    }
}

impl<G, P> Producer for StrippingProducer<G, P>
where
    P: Producer,
    <P as Producer>::Good: StripFrom<G> + Clone + Debug,
    <P as Producer>::Error: Debug,
{
    type Good = G;
    type Error = StripError<<P as Producer>::Error>;

    #[inline]
    #[throws(Self::Error)]
    fn produce(&self, good: Self::Good) -> Option<Self::Good> {
        let mut parts = self.parts.lock().map_err(|_| Self::Error::Lock)?;

        if parts.is_empty() {
            *parts = <<P as Producer>::Good>::strip_from(&good);
        }

        *parts = self
            .producer
            .produce_all(parts.to_vec())
            .map_err(Self::Error::Error)?;
        if parts.is_empty() {
            None
        } else {
            Some(good)
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
    #[throws(<C as Consumer>::Error)]
    fn strip(&self) {
        while let Some(composite) = self.consumer.consume()? {
            for part in P::strip_from(&composite) {
                self.parts.push(part);
            }
        }
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
    #[throws(Self::Error)]
    fn consume(&self) -> Option<Self::Good> {
        // Store result of strip because all stocked parts should be consumed prior to failing.
        let strip_result = self.strip();

        if let Ok(part) = self.parts.pop() {
            Some(part)
        } else if let Err(error) = strip_result {
            throw!(error);
        } else {
            None
        }
    }
}

/// Converts an array of parts into a composite good.
pub trait ComposeFrom<G>
where
    Self: Sized,
{
    /// Converts `parts` into a composite good.
    fn compose_from(parts: &mut Vec<G>) -> Option<Self>;
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
    G: ComposeFrom<<C as Consumer>::Good> + Debug,
    <C as Consumer>::Good: Debug,
{
    type Good = G;
    type Error = <C as Consumer>::Error;

    #[inline]
    #[throws(Self::Error)]
    fn consume(&self) -> Option<Self::Good> {
        self.consumer.consume()?.and_then(|good| {
            let mut buffer = self.buffer.borrow_mut();
            buffer.push(good);
            G::compose_from(&mut buffer)
        })
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
    #[throws(Self::Error)]
    fn consume(&self) -> Option<Self::Good> {
        while let Some(input) = self.consumer.consume()? {
            if self.inspector.allows(&input) {
                return Some(input);
            }
        }

        None
    }
}

/// Produces goods that an [`Inspector`] has allowed.
#[derive(Debug)]
pub struct ApprovedProducer<P, I> {
    /// The producer.
    producer: P,
    /// The inspector.
    inspector: I,
}

impl<P, I> ApprovedProducer<P, I> {
    /// Creates a new [`ApprovedProducer`].
    #[inline]
    pub const fn new(producer: P, inspector: I) -> Self {
        Self {
            producer,
            inspector,
        }
    }
}

impl<P, I> Producer for ApprovedProducer<P, I>
where
    P: Producer,
    I: Inspector<Good = <P as Producer>::Good>,
{
    type Good = <P as Producer>::Good;
    type Error = <P as Producer>::Error;

    #[inline]
    #[throws(Self::Error)]
    fn produce(&self, good: Self::Good) -> Option<Self::Good> {
        if self.inspector.allows(&good) {
            self.producer.produce(good)?
        } else {
            None
        }
    }
}

/// An error that will never occur, equivalent to `!`.
#[derive(Copy, Clone, Debug, ThisError)]
pub enum NeverErr {}

/// An error consuming a good from a closable market.
#[derive(Clone, Copy, Debug, ThisError)]
#[error("market is closed")]
pub struct ClosedMarketError;

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
    #[throws(Self::Error)]
    fn consume(&self) -> Option<Self::Good> {
        match self.queue.pop() {
            Ok(good) => Some(good),
            Err(_) => {
                if self.is_closed.load(Ordering::Relaxed) {
                    throw!(ClosedMarketError);
                } else {
                    None
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

impl<G> Producer for UnlimitedQueue<G> {
    type Good = G;
    type Error = ClosedMarketError;

    #[inline]
    #[throws(Self::Error)]
    fn produce(&self, good: Self::Good) -> Option<Self::Good> {
        if self.is_closed.load(Ordering::Relaxed) {
            throw!(ClosedMarketError);
        } else {
            self.queue.push(good);
            None
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
    type Error = NeverErr;

    #[inline]
    #[throws(Self::Error)]
    fn consume(&self) -> Option<Self::Good> {
        self.queue.pop().ok()
    }
}

impl<G> Producer for PermanentQueue<G> {
    type Good = G;
    type Error = NeverErr;

    #[inline]
    #[throws(Self::Error)]
    fn produce(&self, good: Self::Good) -> Option<Self::Good> {
        self.queue.push(good);
        None
    }
}
