//! Implements a [`Consumer`] that implements read functionality and a [`Producer`] that implements write functionality.
use {
    crate::{
        channel::{CrossbeamConsumer, CrossbeamProducer},
        ComposeFrom, ComposingConsumer, Consumer, Producer, StripFrom, StrippingProducer,
    },
    core::sync::atomic::{AtomicBool, Ordering},
    crossbeam_channel::TryRecvError,
    log::error,
    std::{
        io::{self, ErrorKind, Read, Write},
        sync::Arc,
        thread::{self, JoinHandle},
    },
};

/// Consumes goods of type `G` by reading bytes via an item implementing [`Read`].
#[derive(Debug)]
pub struct Reader<G> {
    /// The consumer.
    consumer: ComposingConsumer<ByteConsumer, G>,
}

impl<G> Reader<G> {
    /// Creates a new [`Reader`] that reads bytes via `reader`.
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
    type Error = <ComposingConsumer<ByteConsumer, G> as Consumer>::Error;

    #[inline]
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error> {
        self.consumer.consume()
    }
}

/// Produces goods of type `G` by writing bytes via an item implementing [`Write`].
#[derive(Debug)]
pub struct Writer<G> {
    /// The producer.
    producer: StrippingProducer<G, ByteProducer>,
}

impl<G> Writer<G> {
    /// Creates a new [`Writer`] that writes bytes via `writer`.
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
{
    type Good = G;
    type Error = <StrippingProducer<G, ByteProducer> as Producer>::Error;

    #[inline]
    fn produce(&self, good: Self::Good) -> Result<Option<Self::Good>, Self::Error> {
        self.producer.produce(good)
    }
}

/// Consumes bytes using an item that implements [`Read`].
///
/// Reading is done within a separate thread to ensure consume() is non-blocking.
#[derive(Debug)]
pub struct ByteConsumer {
    /// Consumes bytes from the reading thread.
    consumer: CrossbeamConsumer<u8>,
    /// Consumes errors from the reading thread.
    error_consumer: CrossbeamConsumer<io::Error>,
    /// The handle to join the thread that processes writes.
    join_handle: Option<JoinHandle<()>>,
    /// If the thread is quitting.
    is_quitting: Arc<AtomicBool>,
}

impl ByteConsumer {
    /// Creates a new [`ByteConsumer`].
    #[inline]
    fn new<R: Read + Send + 'static>(mut reader: R) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let (err_tx, err_rx) = crossbeam_channel::bounded(1);
        let is_quitting = Arc::new(AtomicBool::new(false));
        let quitting = Arc::clone(&is_quitting);

        let join_handle = thread::spawn(move || {
            let mut buffer = [0; 1024];

            while !quitting.load(Ordering::Relaxed) {
                match reader.read(&mut buffer) {
                    Ok(len) => {
                        let (bytes, _) = buffer.split_at(len);

                        for byte in bytes {
                            if tx.send(*byte).is_err() {
                                if let Err(send_error) = err_tx.send(io::Error::new(
                                    ErrorKind::Other,
                                    "failed to send read bytes",
                                )) {
                                    error!(
                                        "Unable to store `ByteConsumer` send error: {}",
                                        send_error.into_inner()
                                    );
                                }

                                break;
                            }
                        }
                    }
                    Err(error) => {
                        if let Err(send_error) = err_tx.send(error) {
                            error!(
                                "Unable to store `ByteConsumer` read error: {}",
                                send_error.into_inner()
                            );
                        }

                        break;
                    }
                }
            }
        });

        Self {
            consumer: rx.into(),
            error_consumer: err_rx.into(),
            join_handle: Some(join_handle),
            is_quitting,
        }
    }
}

impl Consumer for ByteConsumer {
    type Good = u8;
    type Error = io::Error;

    #[inline]
    fn consume(&self) -> Result<Option<Self::Good>, Self::Error> {
        // consumer.consume() errs when the thread ends due to either read() or send() error.
        self.consumer.consume().or_else(|_| {
            Err(if let Ok(Some(error)) = self.error_consumer.consume() {
                error
            } else {
                io::Error::new(ErrorKind::Other, "failed to retrieve error")
            })
        })
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
pub struct ByteProducer {
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
    pub fn new<W>(mut writer: W) -> Self
    where
        W: Write + Send + 'static,
    {
        let (tx, rx) = crossbeam_channel::unbounded();
        let (err_tx, err_rx) = crossbeam_channel::bounded(1);
        let is_dropping = Arc::new(AtomicBool::new(false));
        let is_quitting = Arc::clone(&is_dropping);

        let join_handle = thread::spawn(move || {
            let mut buffer = [0; 1024];
            let mut len = 0;

            while !is_quitting.load(Ordering::Relaxed) {
                for element in buffer.iter_mut() {
                    match rx.try_recv() {
                        Ok(byte) => {
                            *element = byte;
                            #[allow(clippy::integer_arithmetic)]
                            {
                                // Overflow cannot occur since len = 0 at start of for loop and loop only iterates buffer.len() times.
                                len += 1;
                            }
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

                if len != 0 {
                    let (bytes, _) = buffer.split_at(len);

                    if let Err(error) = writer.write_all(bytes) {
                        if let Err(send_error) = err_tx.send(error) {
                            error!(
                                "Unable to store `ByteProducer` write error: {}",
                                send_error.into_inner()
                            );
                        }

                        is_quitting.store(true, Ordering::Relaxed);
                    }

                    len = 0;
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
    type Error = io::Error;

    #[inline]
    fn produce(&self, good: Self::Good) -> Result<Option<Self::Good>, Self::Error> {
        if let Ok(Some(error)) = self.error_consumer.consume() {
            Err(error)
        } else {
            self.producer
                .produce(good)
                .map_err(|_| io::Error::new(ErrorKind::Other, "failed to send bytes"))
        }
    }
}
