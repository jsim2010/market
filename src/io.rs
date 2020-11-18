//! Implements [`Producer`] and [`Consumer`] for [`std::io::Write`] and [`std::io::Read`] trait objects.
use {
    crate::{
        channel, sync, thread, ConsumeFailure, ConsumeFault, Consumer, Failure, ProduceFailure,
        ProduceFault, Producer, TakenParticipant,
    },
    core::{
        cell::RefCell,
        convert::TryFrom,
        fmt::{Debug, Display},
        marker::PhantomData,
    },
    fehler::{throw, throws},
    log::error,
    std::{
        error::Error,
        io::{Read, Write},
        panic::UnwindSafe,
    },
};

/// An error thrown by the read thread.
#[derive(Debug, thiserror::Error)]
pub enum ReadThreadError {
    /// Reader threw an error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// [`Consumer`] to which the read thread is sending bytes was dropped.
    #[error(transparent)]
    Closed(#[from] channel::DisconnectedFault),
}

/// A fault thrown by [`ByteConsumer`].
#[derive(ConsumeFault, Debug, thiserror::Error)]
pub enum ReadBytesFault {
    /// Read thread threw an error.
    #[error(transparent)]
    Thread(#[from] thread::Fault<ReadThreadError>),
    /// [`Producer`] in read thread was dropped.
    #[error(transparent)]
    Channel(#[from] channel::DisconnectedFault),
}

impl From<thread::Fault<ReadThreadError>> for ConsumeFailure<ReadBytesFault> {
    #[inline]
    fn from(fault: thread::Fault<ReadThreadError>) -> Self {
        Self::Fault(fault.into())
    }
}

/// Consumes bytes using a [`std::io::Read`] trait object.
///
/// Bytes are read in a separate thread to ensure [`consume()`] is non-blocking.
#[derive(Debug)]
struct ByteConsumer {
    /// Consumes bytes that have been read.
    consumer: channel::CrossbeamConsumer<u8>,
    /// The thread that reads bytes.
    thread: thread::Thread<(), ReadThreadError>,
    /// Triggers termination of the thread.
    terminator: sync::Trigger,
}

impl ByteConsumer {
    /// Creates a new [`ByteConsumer`].
    #[inline]
    #[throws(TakenParticipant)]
    fn new<R: Read + Send + UnwindSafe + 'static>(mut reader: R) -> Self {
        let mut channel = channel::Crossbeam::new(channel::Size::Infinite);
        let mut lock = sync::Lock::new();
        let producer = channel.producer()?;
        let hammer = lock.hammer()?;

        Self {
            consumer: channel.consumer()?,
            thread: thread::Thread::new(move || {
                let mut buffer = [0; 1024];

                while hammer.consume().is_err() {
                    let len = reader.read(&mut buffer)?;
                    let (bytes, _) = buffer.split_at(len);

                    producer.force_all(bytes.to_vec())?;
                }

                Ok(())
            }),
            terminator: lock.trigger()?,
        }
    }

    /// Terminates the read thread.
    #[allow(unused_must_use)] // Trigger::produce() cannot fail.
    #[throws(thread::Fault<ReadThreadError>)]
    fn terminate(&self) {
        self.terminator.produce(());

        self.thread.demand()
    }
}

impl Consumer for ByteConsumer {
    type Good = u8;
    type Failure = ConsumeFailure<ReadBytesFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        let consumption = self.consumer.consume();

        if let Err(ConsumeFailure::Fault(_)) = consumption {
            // If there is a fault in the consumption, it should be because the thread threw an error. A thread error provides more detailed information about the fault so throw that if possible.
            if let Err(ConsumeFailure::Fault(thread_fault)) = self.thread.consume() {
                throw!(thread_fault);
            }
        }

        consumption.map_err(Self::Failure::map_from)?
    }
}

/// A fault while reading a good of type `G`.
#[derive(ConsumeFault, Debug)]
pub enum ReadFault<G>
where
    G: conventus::AssembleFrom<u8>,
{
    /// [`ByteConsumer`] threw a fault.
    Read(ReadBytesFault),
    /// Unable to assemble the good from bytes.
    Assemble(<G as conventus::AssembleFrom<u8>>::Error),
}

impl<G: Display> Display for ReadFault<G>
where
    G: conventus::AssembleFrom<u8>,
{
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::Read(ref fault) => write!(f, "{}", fault),
            Self::Assemble(ref error) => write!(f, "{}", error),
        }
    }
}

impl<G: Debug + Display> Error for ReadFault<G> where G: conventus::AssembleFrom<u8> {}

impl<G> From<ReadBytesFault> for ConsumeFailure<ReadFault<G>>
where
    G: conventus::AssembleFrom<u8>,
{
    #[inline]
    fn from(fault: ReadBytesFault) -> Self {
        Self::Fault(ReadFault::Read(fault))
    }
}

/// Consumes goods of type `G` from bytes read by an `std::io::Read` trait object.
#[derive(Debug)]
pub struct Reader<G> {
    /// The consumer.
    byte_consumer: ByteConsumer,
    /// The current buffer of bytes.
    buffer: RefCell<Vec<u8>>,
    #[doc(hidden)]
    phantom: PhantomData<G>,
}

impl<G> Reader<G> {
    /// Creates a new `Reader` that composes goods from the bytes consumed by `reader`.
    #[inline]
    #[throws(TakenParticipant)]
    pub fn new<R>(reader: R) -> Self
    where
        R: Read + Send + UnwindSafe + 'static,
    {
        Self {
            byte_consumer: ByteConsumer::new(reader)?,
            buffer: RefCell::new(Vec::new()),
            phantom: PhantomData,
        }
    }

    /// Terminates the read thread.
    #[inline]
    #[throws(thread::Fault<ReadThreadError>)]
    pub fn terminate(&self) {
        self.byte_consumer.terminate()?
    }
}

impl<G> Consumer for Reader<G>
where
    G: conventus::AssembleFrom<u8>,
    ReadFault<G>: TryFrom<ConsumeFailure<ReadFault<G>>>,
{
    type Good = G;
    type Failure = ConsumeFailure<ReadFault<G>>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        let mut bytes = self.byte_consumer.consume_all()?;
        let mut buffer = self.buffer.borrow_mut();
        buffer.append(&mut bytes);
        G::assemble_from(&mut buffer).map_err(|error| match error {
            conventus::AssembleFailure::Incomplete => ConsumeFailure::EmptyStock,
            conventus::AssembleFailure::Error(e) => ConsumeFailure::Fault(ReadFault::Assemble(e)),
        })?
    }
}

/// An error thrown in the write thread.
#[derive(Debug, thiserror::Error)]
pub enum WriteThreadError {
    /// The write was unsuccessful.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The [`Producer`] sending bytes to the write thread was dropped.
    #[error(transparent)]
    Closed(#[from] channel::DisconnectedFault),
}

/// Produces bytes using a [`std::io::Write`] trait object.
///
/// Writing is done within a separate thread to ensure [`produce()`] is non-blocking.
#[derive(Debug)]
struct ByteProducer {
    /// Produces bytes to be written by the writing thread.
    producer: channel::CrossbeamProducer<u8>,
    /// Triggers the termination of the thread.
    terminator: sync::Trigger,
    /// The thread.
    thread: thread::Thread<(), WriteThreadError>,
}

impl ByteProducer {
    /// Creates a new [`ByteProducer`].
    #[inline]
    #[throws(TakenParticipant)]
    fn new<W>(mut writer: W) -> Self
    where
        W: Write + Send + UnwindSafe + 'static,
    {
        let mut channel = channel::Crossbeam::new(channel::Size::Infinite);
        let mut lock = sync::Lock::new();
        let hammer = lock.hammer()?;
        let consumer = channel.consumer()?;

        let thread = thread::Thread::new(move || {
            while hammer.consume().is_err() {
                writer.write_all(&consumer.consume_all()?)?;
            }

            Ok(())
        });

        Self {
            producer: channel.producer()?,
            terminator: lock.trigger()?,
            thread,
        }
    }

    /// Terminates the write thread.
    #[allow(unused_must_use)] // Trigger::produce() cannot fail.
    #[throws(thread::Fault<WriteThreadError>)]
    fn terminate(&self) {
        self.terminator.produce(());

        self.thread.demand()
    }
}

impl Producer for ByteProducer {
    type Good = u8;
    type Failure = ProduceFailure<channel::DisconnectedFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        self.producer.produce(good)?
    }
}

/// Produces goods of type `G` by writing bytes via an item implementing `std::io::Write`.
#[derive(Debug)]
pub struct Writer<G> {
    /// The byte producer.
    byte_producer: ByteProducer,
    #[doc(hidden)]
    phantom: PhantomData<G>,
}

impl<G> Writer<G> {
    /// Creates a new `Writer` that strips bytes from goods and writes them using `writer`.
    #[inline]
    #[throws(TakenParticipant)]
    pub fn new<W>(writer: W) -> Self
    where
        W: Write + Send + UnwindSafe + 'static,
    {
        Self {
            byte_producer: ByteProducer::new(writer)?,
            phantom: PhantomData,
        }
    }

    /// Terminates the writing thread.
    #[inline]
    #[throws(thread::Fault<WriteThreadError>)]
    pub fn terminate(&self) {
        self.byte_producer.terminate()?
    }
}

impl<G> Producer for Writer<G>
where
    G: conventus::DisassembleInto<u8>,
    WriteError<G>: TryFrom<ProduceFailure<WriteError<G>>>,
{
    type Good = G;
    type Failure = ProduceFailure<WriteError<G>>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        self.byte_producer
            .produce_all(good.disassemble_into().map_err(WriteError::Disassemble)?)
            .map_err(ProduceFailure::map_into)?
    }
}

/// An error while writing a good of type `G`.
#[derive(ProduceFault, Debug)]
pub enum WriteError<G>
where
    G: conventus::DisassembleInto<u8>,
{
    /// Unable to disassemble the good into bytes.
    Disassemble(<G as conventus::DisassembleInto<u8>>::Error),
    /// Writer was closed.
    Closed,
}

impl<G: Display> Display for WriteError<G>
where
    G: conventus::DisassembleInto<u8>,
{
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::Disassemble(ref error) => write!(f, "{}", error),
            Self::Closed => write!(f, "writer was closed"),
        }
    }
}

impl<G: Debug + Display> Error for WriteError<G> where G: conventus::DisassembleInto<u8> {}

impl<G> From<channel::DisconnectedFault> for WriteError<G>
where
    G: conventus::DisassembleInto<u8>,
{
    #[inline]
    fn from(_: channel::DisconnectedFault) -> Self {
        Self::Closed
    }
}
