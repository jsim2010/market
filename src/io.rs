//! Implements [`Producer`] and [`Consumer`] for [`Write`] and [`Read`] trait objects.
mod error;

pub use error::{ReadFault, WriteFault};

use {
    crate::{
        convert::{self, Assembler, Disassembler},
        thread::{Kind, Thread},
        ConsumeFailure, Consumer, Failure, Producer,
    },
    conventus::{AssembleFrom, DisassembleInto},
    core::{convert::TryFrom, fmt::Debug},
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
pub struct Reader<G: AssembleFrom<u8>> {
    /// Assembles goods of type `G` from [`u8`]s.
    assembler: Assembler<u8, G>,
    /// The thread which executes the reads.
    thread: Thread<(), std::io::Error>,
}

impl<G: AssembleFrom<u8>> Reader<G> {
    /// Creates a new [`Reader`] with `reader`.
    #[inline]
    pub fn new<R>(mut reader: R) -> Self
    where
        R: Read + RefUnwindSafe + Send + 'static,
    {
        let (parts_input, assembler) = convert::create_assembly_line();
        let buf = [0; 1024];

        Self {
            assembler,
            thread: Thread::new(Kind::Cancelable, buf, move |buf| {
                let len = reader.read(buf)?;
                let (bytes, _) = buf.split_at(len);

                #[allow(clippy::unwrap_used)]
                // PartsInput::force_all() returns Result<_, Infallible>.
                parts_input.force_all(bytes.to_vec()).unwrap();
                Ok(())
            }),
        }
    }

    /// Requests that the thread be canceled.
    #[inline]
    pub fn cancel(&self) {
        self.thread.cancel();
    }
}

impl<G: AssembleFrom<u8>> Consumer for Reader<G>
where
    <G as AssembleFrom<u8>>::Error: TryFrom<ConsumeFailure<<G as AssembleFrom<u8>>::Error>>,
{
    type Good = G;
    type Failure = ConsumeFailure<ReadFault<G>>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        self.assembler.consume().map_err(|failure| match failure {
            ConsumeFailure::EmptyStock => match self.thread.consume() {
                Ok(()) => ReadFault::Terminated.into(),
                Err(ConsumeFailure::EmptyStock) => ConsumeFailure::EmptyStock,
                Err(ConsumeFailure::Fault(fault)) => ConsumeFailure::Fault(fault.into()),
            },
            ConsumeFailure::Fault(fault) => ConsumeFailure::Fault(ReadFault::Assemble(fault)),
        })?
    }
}

/// Writes bytes disassembled from goods of type `G` via a [`Write`] trait object.
///
/// Because [`Write::write()`] does not provide any guarantees about blocking, the write is executed in a separate thread. The current thread attempts to disassemble the good into bytes that are produced to the thread.
#[derive(Debug)]
pub struct Writer<G: DisassembleInto<u8>> {
    /// Disassembles goods of type `G` into [`u8`].
    disassembler: Disassembler<u8, G>,
    /// The thread which executes the writes.
    thread: Thread<(), std::io::Error>,
}

impl<G: DisassembleInto<u8>> Writer<G> {
    /// Creates a new [`Writer`] with `writer`.
    #[inline]
    pub fn new<W>(mut writer: W) -> Self
    where
        W: Write + RefUnwindSafe + Send + 'static,
    {
        let (disassembler, parts_output) = convert::create_disassembly_line();

        Self {
            disassembler,
            thread: Thread::new(Kind::Cancelable, (), move |_| {
                #[allow(clippy::unwrap_used)]
                // Consumer::goods() returns Result<_, Infallible>.
                writer.write_all(
                    &parts_output
                        .goods()
                        .collect::<Result<Vec<u8>, _>>()
                        .unwrap(),
                )?;
                Ok(())
            }),
        }
    }

    /// Requests that the thread be canceled.
    #[inline]
    pub fn cancel(&self) {
        self.thread.cancel();
    }
}

impl<G: DisassembleInto<u8>> Producer for Writer<G>
where
    <G as DisassembleInto<u8>>::Error: Failure,
{
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
                    self.disassembler
                        .produce(good)
                        .map_err(WriteFault::Disassemble)?
                }
            }
        }
    }
}
