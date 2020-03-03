//! A library to simplify development with producers and consumers.
//!
//! The core purpose of this library is to define the traits [`Consumer`] and [`Producer`]. These traits provide the functionality to send and receive items known as **goods**. The most basic implementations of these traits are the [`mpsc::Receiver`] and [`mpsc::Sender`] structs.
//!
//! [`Consumer`]: trait.Consumer.html
//! [`Producer`]: trait.Producer.html
use {
    core::{sync::atomic::{AtomicBool, Ordering}, fmt::{self, Debug, Display}},
    crossbeam_channel::SendError,
    crossbeam_queue::SegQueue,
    std::{error::Error, sync::{Mutex, mpsc::{self, TryRecvError}}},
};

/// Retrieves goods that have been produced.
///
/// Because retrieving a good is failable, a [`Consumer`] actually retrieves a `Result<Self::Good, Self::Error>`.
pub trait Consumer: Debug {
    /// The type of the item returned by a successful consumption.
    type Good: Debug;
    /// The type of an error during consumption.
    type Error: Error;

    /// Returns if [`consume`] will return immediately.
    ///
    /// Note that returning true does not indicate that the returned value of [`consume`] will be [`Ok`].
    fn can_consume(&self) -> bool;
    /// Returns the next consumable, blocking the current thread if needed.
    ///
    /// The definition of **next** shall be defined by the implementing item.
    fn consume(&self) -> Result<Self::Good, Self::Error>;

    /// Attempts to consume a good without blocking the current thread.
    ///
    /// Returning [`None`] indicates that no consumable was found.
    fn optional_consume(&self) -> Option<Result<Self::Good, Self::Error>> {
        if self.can_consume() {
            Some(self.consume())
        } else {
            None
        }
    }

    /// Returns an [`Iterator`] that yields goods, blocking the current thread if needed.
    ///
    /// The returned [`Iterator`] shall yield [`None`] if and only if an error occurs during consumption.
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
    fn produce(&'a self, good: Self::Good) -> Result<(), Self::Error>;
}

/// Defines a [`mpsc::Receiver`] that implements [`Consumer`].
#[derive(Debug)]
pub struct MpscConsumer<G>
{
    /// The receiver.
    rx: mpsc::Receiver<G>,
    /// The next good to be consumed.
    ///
    /// Used for implementing [`can_consume`] as [`mpsc::Receiver`] does not provide the functionality of checking if an item can be received without actually receiving the item.
    next_good: Mutex<Option<G>>,
}

impl<G> Consumer for MpscConsumer<G>
where
    G: Debug,
{
    type Good = G;
    type Error = mpsc::RecvError;

    fn can_consume(&self) -> bool {
        let mut next_good = self.next_good.lock().unwrap();

        next_good.is_some()
            || match self.rx.try_recv() {
                Err(TryRecvError::Disconnected) => true,
                Err(TryRecvError::Empty) => false,
                Ok(good) => {
                    let _ = next_good.replace(good);
                    true
                }
            }
    }

    fn consume(&self) -> Result<Self::Good, Self::Error> {
        self.next_good.lock().unwrap().take().map(Ok).unwrap_or_else(|| self.rx.recv())
    }
}

impl<G> From<mpsc::Receiver<G>> for MpscConsumer<G>
where
    G: Debug,
{
    fn from(value: mpsc::Receiver<G>) -> Self {
        Self {
            rx: value,
            next_good: Mutex::new(None),
        }
    }
}

/// Defines a [`crossbeam_channel::Receiver`] that implements [`Consumer`].
#[derive(Debug)]
pub struct CrossbeamConsumer<G> {
    /// The [`crossbeam_channel::Recevier`].
    rx: crossbeam_channel::Receiver<G>,
}

impl<G> Consumer for CrossbeamConsumer<G>
where
    G: Debug,
{
    type Good = G;
    type Error = crossbeam_channel::RecvError;

    fn can_consume(&self) -> bool {
        !self.rx.is_empty()
    }

    fn consume(&self) -> Result<Self::Good, Self::Error> {
        self.rx.recv()
    }
}

impl<G> From<crossbeam_channel::Receiver<G>> for CrossbeamConsumer<G> {
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

impl<'a, G> Producer<'a> for CrossbeamProducer<G> {
    type Good = G;
    type Error = SendError<G>;

    fn produce(&self, record: Self::Good) -> Result<(), Self::Error> {
        self.tx.send(record)
    }
}

impl<G> From<crossbeam_channel::Sender<G>> for CrossbeamProducer<G> {
    fn from(value: crossbeam_channel::Sender<G>) -> Self {
        Self { tx: value }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum QueueError {
    Poisoned,
    Closed,
}

impl Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "queue is {}", match self {
            Self::Closed => "closed",
            Self::Poisoned => "poisoned",
        })
    }
}

impl Error for QueueError {
}

/// Defines a [`crossbeam_queue::SegQueue`] that implements [`Consumer`] and [`Producer`].
#[derive(Debug)]
pub struct UnlimitedQueue<G> {
    /// The queue.
    queue: SegQueue<G>,
    /// If the queue is closed.
    is_closed: AtomicBool,
}

impl<G> UnlimitedQueue<G> {
    /// Creates a new empty [`UnlimitedQueue`].
    pub fn new() -> Self {
        Self::default()
    }

    pub fn close(&self) {
        self.is_closed.store(false, Ordering::Relaxed);
    }
}

impl<G> Consumer for UnlimitedQueue<G>
where
    G: Debug,
{
    type Good = G;
    type Error = QueueError;

    fn can_consume(&self) -> bool {
        self.is_closed.load(Ordering::Relaxed) || !self.queue.is_empty()
    }

    fn consume(&self) -> Result<Self::Good, Self::Error> {
        let mut consumed = Err(Self::Error::Closed);

        while consumed.is_err() {
            // Call pop after loading is_closed to ensure no goods have been added prior to closing.
            let is_closed = self.is_closed.load(Ordering::Relaxed);

            consumed = self.queue.pop().map_err(|_| Self::Error::Closed);

            if is_closed {
                break;
            }
        }

        consumed
    }
}

impl<G> Default for UnlimitedQueue<G> {
    fn default() -> Self {
        Self {
            queue: SegQueue::new(),
            is_closed: AtomicBool::new(false),
        }
    }
}

impl<'a, G> Producer<'a> for UnlimitedQueue<G> {
    type Good = G;
    type Error = QueueError;

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
#[derive(Debug)]
pub struct GoodsIter<'a, G, E> {
    /// The consumer.
    consumer: &'a dyn Consumer<Good = G, Error = E>,
}

impl<G, E> Iterator for GoodsIter<'_, G, E>
where
    G: Debug,
    E: Error,
{
    type Item = G;

    fn next(&mut self) -> Option<Self::Item> {
        self.consumer.consume().ok()
    }
}

pub trait GoodFinisher {
    type Intermediate;
    type Final;

    fn finish(&self, intermediate_good: Self::Intermediate) -> Vec<Self::Final>;
}

#[derive(Debug)]
pub struct IntermediateConsumer<I, E, F, G>
{
    consumer: Box<dyn Consumer<Good=I, Error=E>>,
    queue: UnlimitedQueue<G>,
    finisher: F,
}

impl<I, E, F, G> IntermediateConsumer<I, E, F, G>
where
    I: Debug,
    E: Error,
    F: GoodFinisher<Intermediate=I,Final=G>,
    G: Debug,
{
    pub fn new(consumer: impl Consumer<Good=I, Error=E> + 'static, finisher: F) -> Self {
        Self {
            consumer: Box::new(consumer),
            queue: UnlimitedQueue::new(),
            finisher,
        }
    }

    fn process(&self) {
        while let Some(Ok(intermediate_good)) = self.consumer.optional_consume() {
            for finished_good in self.finisher.finish(intermediate_good) {
                let _ = self.queue.produce(finished_good);
            }
        }
    }
}

impl<I, E, F, G> Consumer for IntermediateConsumer<I, E, F, G>
where
    I: Debug,
    E: Debug + Error,
    F: Debug + GoodFinisher<Intermediate=I,Final=G>,
    G: Debug,
{
    type Good = G;
    type Error = <UnlimitedQueue<G> as Consumer>::Error;

    fn can_consume(&self) -> bool {
        self.process();
        self.queue.can_consume()
    }

    fn consume(&self) -> Result<Self::Good, Self::Error> {
        while !self.can_consume() {
            self.process();
        }

        self.queue.consume()
    }
}
