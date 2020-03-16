//! A library to simplify development with producers and consumers.
//!
//! The core purpose of this library is to define the traits [`Consumer`] and [`Producer`]. These traits provide the functionality to send and receive items known as **goods**.
//!
//! [`Consumer`]: trait.Consumer.html
//! [`Producer`]: trait.Producer.html
use {
    core::{
        cell::RefCell,
        fmt::{self, Debug, Display},
        marker::PhantomData,
        sync::atomic::{AtomicBool, Ordering},
    },
    crossbeam_channel::SendError,
    crossbeam_queue::SegQueue,
    std::{
        error::Error,
        io::{self, Write, BufRead},
        sync::mpsc,
    },
};

/// Retrieves goods that have been produced.
///
/// Because retrieving a good is failable, a [`Consumer`] actually retrieves a `Result<Self::Good, Self::Error>`.
pub trait Consumer {
    /// The type of the item returned by a successful consumption.
    type Good;
    /// The type of an error during consumption.
    type Error;

    /// Attempts to consume the next good without blocking the current thread.
    ///
    /// The definition of **next** shall be defined by the implementing item.
    ///
    /// Returning [`None`] indicates that no consumable was found.
    fn consume(&self) -> Option<Result<Self::Good, Self::Error>>;

    /// Continues to attempt consumption until a consumable is found.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] when consumer fails to consume a good.
    #[inline]
    fn demand(&self) -> Result<Self::Good, Self::Error> {
        loop {
            if let Some(result) = self.consume() {
                return result;
            }
        }
    }

    /// Returns an [`Iterator`] that yields goods, blocking the current thread if needed.
    ///
    /// The returned [`Iterator`] shall yield [`None`] if and only if an error occurs during consumption.
    #[inline]
    fn goods(&self) -> GoodsIter<'_, Self::Good, Self::Error>
    where
        Self: Sized,
    {
        GoodsIter { consumer: self }
    }
}

/// Generates goods to be consumed.
pub trait Producer<'a> {
    /// The type of the item being produced.
    type Good;
    /// The type of an error during production.
    type Error;

    /// Transfers `good` to be consumed, blocking the current thread if needed.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] when producer fails to produce `good`.
    fn produce(&'a self, good: Self::Good) -> Result<(), Self::Error>;
}

/// An error consuming a good.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConsumeGoodError {
    /// The market is closed.
    Closed,
}

impl Display for ConsumeGoodError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cannot consume from closed market")
    }
}

impl Error for ConsumeGoodError {}

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
    fn consume(&self) -> Option<Result<Self::Good, Self::Error>> {
        match self.rx.try_recv() {
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(Self::Error::Closed)),
            Ok(good) => Some(Ok(good)),
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
    fn consume(&self) -> Option<Result<Self::Good, Self::Error>> {
        match self.rx.try_recv() {
            Err(crossbeam_channel::TryRecvError::Empty) => None,
            Err(crossbeam_channel::TryRecvError::Disconnected) => Some(Err(Self::Error::Closed)),
            Ok(good) => Some(Ok(good)),
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
#[derive(Clone, Copy, Debug)]
pub enum ProduceGoodError {
    /// An error consuming or producing with a closed queue.
    Closed,
}

impl Display for ProduceGoodError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "queue is closed")
    }
}

impl Error for ProduceGoodError {}

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
    fn consume(&self) -> Option<Result<Self::Good, Self::Error>> {
        match self.queue.pop() {
            Ok(good) => Some(Ok(good)),
            Err(_) => {
                if self.is_closed.load(Ordering::Relaxed) {
                    Some(Err(Self::Error::Closed))
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

/// An [`Iterator`] that yields consumed goods, blocking the current thread if needed.
///
/// Shall yield [`None`] if and only if an error occurs during consumption.
pub struct GoodsIter<'a, G, E> {
    /// The consumer.
    consumer: &'a dyn Consumer<Good = G, Error = E>,
}

impl<G, E> Debug for GoodsIter<'_, G, E> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GoodsIter {{ .. }}")
    }
}

impl<G, E> Iterator for GoodsIter<'_, G, E> {
    type Item = G;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.consumer.demand().ok()
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
    fn strip(&self) {
        while let Some(Ok(composite)) = self.consumer.consume() {
            for part in P::strip(composite) {
                let _ = self.parts.produce(part);
            }
        }
    }
}

impl<C, P, E> Consumer for Stripper<C, P, E>
where
    P: Strip<C>,
{
    type Good = P;
    type Error = <UnlimitedQueue<P> as Consumer>::Error;

    #[inline]
    fn consume(&self) -> Option<Result<Self::Good, Self::Error>> {
        self.strip();
        self.parts.consume()
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
    fn consume(&self) -> Option<Result<Self::Good, Self::Error>> {
        while let Some(input_consumption) = self.consumer.consume() {
            match input_consumption {
                Ok(input) => {
                    if self.validator.is_valid(&input) {
                        return Some(Ok(input));
                    }
                }
                Err(error) => {
                    return Some(Err(error));
                }
            }
        }

        None
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

pub struct Writer<G, W> {
    phantom: PhantomData<G>,
    writer: RefCell<W>,
}

impl<G, W> Writer<G, W> {
    pub fn new(writer: W) -> Self
    where
        W: Write,
    {
        Self {
            writer: RefCell::new(writer),
            phantom: PhantomData,
        }
    }
}

impl<G, W> Debug for Writer<G, W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Writer {{ .. }}")
    }
}

impl<G, W> Producer<'_> for Writer<G, W>
where
    G: Writable,
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

#[derive(Debug)]
pub enum WriteGoodError<T> {
    Write(T),
    Flush(io::Error),
}

impl<T> Display for WriteGoodError<T>
where
    T: Display,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(error) => write!(f, "unable to write good: {}", error),
            Self::Flush(error) => write!(f, "unable to flush: {}", error),
        }
    }
}

impl<T> From<T> for WriteGoodError<T> {
    fn from(value: T) -> Self {
        Self::Write(value)
    }
}

impl<T> Error for WriteGoodError<T> where T: Debug + Display {}

pub trait Readable
where
    Self: Sized,
{
    type Error;

    fn from_bytes(bytes: &[u8]) -> (usize, Result<Self, Self::Error>);
}

pub struct Reader<G, R> {
    phantom: PhantomData<G>,
    reader: RefCell<R>,
    buffer: RefCell<Vec<u8>>,
}

impl<G, R> Consumer for Reader<G, R>
where
    G: Readable,
    R: BufRead,
{
    type Good = G;
    type Error = io::Error;

    fn consume(&self) -> Option<Result<Self::Good, Self::Error>> {
        match self.reader.borrow_mut().fill_buf() {
            Ok(buf) => {
                self.buffer.borrow_mut().extend_from_slice(buf);
                let (processed_len, good) = G::from_bytes(&self.buffer.borrow());

                self.buffer.borrow_mut().drain(..processed_len);
                self.reader.borrow_mut().consume(processed_len);
                good.ok().map(Ok)
            }
            Err(error) => Some(Err(error)),
        }
    }
}
