//! Implements `Consumer` and `Producer` for `std::io::Read` and `std::io::Write` trait objects.
use {
    crate::{
        channel::{CrossbeamConsumer, CrossbeamProducer},
        thread::Thread,
        ClosedMarketFault, ClassicalConsumerFailure, Consumer, ClassicalProducerFailure, Producer,
    },
    conventus::{AssembleFailure, AssembleFrom, DisassembleInto},
    core::{
        cell::RefCell,
        fmt::Debug,
        marker::PhantomData,
        sync::atomic::{AtomicBool, Ordering},
    },
    crossbeam_channel::TryRecvError,
    fehler::{throw, throws},
    log::{error, warn},
    std::{
        io::{self, ErrorKind, Read, Write},
        panic::UnwindSafe,
        sync::Arc,
        thread::{self, JoinHandle},
    },
};

/// A fault while reading a good of type `G`.
#[derive(Debug, thiserror::Error)]
pub enum ReadFault<G>
where
    G: AssembleFrom<u8> + Debug,
    <G as AssembleFrom<u8>>::Error: 'static,
{
    /// Unable to assemble the good from bytes.
    #[error("{0}")]
    // This cannot be #[from] as it conflicts with From<T> for T
    Assemble(#[source] <G as AssembleFrom<u8>>::Error),
    /// Reader was closed.
    #[error("reader was closed")]
    Closed,
}

impl<G> core::convert::TryFrom<ClassicalConsumerFailure<ReadFault<G>>> for ReadFault<G>
where
    G: AssembleFrom<u8> + Debug,
{
    type Error = ();

    #[inline]
    #[throws(Self::Error)]
    fn try_from(failure: ClassicalConsumerFailure<Self>) -> Self {
        if let ClassicalConsumerFailure::Fault(fault) = failure {
            fault
        } else {
            throw!(())
        }
    }
}

/// Consumes goods of type `G` from bytes read by an item implementing `std::io::Read`.
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
    pub fn new<R>(reader: R) -> Self
    where
        R: Read + Send + 'static,
    {
        Self {
            byte_consumer: ByteConsumer::new(reader),
            buffer: RefCell::new(Vec::new()),
            phantom: PhantomData,
        }
    }
}

impl<G> Consumer for Reader<G>
where
    G: AssembleFrom<u8> + Debug +'static,
    <G as AssembleFrom<u8>>::Error: 'static,
{
    type Good = G;
    type Failure = ClassicalConsumerFailure<ReadFault<G>>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        let mut bytes = self
            .byte_consumer
            .consume_all()
            .map_err(|_| ReadFault::Closed)?;
        let mut buffer = self.buffer.borrow_mut();
        buffer.append(&mut bytes);
        G::assemble_from(&mut buffer).map_err(|error| match error {
            AssembleFailure::Incomplete => ClassicalConsumerFailure::EmptyStock,
            AssembleFailure::Error(e) => ClassicalConsumerFailure::Fault(ReadFault::Assemble(e)),
        })?
    }
}

/// An error while reading a good of type `G`.
#[derive(Debug, thiserror::Error)]
pub enum ReadError<G>
where
    G: AssembleFrom<u8> + Debug,
    <G as AssembleFrom<u8>>::Error: 'static,
{
    /// Unable to assemble the good from bytes.
    #[error("{0}")]
    // This cannot be #[from] as it conflicts with From<T> for T
    Assemble(#[source] <G as AssembleFrom<u8>>::Error),
    /// Reader was closed.
    #[error("reader was closed")]
    Closed,
}

impl<G> From<ClosedMarketFault> for ReadError<G>
where
    G: AssembleFrom<u8> + Debug,
{
    #[inline]
    fn from(_: ClosedMarketFault) -> Self {
        Self::Closed
    }
}

/// Produces goods of type `G` by writing bytes via an item implementing `std::io::Write`.
#[derive(Debug)]
pub struct Writer<G> {
    /// The byte producer.
    // TODO: Move ByteProducer into Writer.
    byte_producer: ByteProducer,
    #[doc(hidden)]
    phantom: PhantomData<G>,
}

impl<G> Writer<G> {
    /// Creates a new `Writer` that strips bytes from goods and writes them using `writer`.
    #[inline]
    pub fn new<W>(writer: W) -> Self
    where
        W: Write + Send + UnwindSafe + 'static,
    {
        Self {
            byte_producer: ByteProducer::new(writer),
            phantom: PhantomData,
        }
    }
}

impl<G> Producer for Writer<G>
where
    G: DisassembleInto<u8> + Debug,
    <G as DisassembleInto<u8>>::Error: 'static,
{
    type Good = G;
    type Failure = ClassicalProducerFailure<WriteError<G>>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        self.byte_producer
            .produce_all(good.disassemble_into().map_err(WriteError::Disassemble)?)
            .map_err(ClassicalProducerFailure::map_into)?
    }
}

/// An error while writing a good of type `G`.
#[derive(Debug, thiserror::Error)]
pub enum WriteError<G>
where
    G: DisassembleInto<u8> + Debug,
    <G as DisassembleInto<u8>>::Error: 'static,
{
    /// Unable to disassemble the good into bytes.
    #[error("{0}")]
    // This cannot be #[from] as it conflicts with From<T> for T
    Disassemble(#[source] <G as DisassembleInto<u8>>::Error),
    /// Writer was closed.
    #[error("writer was closed")]
    Closed,
}

impl<G> core::convert::TryFrom<ClassicalProducerFailure<WriteError<G>>> for WriteError<G>
where
    G: DisassembleInto<u8> + Debug,
{
    type Error = ();

    #[inline]
    #[throws(Self::Error)]
    fn try_from(failure: ClassicalProducerFailure<Self>) -> Self {
        if let ClassicalProducerFailure::Fault(fault) = failure {
            fault
        } else {
            throw!(())
        }
    }
}

impl<G> From<ClosedMarketFault> for WriteError<G>
where
    G: DisassembleInto<u8> + Debug,
{
    #[inline]
    fn from(_: ClosedMarketFault) -> Self {
        Self::Closed
    }
}

/// Consumes bytes using an item that implements [`Read`].
///
/// Reading is done in a separate thread to ensure consume() is non-blocking.
#[derive(Debug)]
struct ByteConsumer {
    /// Consumes bytes from the reading thread.
    consumer: CrossbeamConsumer<u8>,
    /// The thread that reads bytes.
    join_handle: Option<JoinHandle<()>>,
    /// If the thread is quitting.
    is_quitting: Arc<AtomicBool>,
}

impl ByteConsumer {
    /// Creates a new [`ByteConsumer`].
    #[inline]
    fn new<R: Read + Send + 'static>(mut reader: R) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let is_quitting = Arc::new(AtomicBool::new(false));
        let quitting = Arc::clone(&is_quitting);

        let join_handle = thread::spawn(move || {
            let mut buffer = [0; 1024];
            let producer: CrossbeamProducer<u8> = tx.into();

            while !quitting.load(Ordering::Relaxed) {
                match reader.read(&mut buffer) {
                    Ok(len) => {
                        let (bytes, _) = buffer.split_at(len);

                        if let Err(error) = producer.force_all(bytes.to_vec()) {
                            error!("`ByteConsumer` failed to store bytes: {}", error);
                        }
                    }
                    Err(error) => {
                        warn!("`ByteConsumer` failed to read bytes: {}", error);
                    }
                }
            }
        });

        Self {
            consumer: rx.into(),
            join_handle: Some(join_handle),
            is_quitting,
        }
    }
}

impl Consumer for ByteConsumer {
    type Good = u8;
    type Failure = ClassicalConsumerFailure<ClosedMarketFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        self.consumer.consume()?
    }
}

impl Drop for ByteConsumer {
    #[inline]
    fn drop(&mut self) {
        self.is_quitting.store(true, Ordering::Relaxed);

        if let Some(Err(error)) = self.join_handle.take().map(JoinHandle::join) {
            error!("Unable to join `ByteConsumer` thread: {:?}", error);
        }
    }
}

/// Produces bytes using an item of type [`Write`].
///
/// Writing is done within a separate thread to ensure produce() is non-blocking.
#[derive(Debug)]
struct ByteProducer {
    /// Produces bytes to be written by the writing thread.
    producer: CrossbeamProducer<u8>,
    /// Consumes errors from the writing thread.
    error_consumer: CrossbeamConsumer<io::Error>,
    /// If `Self` is currently being dropped.
    is_dropping: Arc<AtomicBool>,
    /// The thread.
    thread: Thread<(), ClosedMarketFault>,
}

impl ByteProducer {
    /// Creates a new [`ByteProducer`].
    #[inline]
    fn new<W>(mut writer: W) -> Self
    where
        W: Write + Send + UnwindSafe + 'static,
    {
        let (tx, rx) = crossbeam_channel::unbounded();
        let (err_tx, err_rx) = crossbeam_channel::bounded(1);
        let is_dropping = Arc::new(AtomicBool::new(false));
        let is_quitting = Arc::clone(&is_dropping);

        let thread = Thread::new(move || {
            let mut buffer = Vec::new();

            while !is_quitting.load(Ordering::Relaxed) {
                loop {
                    match rx.try_recv() {
                        Ok(byte) => {
                            buffer.push(byte);
                        }
                        Err(TryRecvError::Empty) => {
                            break;
                        }
                        Err(TryRecvError::Disconnected) => {
                            if let Err(send_error) = err_tx.send(io::Error::new(
                                ErrorKind::Other,
                                "failed to retrieve bytes to write",
                            )) {
                                error!(
                                    "Unable to store `ByteProducer` receive error: {}",
                                    send_error.into_inner()
                                );
                            }

                            is_quitting.store(true, Ordering::Relaxed);
                        }
                    }
                }

                if !buffer.is_empty() {
                    if let Err(error) = writer.write_all(&buffer) {
                        if let Err(send_error) = err_tx.send(error) {
                            error!(
                                "Unable to store `ByteProducer` write error: {}",
                                send_error.into_inner()
                            );
                        }

                        is_quitting.store(true, Ordering::Relaxed);
                    }

                    buffer.clear();
                }
            }

            Ok(())
        });

        Self {
            producer: tx.into(),
            error_consumer: err_rx.into(),
            is_dropping,
            thread,
        }
    }
}

impl Drop for ByteProducer {
    #[inline]
    fn drop(&mut self) {
        self.is_dropping.store(true, Ordering::Relaxed);

        if let Err(error) = self.thread.demand() {
            error!("Unable to join `ByteProducer` thread: {:?}", error);
        }
    }
}

impl Producer for ByteProducer {
    type Good = u8;
    type Failure = ClassicalProducerFailure<ClosedMarketFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        self.producer.produce(good)?
    }
}
