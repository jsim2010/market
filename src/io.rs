//! Implements `Consumer` and `Producer` for `std::io::Read` and `std::io::Write` trait objects.
use {
    crate::{
        channel::{CrossbeamConsumer, CrossbeamProducer},
        ClosedMarketError, ComposeFrom, ComposingConsumer, ConsumeFailure, Consumer,
        ProduceFailure, Producer, StripFrom, StrippingProducer,
    },
    core::{
        fmt::{Debug, Display},
        sync::atomic::{AtomicBool, Ordering},
    },
    crossbeam_channel::TryRecvError,
    fehler::throws,
    log::{error, warn},
    std::{
        io::{self, ErrorKind, Read, Write},
        sync::Arc,
        thread::{self, JoinHandle},
    },
};

/// Consumes goods of type `G` from bytes read by an item implementing `std::io::Read`.
#[derive(Debug)]
pub struct Reader<G> {
    /// The consumer.
    consumer: ComposingConsumer<ByteConsumer, G>,
}

impl<G> Reader<G> {
    /// Creates a new `Reader` that composes goods from the bytes consumed by `reader`.
    #[inline]
    pub fn new<R>(reader: R) -> Self
    where
        R: Read + Send + 'static,
    {
        Self {
            consumer: ComposingConsumer::new(ByteConsumer::new(reader)),
        }
    }
}

impl<G> Consumer for Reader<G>
where
    G: ComposeFrom<u8>,
{
    type Good = G;
    // This is equivalent to <ByteConsumer as Consumer>::Error. ClosedMarketError is prefered in order to keep ByteConsumer private.
    type Error = ClosedMarketError;

    #[inline]
    #[throws(ConsumeFailure<Self::Error>)]
    fn consume(&self) -> Self::Good {
        self.consumer.consume()?
    }
}

/// Produces goods of type `G` by writing bytes via an item implementing `std::io::Write`.
#[derive(Debug)]
pub struct Writer<G> {
    /// The producer.
    producer: StrippingProducer<G, ByteProducer>,
}

impl<G> Writer<G> {
    /// Creates a new `Writer` that strips bytes from goods and writes them using `writer`.
    #[inline]
    pub fn new<W>(writer: W) -> Self
    where
        W: Write + Send + 'static,
    {
        Self {
            producer: StrippingProducer::new(ByteProducer::new(writer)),
        }
    }
}

impl<G> Producer for Writer<G>
where
    u8: StripFrom<G>,
    G: Debug + Display,
{
    type Good = G;
    // This is equivalent to <ByteProducer as Producer>::Error. ClosedMarketError is prefered in order to keep ByteProducer private.
    type Error = ClosedMarketError;

    #[inline]
    #[throws(ProduceFailure<Self::Error>)]
    fn produce(&self, good: Self::Good) {
        self.producer.produce(good)?
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
    type Error = ClosedMarketError;

    #[inline]
    #[throws(ConsumeFailure<Self::Error>)]
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
    /// The handle to join the thread that processes writes.
    join_handle: Option<JoinHandle<()>>,
    /// If `Self` is currently being dropped.
    is_dropping: Arc<AtomicBool>,
}

impl ByteProducer {
    /// Creates a new [`ByteProducer`].
    #[inline]
    fn new<W>(mut writer: W) -> Self
    where
        W: Write + Send + 'static,
    {
        let (tx, rx) = crossbeam_channel::unbounded();
        let (err_tx, err_rx) = crossbeam_channel::bounded(1);
        let is_dropping = Arc::new(AtomicBool::new(false));
        let is_quitting = Arc::clone(&is_dropping);

        let join_handle = thread::spawn(move || {
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
        });

        Self {
            producer: tx.into(),
            error_consumer: err_rx.into(),
            join_handle: Some(join_handle),
            is_dropping,
        }
    }
}

impl Drop for ByteProducer {
    #[inline]
    fn drop(&mut self) {
        self.is_dropping.store(true, Ordering::Relaxed);

        if let Some(Err(error)) = self.join_handle.take().map(JoinHandle::join) {
            error!("Unable to join `ByteProducer` thread: {:?}", error);
        }
    }
}

impl Producer for ByteProducer {
    type Good = u8;
    type Error = ClosedMarketError;

    #[inline]
    #[throws(ProduceFailure<Self::Error>)]
    fn produce(&self, good: Self::Good) {
        self.producer.produce(good)?
    }
}
