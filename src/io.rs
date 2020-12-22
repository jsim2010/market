//! Implements [`Producer`] and [`Consumer`] for [`Write`] and [`Read`] trait objects.
mod error;

pub use error::{ReadFault, WriteFault};

use {
    crate::{
        queue::{create_supply_chain, Procurer, Supplier},
        thread::{Kind, Thread},
        ConsumeFailure, Consumer, Producer,
    },
    conventus::{AssembleFailure, AssembleFrom, DisassembleInto},
    core::{cell::RefCell, fmt::Debug, marker::PhantomData},
    fehler::{throw, throws},
    std::{
        io::{Read, Write},
        panic::RefUnwindSafe,
    },
};

/// Consumes goods of type `G` assembled from bytes read by a [`Read`] trait object.
///
/// Because [`Read::read()`] does not provide any guarantees about blocking, the read is executed in a separate thread which produces the read bytes. The current thread attempts to assemble the consumed bytes into a good.
#[derive(Debug)]
pub struct Reader<G> {
    /// Consumes bytes from the thread.
    byte_consumer: Procurer<u8>,
    /// The thread which executes the reads.
    thread: Thread<(), std::io::Error>,
    /// The current buffer of bytes.
    buffer: RefCell<Vec<u8>>,
    #[doc(hidden)]
    phantom: PhantomData<G>,
}

impl<G> Reader<G> {
    /// Creates a new [`Reader`] with `reader`.
    #[inline]
    pub fn new<R>(mut reader: R) -> Self
    where
        R: Read + RefUnwindSafe + Send + 'static,
    {
        let (byte_producer, byte_consumer) = create_supply_chain();
        let buf = [0; 1024];

        Self {
            byte_consumer,
            thread: Thread::new(Kind::Cancelable, buf, move |buf| {
                let len = reader.read(buf)?;
                let (bytes, _) = buf.split_at(len);

                #[allow(clippy::unwrap_used)]
                // Supplier::force_all() returns Result<_, Infallible>.
                byte_producer.force_all(bytes.to_vec()).unwrap();
                Ok(())
            }),
            buffer: RefCell::new(Vec::new()),
            phantom: PhantomData,
        }
    }

    /// Requests that the thread be canceled.
    #[inline]
    pub fn cancel(&self) {
        self.thread.cancel();
    }
}

impl<G: AssembleFrom<u8>> Consumer for Reader<G> {
    type Good = G;
    type Failure = ConsumeFailure<ReadFault<G>>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        match self.byte_consumer.consume_all() {
            Ok(mut bytes) => {
                let mut buffer = self.buffer.borrow_mut();
                buffer.append(&mut bytes);
                G::assemble_from(&mut buffer).map_err(|error| match error {
                    AssembleFailure::Incomplete => ConsumeFailure::EmptyStock,
                    AssembleFailure::Error(e) => ConsumeFailure::Fault(ReadFault::Assemble(e)),
                })?
            }
            Err(_) => {
                // Procurer::Failure is FaultlessFailure so a failure thrown by consume_all() is caused by an empty stock. Check to see if the thread has terminated.
                match self.thread.consume() {
                    // Thread was terminated.
                    Ok(()) => throw!(ReadFault::Terminated),
                    Err(failure) => throw!(match failure {
                        ConsumeFailure::EmptyStock => ConsumeFailure::EmptyStock,
                        ConsumeFailure::Fault(fault) =>
                            ConsumeFailure::Fault(ReadFault::from(fault)),
                    }),
                }
            }
        }
    }
}

/// Writes bytes disassembled from goods of type `G` via a [`Write`] trait object.
///
/// Because [`Write::write()`] does not provide any guarantees about blocking, the write is executed in a separate thread. The current thread attempts to disassemble the good into bytes that are produced to the thread.
#[derive(Debug)]
pub struct Writer<G> {
    /// Produces bytes to the thread.
    byte_producer: Supplier<u8>,
    /// The thread which executes the writes.
    thread: Thread<(), std::io::Error>,
    #[doc(hidden)]
    phantom: PhantomData<G>,
}

impl<G> Writer<G> {
    /// Creates a new [`Writer`] with `writer`.
    #[inline]
    pub fn new<W>(mut writer: W) -> Self
    where
        W: Write + RefUnwindSafe + Send + 'static,
    {
        let (byte_producer, byte_consumer) = create_supply_chain();

        Self {
            byte_producer,
            thread: Thread::new(Kind::Cancelable, (), move |_| {
                #[allow(clippy::unwrap_used)]
                // Procurer::consume_all() returns Result<_, Infallible>.
                writer.write_all(&byte_consumer.consume_all().unwrap())?;
                Ok(())
            }),
            phantom: PhantomData,
        }
    }

    /// Requests that the thread be canceled.
    #[inline]
    pub fn cancel(&self) {
        self.thread.cancel();
    }
}

impl<G: DisassembleInto<u8>> Producer for Writer<G> {
    type Good = G;
    type Failure = WriteFault<G>;

    #[allow(clippy::unwrap_in_result)] // Supplier::produce_all returns Result<_, Infallible>.
    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        // Check to see if the thread was terminated.
        match self.thread.consume() {
            // Thread was terminated.
            Ok(()) => throw!(WriteFault::Terminated),
            Err(failure) => {
                if let ConsumeFailure::Fault(error) = failure {
                    throw!(WriteFault::Io(error));
                } else {
                    // Thread is still running.
                    #[allow(clippy::unwrap_used)]
                    // Supplier::produce_all returns Result<_, Infallible>.
                    self.byte_producer
                        .produce_all(good.disassemble_into().map_err(WriteFault::Disassemble)?)
                        .unwrap()
                }
            }
        }
    }
}
