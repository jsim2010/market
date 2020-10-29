//! Implements `Consumer` and `Producer` for `std::io::Read` and `std::io::Write` trait objects.
use {
    core::{
        cell::RefCell,
        fmt::Debug,
        marker::PhantomData,
        sync::atomic::{AtomicBool, Ordering},
    },
    fehler::{throw, throws},
    log::{error, warn},
    std::{
        io::{ErrorKind, Read, Write},
        panic::UnwindSafe,
        sync::Arc,
        thread::JoinHandle,
    },
};

/// A fault while reading a good of type `G`.
#[derive(Debug, thiserror::Error)]
pub enum ReadFault<G>
where
    G: conventus::AssembleFrom<u8> + Debug,
    <G as conventus::AssembleFrom<u8>>::Error: 'static,
{
    /// Unable to assemble the good from bytes.
    #[error("{0}")]
    // This cannot be #[from] as it conflicts with From<T> for T
    Assemble(#[source] <G as conventus::AssembleFrom<u8>>::Error),
    /// Reader was closed.
    #[error("reader was closed")]
    Closed,
}

impl<G> core::convert::TryFrom<crate::error::ConsumerFailure<ReadFault<G>>> for ReadFault<G>
where
    G: conventus::AssembleFrom<u8> + Debug,
{
    type Error = ();

    #[inline]
    #[throws(Self::Error)]
    fn try_from(failure: crate::error::ConsumerFailure<Self>) -> Self {
        if let crate::error::ConsumerFailure::Fault(fault) = failure {
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

impl<G> crate::Consumer for Reader<G>
where
    G: conventus::AssembleFrom<u8> + Debug +'static,
    <G as conventus::AssembleFrom<u8>>::Error: 'static,
{
    type Good = G;
    type Failure = crate::error::ConsumerFailure<ReadFault<G>>;

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
            conventus::AssembleFailure::Incomplete => crate::error::ConsumerFailure::EmptyStock,
            conventus::AssembleFailure::Error(e) => crate::error::ConsumerFailure::Fault(ReadFault::Assemble(e)),
        })?
    }
}

/// An error while reading a good of type `G`.
#[derive(Debug, thiserror::Error)]
pub enum ReadError<G>
where
    G: conventus::AssembleFrom<u8> + Debug,
    <G as conventus::AssembleFrom<u8>>::Error: 'static,
{
    /// Unable to assemble the good from bytes.
    #[error("{0}")]
    // This cannot be #[from] as it conflicts with From<T> for T
    Assemble(#[source] <G as conventus::AssembleFrom<u8>>::Error),
    /// Reader was closed.
    #[error("reader was closed")]
    Closed,
}

impl<G> From<crate::error::ClosedMarketFault> for ReadError<G>
where
    G: conventus::AssembleFrom<u8> + Debug,
{
    #[inline]
    fn from(_: crate::error::ClosedMarketFault) -> Self {
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

impl<G> crate::Producer for Writer<G>
where
    G: conventus::DisassembleInto<u8> + Debug,
    <G as conventus::DisassembleInto<u8>>::Error: 'static,
{
    type Good = G;
    type Failure = crate::error::ProducerFailure<WriteError<G>>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        self.byte_producer
            .produce_all(good.disassemble_into().map_err(WriteError::Disassemble)?)
            .map_err(crate::error::ProducerFailure::map_into)?
    }
}

/// An error while writing a good of type `G`.
#[derive(Debug, thiserror::Error)]
pub enum WriteError<G>
where
    G: conventus::DisassembleInto<u8> + Debug,
    <G as conventus::DisassembleInto<u8>>::Error: 'static,
{
    /// Unable to disassemble the good into bytes.
    #[error("{0}")]
    // This cannot be #[from] as it conflicts with From<T> for T
    Disassemble(#[source] <G as conventus::DisassembleInto<u8>>::Error),
    /// Writer was closed.
    #[error("writer was closed")]
    Closed,
}

impl<G> core::convert::TryFrom<crate::error::ProducerFailure<WriteError<G>>> for WriteError<G>
where
    G: conventus::DisassembleInto<u8> + Debug,
{
    type Error = ();

    #[inline]
    #[throws(Self::Error)]
    fn try_from(failure: crate::error::ProducerFailure<Self>) -> Self {
        if let crate::error::ProducerFailure::Fault(fault) = failure {
            fault
        } else {
            throw!(())
        }
    }
}

impl<G> From<crate::error::ClosedMarketFault> for WriteError<G>
where
    G: conventus::DisassembleInto<u8> + Debug,
{
    #[inline]
    fn from(_: crate::error::ClosedMarketFault) -> Self {
        Self::Closed
    }
}

/// Consumes bytes using an item that implements [`Read`].
///
/// Reading is done in a separate thread to ensure consume() is non-blocking.
#[derive(Debug)]
struct ByteConsumer {
    /// Consumes bytes from the reading thread.
    consumer: crate::channel::CrossbeamConsumer<u8>,
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

        let join_handle = std::thread::spawn(move || {
            let mut buffer = [0; 1024];
            let producer: crate::channel::CrossbeamProducer<u8> = tx.into();

            while !quitting.load(Ordering::Relaxed) {
                match reader.read(&mut buffer) {
                    Ok(len) => {
                        let (bytes, _) = buffer.split_at(len);

                        if let Err(error) = {
                            use crate::Producer as _;
                            producer.force_all(bytes.to_vec())
                        } {
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

impl crate::Consumer for ByteConsumer {
    type Good = u8;
    type Failure = crate::error::ConsumerFailure<crate::error::ClosedMarketFault>;

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
    producer: crate::channel::CrossbeamProducer<u8>,
    /// Consumes errors from the writing thread.
    error_consumer: crate::channel::CrossbeamConsumer<std::io::Error>,
    /// If `Self` is currently being dropped.
    is_dropping: Arc<AtomicBool>,
    /// The thread.
    thread: crate::thread::Thread<(), crate::error::ClosedMarketFault>,
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

        let thread = crate::thread::Thread::new(move || {
            let mut buffer = Vec::new();

            while !is_quitting.load(Ordering::Relaxed) {
                loop {
                    match rx.try_recv() {
                        Ok(byte) => {
                            buffer.push(byte);
                        }
                        Err(crossbeam_channel::TryRecvError::Empty) => {
                            break;
                        }
                        Err(crossbeam_channel::TryRecvError::Disconnected) => {
                            if let Err(send_error) = err_tx.send(std::io::Error::new(
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

        if let Err(error) = {
            use crate::Consumer as _;
            self.thread.demand()
        } {
            error!("Unable to join `ByteProducer` thread: {:?}", error);
        }
    }
}

impl crate::Producer for ByteProducer {
    type Good = u8;
    type Failure = crate::error::ProducerFailure<crate::error::ClosedMarketFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        self.producer.produce(good)?
    }
}
