//! A library to simplify development with producers and consumers.
//!
//! The core purpose of this library is to define the traits [`Consumer`] and [`Producer`]. These traits provide the functionality to send and receive items known as **goods**. The most basic implementations of these traits are the [`mpsc::Receiver`] and [`mpsc::Sender`] structs.
//!
//! [`Consumer`]: trait.Consumer.html
//! [`Producer`]: trait.Producer.html
use {
    core::fmt::{self, Debug, Display},
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
pub struct MpscConsumer<G: Debug> {
    /// The receiver.
    rx: mpsc::Receiver<G>,
    /// The next good to be consumed.
    ///
    /// Used for implementing [`can_consume`] as [`mpsc::Receiver`] does not provide the functionality of checking if an item can be received without actually receiving the item.
    next_good: Mutex<Option<G>>,
}

impl<G: Debug> Consumer for MpscConsumer<G> {
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

impl<G: Debug> From<mpsc::Receiver<G>> for MpscConsumer<G> {
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

impl<G: Debug> Consumer for CrossbeamConsumer<G> {
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
    is_closed: Mutex<bool>,
}

impl<G> UnlimitedQueue<G> {
    /// Creates a new empty [`UnlimitedQueue`].
    pub fn new() -> Self {
        Self::default()
    }

    pub fn close(&self) {
        let mut is_closed = self.is_closed.lock().unwrap();
        *is_closed = true;
    }
}

impl<G: Debug> Consumer for UnlimitedQueue<G> {
    type Good = G;
    type Error = QueueError;

    fn can_consume(&self) -> bool {
        match self.is_closed.lock() {
            Err(_) => true,
            Ok(is_closed) => *is_closed || self.queue.is_empty(),
        }
    }

    fn consume(&self) -> Result<Self::Good, Self::Error> {
        let mut consumed = Err(Self::Error::Closed);

        while consumed.is_err() {
            // Mutex ensures no good is added in between pop and checking is_closed.
            let is_closed = self.is_closed.lock().map_err(|_| Self::Error::Poisoned)?;

            consumed = self.queue.pop().map_err(|_| Self::Error::Closed);

            if *is_closed {
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
            is_closed: Mutex::new(false),
        }
    }
}

impl<'a, G> Producer<'a> for UnlimitedQueue<G> {
    type Good = G;
    type Error = QueueError;

    fn produce(&self, good: Self::Good) -> Result<(), Self::Error> {
        // See consume() for use of is_closed.
        let is_closed = self.is_closed.lock().map_err(|_| Self::Error::Poisoned)?;

        if *is_closed {
            Err(Self::Error::Closed)
        } else {
            Ok(self.queue.push(good))
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

impl<G: Debug, E: Error> Iterator for GoodsIter<'_, G, E> {
    type Item = G;

    fn next(&mut self) -> Option<Self::Item> {
        self.consumer.consume().ok()
    }
}
