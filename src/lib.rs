//! A library to simplify development with producers and consumers.
//!
//! The core purpose of this library is to define the traits [`Consumer`] and [`Producer`]. These traits provide the functionality to send and receive items known as **goods** on a market.
//!
//! [`Consumer`]: trait.Consumer.html
//! [`Producer`]: trait.Producer.html

#![allow(
    clippy::empty_enum, // Recommended ! type is not stable.
    clippy::implicit_return, // Goes against rust convention.
)]

pub mod channel;

use {
    core::{
        cell::RefCell,
        fmt::{self, Debug},
        marker::PhantomData,
        sync::atomic::{AtomicBool, Ordering},
    },
    crossbeam_queue::SegQueue,
    std::{
        error::Error,
        io::{self, BufRead, Write},
    },
    thiserror::Error as ThisError,
};

/// Retrieves goods from a defined market.
///
/// The order in which available goods are retrieved is defined by the consumer.
pub trait Consumer {
    /// The type of the item being consumed.
    type Good;
    /// The type of the error when failing to consume a good.
    type Error;

    /// Attempts to retrieve the next good from the market without blocking.
    ///
    /// Returns `Ok(None)` when no good was available.
    ///
    /// # Errors
    ///
    /// Returns `Self::Error` when a good is available but `self` fails to consume it.
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error>;

    /// Retrieves the next good of the market, blocking until one is available or the consumption fails.
    ///
    /// # Errors
    ///
    /// Returns `Self::Error` when `self` fails to consume a good.
    #[inline]
    fn demand(&self) -> Result<Self::Good, Self::Error> {
        loop {
            if let Some(result) = self.consume().transpose() {
                return result;
            }
        }
    }

    /// Creates a blocking iterator that yields the goods from the market.
    #[inline]
    fn goods(&self) -> GoodsIter<'_, Self>
    where
        Self: Sized,
    {
        GoodsIter { consumer: self }
    }
}

/// Generates goods for a defined market.
pub trait Producer<'a> {
    /// The type of the item being produced.
    type Good;
    /// The type of the error when failing to produce a good to an available spot.
    type Error;

    // TODO: Add a produce() that is non-blocking.
    /// Generates `good` to the market, blocking until a space for the good is available.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] when `self` fails to produce `good`.
    fn force(&'a self, good: Self::Good) -> Result<(), Self::Error>;
}

/// An [`Iterator`] that yields consumed goods, blocking if necessary.
///
/// Shall yield [`None`] if and only if the consumer fails to consume a good.
#[derive(Debug)]
pub struct GoodsIter<'a, C: Consumer> {
    /// The consumer.
    consumer: &'a C,
}

impl<C: Consumer + 'static> Iterator for GoodsIter<'_, C> {
    type Item = <C as Consumer>::Good;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.consumer.demand().ok()
    }
}

/// An unlimited queue that cannot close.
///
/// Has the useful characteristic that its consumption and production are unfailable.
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

impl<G> Producer<'_> for PermanentQueue<G> {
    type Good = G;
    type Error = NeverErr;

    #[inline]
    fn force(&self, good: Self::Good) -> Result<(), Self::Error> {
        self.queue.push(good);
        Ok(())
    }
}

/// Converts a single composite good into parts.
///
/// This is similar to [`From`] except that [`Strip`] allows any number, including 0, of parts.
pub trait Strip<C>
where
    Self: Sized,
{
    /// Converts `composite` into a [`Vec`] of parts.
    fn strip(composite: C) -> Vec<Self>;
}

/// A [`Consumer`] that converts a single composite good into its parts.
#[derive(Debug)]
pub struct Stripper<C, P> {
    /// Consumer of composite goods.
    consumer: Box<C>,
    /// Queue of stripped parts.
    parts: PermanentQueue<P>,
}

impl<C, P> Stripper<C, P>
where
    C: Consumer,
    P: Strip<<C as Consumer>::Good>,
{
    /// Creates a new [`Stripper`]
    #[inline]
    pub fn new(consumer: C) -> Self {
        Self {
            consumer: Box::new(consumer),
            parts: PermanentQueue::new(),
        }
    }

    /// Consumes all available composite goods and strips it for parts.
    fn strip(&self) -> Result<(), <C as Consumer>::Error> {
        while let Some(composite) = self.consumer.consume()? {
            for part in P::strip(composite) {
                #[allow(clippy::result_unwrap_used)] // PermananentQueue.force() cannot fail.
                self.parts.force(part).unwrap();
            }
        }

        Ok(())
    }
}

impl<C, P> Consumer for Stripper<C, P>
where
    C: Consumer,
    P: Strip<<C as Consumer>::Good>,
{
    type Good = P;
    type Error = <C as Consumer>::Error;

    #[inline]
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error> {
        let strip_result = self.strip();
        // Require expression outside of if because attributes are not allowed on if expressions.
        #[allow(clippy::result_unwrap_used)] // PermanentQueue.consume cannot fail.
        let consumed_part = self.parts.consume().unwrap();

        if let Some(part) = consumed_part {
            Ok(Some(part))
        } else if let Err(error) = strip_result {
            Err(error)
        } else {
            Ok(None)
        }
    }
}

/// Determines if a good is valid.
pub trait Validator {
    /// The good to be validated.
    type Good;

    /// Returns if `good` is valid.
    fn is_valid(&self, good: &Self::Good) -> bool;
}

/// Filters consumed goods, only consuming those that have been validated.
pub struct FilterConsumer<G, E> {
    /// The consumer.
    consumer: Box<dyn Consumer<Good = G, Error = E>>,
    /// The validator.
    validator: Box<dyn Validator<Good = G>>,
}

impl<G, E> FilterConsumer<G, E>
where
    E: Error,
{
    /// Creates a new [`FilterConsumer`].
    #[inline]
    pub fn new(
        consumer: impl Consumer<Good = G, Error = E> + 'static,
        validator: impl Validator<Good = G> + 'static,
    ) -> Self {
        Self {
            consumer: Box::new(consumer),
            validator: Box::new(validator),
        }
    }
}

impl<G, E> Consumer for FilterConsumer<G, E>
where
    E: Error,
{
    type Good = G;
    type Error = E;

    #[inline]
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error> {
        while let Some(input) = self.consumer.consume()? {
            if self.validator.is_valid(&input) {
                return Ok(Some(input));
            }
        }

        Ok(None)
    }
}

impl<G, E> Debug for FilterConsumer<G, E> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FilterConsumer {{ .. }}")
    }
}

/// Filters produced goods, only producing those that have been validated.
pub struct FilterProducer<'a, G, E> {
    /// The producer.
    producer: Box<dyn Producer<'a, Good = G, Error = E>>,
    /// The validator.
    validator: Box<dyn Validator<Good = G>>,
}

impl<'a, G, E> FilterProducer<'a, G, E>
where
    E: Error,
{
    /// Creates a new [`FilterProducer`].
    #[inline]
    pub fn new(
        producer: impl Producer<'a, Good = G, Error = E> + 'static,
        validator: impl Validator<Good = G> + 'static,
    ) -> Self {
        Self {
            producer: Box::new(producer),
            validator: Box::new(validator),
        }
    }
}

impl<'a, G, E> Debug for FilterProducer<'a, G, E> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FilterProducer {{ .. }}")
    }
}

impl<'a, G, E> Producer<'a> for FilterProducer<'a, G, E>
where
    E: Error,
{
    type Good = G;
    type Error = E;

    #[inline]
    fn force(&'a self, good: Self::Good) -> Result<(), Self::Error> {
        if self.validator.is_valid(&good) {
            self.producer.force(good)
        } else {
            Ok(())
        }
    }
}

/// An item that can be written.
pub trait Writable {
    /// The type of the error when failing to write the item.
    type Error;

    /// Writes `self` to `writer`.
    ///
    /// # Errors
    ///
    /// Returns `Self::Error` when `writer` fails to write `self`.
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error>;
}

/// Writes goods of type `G` to a writer of type `W`.
#[derive(Debug)]
pub struct Writer<G, W> {
    #[doc(hidden)]
    phantom: PhantomData<G>,
    /// The writer.
    writer: RefCell<W>,
}

impl<G, W> Writer<G, W> {
    /// Creates a new [`Writer`].
    #[inline]
    pub const fn new(writer: W) -> Self {
        Self {
            writer: RefCell::new(writer),
            phantom: PhantomData,
        }
    }
}

impl<G, W> Producer<'_> for Writer<G, W>
where
    G: Writable,
    <G as Writable>::Error: Error + 'static,
    W: Write,
{
    type Good = G;
    type Error = WriteGoodError<<Self::Good as Writable>::Error>;

    #[inline]
    fn force(&self, good: Self::Good) -> Result<(), Self::Error> {
        let mut writer = self.writer.borrow_mut();
        good.write_to(writer.by_ref()).map_err(Self::Error::Write)?;
        Ok(writer.flush().map_err(Self::Error::Flush)?)
    }
}

/// An item that can be read from bytes.
pub trait Readable
where
    Self: Sized,
{
    /// An error converting bytes to a `Self`.
    type Error: Error;

    /// Converts `bytes` into a `Self`.
    fn from_bytes(bytes: &[u8]) -> (usize, Result<Self, Self::Error>);
}

/// Reads goods of type `G` from a reader of type `R`.
#[derive(Debug)]
pub struct Reader<G, R> {
    #[doc(hidden)]
    phantom: PhantomData<G>,
    /// The reader.
    reader: RefCell<R>,
    /// The current buffer of bytes.
    buffer: RefCell<Vec<u8>>,
}

impl<G, R> Reader<G, R> {
    /// Creates a new [`Reader`].
    #[inline]
    pub fn new(reader: R) -> Self {
        Self {
            buffer: RefCell::new(Vec::new()),
            reader: RefCell::new(reader),
            phantom: PhantomData,
        }
    }
}

impl<G, R> Consumer for Reader<G, R>
where
    G: Readable + Debug,
    <G as Readable>::Error: 'static,
    R: BufRead,
{
    type Good = G;
    type Error = ReadGoodError<G>;

    #[inline]
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error> {
        let mut reader = self.reader.borrow_mut();
        let buf = reader.fill_buf().map_err(Self::Error::Io)?;
        let consumed_len = buf.len();
        let mut buffer = self.buffer.borrow_mut();
        buffer.extend_from_slice(buf);
        let (processed_len, good) = G::from_bytes(&buffer);

        let _ = buffer.drain(..processed_len);
        reader.consume(consumed_len);
        Ok(good.map(Some).map_err(Self::Error::Read)?)
    }
}

/// An error that will never occur, equivalent to `!`.
#[derive(Copy, Clone, Debug, ThisError)]
pub enum NeverErr {}

/// An error consuming a good from a closable market.
#[derive(Clone, Copy, Debug, ThisError)]
#[error("market is closed")]
pub struct ClosedMarketError;

/// An error writing a good.
#[derive(Debug, ThisError)]
pub enum WriteGoodError<T: Error + 'static> {
    /// An error writing the input.
    #[error("{0}")]
    Write(#[source] T),
    /// An error flushing the input.
    #[error("{0}")]
    Flush(#[source] io::Error),
}

impl<T: Error> From<T> for WriteGoodError<T> {
    #[inline]
    fn from(value: T) -> Self {
        Self::Write(value)
    }
}
/// An error reading a good.
#[derive(Debug, ThisError)]
pub enum ReadGoodError<G>
where
    G: Readable + Debug,
    <G as Readable>::Error: 'static,
{
    /// Indicates an error with the input/output.
    #[error("{0}")]
    Io(#[from] io::Error),
    /// Indicates an error in converting bytes to a [`Readable`].
    #[error("{0}")]
    Read(#[source] <G as Readable>::Error),
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

impl<G> Producer<'_> for UnlimitedQueue<G> {
    type Good = G;
    type Error = ClosedMarketError;

    #[inline]
    fn force(&self, good: Self::Good) -> Result<(), Self::Error> {
        if self.is_closed.load(Ordering::Relaxed) {
            Err(ClosedMarketError)
        } else {
            self.queue.push(good);
            Ok(())
        }
    }
}
