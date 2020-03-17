//! A library to simplify development with producers and consumers.
//!
//! The core purpose of this library is to define the traits [`Consumer`] and [`Producer`]. These traits provide the functionality to send and receive items known as **goods** on a market.
//!
//! [`Consumer`]: trait.Consumer.html
//! [`Producer`]: trait.Producer.html
use {
    core::{
        cell::RefCell,
        fmt::{self, Debug},
        marker::PhantomData,
        sync::atomic::{AtomicBool, Ordering},
    },
    crossbeam_channel::SendError,
    crossbeam_queue::SegQueue,
    std::{
        error,
        io::{self, BufRead, Write},
        sync::mpsc,
    },
    thiserror::Error,
};

/// Retrieves goods from a defined market.
///
/// The order in which available goods are retrieved is defined by the consumer.
pub trait Consumer {
    /// The type of the item returned by a successful retrieval.
    type Good;
    /// The type of the error produced when consuming from an invalid market.
    ///
    /// An invalid market is one that has no available goods and no current producers.
    type Error;

    /// Attempts to retrieve the next consumable of the market without blocking.
    ///
    /// Returning `Ok(None)` indicates that no consumable was available.
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error>;

    /// Retrieves the next consumable of the market, blocking until one is found.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] when the market is invalid.
    #[inline]
    fn demand(&self) -> Result<Self::Good, Self::Error> {
        loop {
            if let Some(result) = self.consume().transpose() {
                return result;
            }
        }
    }

    /// Returns the [`GoodsIter`] of `self`.
    #[inline]
    fn goods(&self) -> GoodsIter<'_, Self>
    where
        Self: Sized,
    {
        GoodsIter { consumer: self }
    }
}

/// An [`Iterator`] that yields consumed goods, blocking if necessary.
///
/// Shall yield [`None`] if and only if the market is invalid.
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

/// Generates goods for a defined market.
pub trait Producer<'a> {
    /// The type of the item being produced.
    type Good;
    /// The type of an error during production.
    type Error;

    // TODO: Add a produce() that is non-blocking.
    // TODO: Change this to force().
    /// Generates `good` to the market, blocking until a space for the good is available.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] when producer fails to produce `good`.
    fn produce(&'a self, good: Self::Good) -> Result<(), Self::Error>;
}

/// An error consuming a good.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
pub enum ConsumeGoodError {
    /// The market is closed.
    #[error("unable to consume from closed market")]
    Closed,
}

/// Defines a [`mpsc::Receiver`] that implements [`Consumer`].
#[derive(Debug)]
pub struct MpscConsumer<G> {
    /// The receiver.
    rx: mpsc::Receiver<G>,
}

impl<G> Consumer for MpscConsumer<G> {
    type Good = G;
    type Error = ConsumeGoodError;

    #[inline]
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error> {
        match self.rx.try_recv() {
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(Self::Error::Closed),
            Ok(good) => Ok(Some(good)),
        }
    }
}

impl<G> From<mpsc::Receiver<G>> for MpscConsumer<G> {
    #[inline]
    fn from(value: mpsc::Receiver<G>) -> Self {
        Self { rx: value }
    }
}

/// Defines a [`crossbeam_channel::Receiver`] that implements [`Consumer`].
#[derive(Debug)]
pub struct CrossbeamConsumer<G> {
    /// The [`crossbeam_channel::Recevier`].
    rx: crossbeam_channel::Receiver<G>,
}

impl<G> Consumer for CrossbeamConsumer<G> {
    type Good = G;
    type Error = ConsumeGoodError;

    #[inline]
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error> {
        match self.rx.try_recv() {
            Err(crossbeam_channel::TryRecvError::Empty) => Ok(None),
            Err(crossbeam_channel::TryRecvError::Disconnected) => Err(Self::Error::Closed),
            Ok(good) => Ok(Some(good)),
        }
    }
}

impl<G> From<crossbeam_channel::Receiver<G>> for CrossbeamConsumer<G> {
    #[inline]
    fn from(value: crossbeam_channel::Receiver<G>) -> Self {
        Self { rx: value }
    }
}

/// Defines a [`crossbeam_channel::Sender`] that implements [`Producer`].
#[derive(Debug)]
pub struct CrossbeamProducer<G> {
    /// The sender.
    tx: crossbeam_channel::Sender<G>,
}

impl<G> Producer<'_> for CrossbeamProducer<G> {
    type Good = G;
    type Error = SendError<G>;

    #[inline]
    fn produce(&self, record: Self::Good) -> Result<(), Self::Error> {
        self.tx.send(record)
    }
}

impl<G> From<crossbeam_channel::Sender<G>> for CrossbeamProducer<G> {
    #[inline]
    fn from(value: crossbeam_channel::Sender<G>) -> Self {
        Self { tx: value }
    }
}

/// An error producing a good.
#[derive(Clone, Copy, Debug, Error)]
pub enum ProduceGoodError {
    /// An error consuming or producing with a closed queue.
    #[error("unable to produce to closed market")]
    Closed,
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
    type Error = ConsumeGoodError;

    #[inline]
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error> {
        match self.queue.pop() {
            Ok(good) => Ok(Some(good)),
            Err(_) => {
                if self.is_closed.load(Ordering::Relaxed) {
                    Err(Self::Error::Closed)
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
    type Error = ProduceGoodError;

    #[inline]
    fn produce(&self, good: Self::Good) -> Result<(), Self::Error> {
        if self.is_closed.load(Ordering::Relaxed) {
            Err(Self::Error::Closed)
        } else {
            self.queue.push(good);
            Ok(())
        }
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
pub struct Stripper<C, P, E> {
    /// Consumer of composite goods.
    consumer: Box<dyn Consumer<Good = C, Error = E>>,
    /// Queue of stripped parts.
    parts: UnlimitedQueue<P>,
}

impl<C, P, E> Stripper<C, P, E>
where
    P: Strip<C>,
{
    /// Creates a new [`Stripper`]
    #[inline]
    pub fn new(consumer: impl Consumer<Good = C, Error = E> + 'static) -> Self {
        Self {
            consumer: Box::new(consumer),
            parts: UnlimitedQueue::new(),
        }
    }

    /// Consumes all available composite goods and strips it for parts.
    fn strip(&self) -> Result<(), E> {
        while let Some(composite) = self.consumer.consume()? {
            for part in P::strip(composite) {
                let _ = self.parts.produce(part);
            }
        }

        Ok(())
    }
}

impl<C, P, E> Consumer for Stripper<C, P, E>
where
    P: Strip<C>,
{
    type Good = P;
    type Error = E;

    #[inline]
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error> {
        self.strip()?;
        Ok(self.parts.consume().expect("consuming from parts queue"))
    }
}

impl<C, P, E> Debug for Stripper<C, P, E> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stripper {{ .. }}")
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

impl<G, E> FilterConsumer<G, E> {
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

impl<G, E> Consumer for FilterConsumer<G, E> {
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

impl<'a, G, E> FilterProducer<'a, G, E> {
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

impl<'a, G, E> Producer<'a> for FilterProducer<'a, G, E> {
    type Good = G;
    type Error = E;

    fn produce(&'a self, good: Self::Good) -> Result<(), Self::Error> {
        if self.validator.is_valid(&good) {
            self.producer.produce(good)
        } else {
            Ok(())
        }
    }
}

pub trait Writable {
    type Error;

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error>;
}

#[derive(Debug)]
pub struct Writer<G, W> {
    phantom: PhantomData<G>,
    writer: RefCell<W>,
}

impl<G, W> Writer<G, W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: RefCell::new(writer),
            phantom: PhantomData,
        }
    }
}

impl<G, W> Producer<'_> for Writer<G, W>
where
    G: Writable,
    <G as Writable>::Error: Debug + error::Error + 'static,
    W: Write,
{
    type Good = G;
    type Error = WriteGoodError<<Self::Good as Writable>::Error>;

    fn produce(&self, good: Self::Good) -> Result<(), Self::Error> {
        let mut writer = self.writer.borrow_mut();
        good.write_to(writer.by_ref())?;
        writer.flush().map_err(Self::Error::Flush)
    }
}

#[derive(Debug, Error)]
pub enum WriteGoodError<T: Debug + error::Error + 'static> {
    #[error("{0}")]
    Write(#[source] T),
    #[error("{0}")]
    Flush(#[source] io::Error),
}

impl<T: Debug + error::Error> From<T> for WriteGoodError<T> {
    fn from(value: T) -> Self {
        Self::Write(value)
    }
}

pub trait Readable
where
    Self: Sized,
{
    type Error: error::Error;

    fn from_bytes(bytes: &[u8]) -> (usize, Result<Self, Self::Error>);
}

#[derive(Debug)]
pub struct Reader<G, R> {
    phantom: PhantomData<G>,
    reader: RefCell<R>,
    buffer: RefCell<Vec<u8>>,
}

impl<G, R> Reader<G, R> {
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
    <G as Readable>::Error: Debug + 'static,
    R: BufRead,
{
    type Good = G;
    type Error = ReadGoodError<G>;

    fn consume(&self) -> Result<Option<Self::Good>, Self::Error> {
        let mut reader = self.reader.borrow_mut();
        let buf = reader.fill_buf()?;
        let consumed_len = buf.len();
        let mut buffer = self.buffer.borrow_mut();
        buffer.extend_from_slice(buf);
        let (processed_len, good) = G::from_bytes(&buffer);

        buffer.drain(..processed_len);
        reader.consume(consumed_len);
        good.map(Some).map_err(Self::Error::Deserialize)
    }
}

#[derive(Debug, Error)]
pub enum ReadGoodError<G>
where
    G: Readable + Debug,
    <G as Readable>::Error: Debug + 'static,
{
    #[error("{0}")]
    Read(#[from] io::Error),
    #[error("{0}")]
    Deserialize(#[source] <G as Readable>::Error),
}
