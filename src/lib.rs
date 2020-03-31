//! A library to standardize the traits of producing and consuming.
//!
//! The core purpose of this library is to define the traits of items that interact with markets. A market stores goods that have been produced in its stock until they are consumed.

#![allow(
    clippy::empty_enum, // Recommended ! type is not stable.
    clippy::implicit_return, // Goes against rust convention.
)]

pub mod channel;
pub mod io;

use {
    core::{
        cell::RefCell,
        fmt::Debug,
        marker::PhantomData,
        sync::atomic::{AtomicBool, Ordering},
    },
    crossbeam_queue::SegQueue,
    std::sync::Mutex,
    thiserror::Error as ThisError,
};

/// Consumes goods by retrieving them from the stock of a market.
///
/// The order in which goods are retrieved is defined by the consumer.
pub trait Consumer {
    /// The item being consumed.
    type Good;
    /// The error when `Self` is not functional.
    ///
    /// This can be caused by one of the following:
    ///
    /// 1. `Self` is in an invalid state
    /// 2. the market has no goods in stock and no functional producers
    type Error;

    /// Attempts to retrieve the next good from the market without blocking.
    ///
    /// Returns `Ok(None)` to indicate the stock had no goods.
    ///
    /// # Errors
    ///
    /// An error indicates `self` is not functional.
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error>;

    /// Retrieves the next good from the market, blocking if needed.
    ///
    /// # Errors
    ///
    /// An error indicates `self` is not functional.
    #[inline]
    fn demand(&self) -> Result<Self::Good, Self::Error> {
        loop {
            if let Some(result) = self.consume().transpose() {
                return result;
            }
        }
    }

    /// Creates a blocking iterator that yields goods from the market.
    #[inline]
    fn goods(&self) -> GoodsIter<'_, Self>
    where
        Self: Sized,
    {
        GoodsIter { consumer: self }
    }
}

/// Produces goods by adding them to the stock of a market.
pub trait Producer {
    /// The item being produced.
    type Good;
    /// The error when `Self` is not functional.
    ///
    /// This can be caused by one of the following:
    ///
    /// 1. `Self` is in an invalid state
    /// 2. the market has no functional consumers
    type Error;

    /// Attempts to add `good` to the market without blocking.
    ///
    /// Returns `Some(good)` if `self` desired to add it to its stock but the stock was full. Otherwise returns `None`.
    ///
    /// # Errors
    ///
    /// An error indicates `self` is not functional.
    fn produce(&self, good: Self::Good) -> Result<Option<Self::Good>, Self::Error>;

    /// Attempts to add each good in `goods` to the market without blocking.
    ///
    /// Returns the goods that `self` desired to add to its stock but the stock was full.
    ///
    /// # Errors
    ///
    /// An error indicates `self` is not functional.
    #[inline]
    fn produce_all(&self, goods: Vec<Self::Good>) -> Result<Vec<Self::Good>, Self::Error> {
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

        Ok(failed_goods)
    }

    /// Adds `good` to the market, blocking if needed.
    ///
    /// # Errors
    ///
    /// An error indicates `self` is not functional.
    #[inline]
    fn force(&self, mut good: Self::Good) -> Result<(), Self::Error> {
        while let Some(new_good) = self.produce(good)? {
            good = new_good;
        }

        Ok(())
    }

    /// Adds `goods` to the market, blocking if needed.
    ///
    /// # Errors
    ///
    /// An error indicates `self` is not functional.
    #[inline]
    fn force_all(&self, goods: Vec<Self::Good>) -> Result<(), Self::Error> {
        for good in goods {
            self.force(good)?;
        }

        Ok(())
    }
}

/// An [`Iterator`] that yields consumed goods, blocking if necessary.
///
/// Shall yield [`None`] if and only if the consumer is not functional.
#[derive(Debug)]
pub struct GoodsIter<'a, C: Consumer> {
    /// The consumer.
    consumer: &'a C,
}

impl<C: Consumer> Iterator for GoodsIter<'_, C> {
    type Item = <C as Consumer>::Good;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.consumer.demand().ok()
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
    fn produce(&self, good: Self::Good) -> Result<Option<Self::Good>, Self::Error> {
        let mut parts = self.parts.lock().map_err(|_| Self::Error::Lock)?;

        if parts.is_empty() {
            *parts = <P as Producer>::Good::strip_from(&good);
        }

        *parts = self
            .producer
            .produce_all(parts.to_vec())
            .map_err(Self::Error::Error)?;
        Ok(if parts.is_empty() { None } else { Some(good) })
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
    fn strip(&self) -> Result<(), <C as Consumer>::Error> {
        while let Some(composite) = self.consumer.consume()? {
            for part in P::strip_from(&composite) {
                self.parts.push(part);
            }
        }

        Ok(())
    }
}

impl<C, P> Consumer for StrippingConsumer<C, P>
where
    C: Consumer,
    P: StripFrom<<C as Consumer>::Good>,
{
    type Good = P;
    type Error = <C as Consumer>::Error;

    #[inline]
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error> {
        // Store result of strip because all stocked parts should be consumed prior to failing.
        let strip_result = self.strip();

        if let Ok(part) = self.parts.pop() {
            Ok(Some(part))
        } else if let Err(error) = strip_result {
            Err(error)
        } else {
            Ok(None)
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
    G: ComposeFrom<<C as Consumer>::Good>,
    <C as Consumer>::Good: Debug,
{
    type Good = G;
    type Error = <C as Consumer>::Error;

    #[inline]
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error> {
        Ok(self.consumer.consume()?.and_then(|good| {
            let mut buffer = self.buffer.borrow_mut();
            buffer.push(good);
            //log::trace!("buf: {:?}", String::from_utf8(buffer.to_vec()));
            G::compose_from(&mut buffer)
        }))
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
    I: Inspector<Good = <C as Consumer>::Good>,
{
    type Good = <C as Consumer>::Good;
    type Error = <C as Consumer>::Error;

    #[inline]
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error> {
        while let Some(input) = self.consumer.consume()? {
            if self.inspector.allows(&input) {
                return Ok(Some(input));
            }
        }

        Ok(None)
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
    fn produce(&self, good: Self::Good) -> Result<Option<Self::Good>, Self::Error> {
        if self.inspector.allows(&good) {
            self.producer.produce(good)
        } else {
            Ok(None)
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

impl<G> Consumer for UnlimitedQueue<G> {
    type Good = G;
    type Error = ClosedMarketError;

    #[inline]
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error> {
        match self.queue.pop() {
            Ok(good) => Ok(Some(good)),
            Err(_) => {
                if self.is_closed.load(Ordering::Relaxed) {
                    Err(ClosedMarketError)
                } else {
                    Ok(None)
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
    fn produce(&self, good: Self::Good) -> Result<Option<Self::Good>, Self::Error> {
        if self.is_closed.load(Ordering::Relaxed) {
            Err(ClosedMarketError)
        } else {
            self.queue.push(good);
            Ok(None)
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

impl<G> Consumer for PermanentQueue<G> {
    type Good = G;
    type Error = NeverErr;

    #[inline]
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error> {
        Ok(self.queue.pop().ok())
    }
}

impl<G> Producer for PermanentQueue<G> {
    type Good = G;
    type Error = NeverErr;

    #[inline]
    fn produce(&self, good: Self::Good) -> Result<Option<Self::Good>, Self::Error> {
        self.queue.push(good);
        Ok(None)
    }
}
