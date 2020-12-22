//! Errors related to IO.
use {
    crate::{ConsumeFailure, ConsumeFault, Failure, ProduceFault},
    conventus::{AssembleFrom, DisassembleInto},
    core::{
        convert::TryFrom,
        fmt::{self, Debug, Display, Formatter},
    },
    fehler::{throw, throws},
    std::error::Error,
};

// Cannot derive thiserror::Error as this would require G: Display.
/// A fault while reading a good of type `G`.
#[derive(ConsumeFault)]
pub enum ReadFault<G: AssembleFrom<u8>> {
    /// The read threw an error.
    Io(std::io::Error),
    /// The thread was terminated.
    Terminated,
    /// The assembly of the good from bytes threw an error.
    Assemble(<G as AssembleFrom<u8>>::Error),
}

impl<G: AssembleFrom<u8>> Debug for ReadFault<G>
where
    <G as AssembleFrom<u8>>::Error: Debug,
{
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "ReadFault::")?;

        match *self {
            Self::Io(ref error) => write!(f, "Io({:?})", error),
            Self::Terminated => write!(f, "Terminated"),
            Self::Assemble(ref error) => write!(f, "Assemble({:?})", error),
        }
    }
}

impl<G: AssembleFrom<u8>> Display for ReadFault<G>
where
    <G as AssembleFrom<u8>>::Error: Display,
{
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::Io(ref fault) => write!(f, "{}", fault),
            Self::Terminated => write!(f, "Thread was terminated"),
            Self::Assemble(ref error) => write!(f, "{}", error),
        }
    }
}

impl<G: AssembleFrom<u8>> Error for ReadFault<G> where <G as AssembleFrom<u8>>::Error: Error {}

impl<G: AssembleFrom<u8>> From<std::io::Error> for ReadFault<G> {
    #[inline]
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

// Cannot derive thiserror::Error as this would require G: Display.
/// A fault while writing a good of type `G`.
#[derive(ProduceFault)]
pub enum WriteFault<G: DisassembleInto<u8>> {
    /// The write threw an error.
    Io(std::io::Error),
    /// The thread was terminated.
    Terminated,
    /// The disassembly of the good into bytes threw an error.
    Disassemble(<G as DisassembleInto<u8>>::Error),
}

impl<G: DisassembleInto<u8>> Debug for WriteFault<G>
where
    <G as DisassembleInto<u8>>::Error: Debug,
{
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "WriteFault::")?;

        match *self {
            Self::Io(ref error) => write!(f, "Io({:?}", error),
            Self::Terminated => write!(f, "Terminated"),
            Self::Disassemble(ref error) => write!(f, "Disassemble({:?})", error),
        }
    }
}

impl<G: DisassembleInto<u8>> Display for WriteFault<G>
where
    <G as DisassembleInto<u8>>::Error: Display,
{
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::Io(ref fault) => write!(f, "{}", fault),
            Self::Terminated => write!(f, "Thread was terminated"),
            Self::Disassemble(ref error) => write!(f, "{}", error),
        }
    }
}

impl<G: DisassembleInto<u8>> Error for WriteFault<G> where <G as DisassembleInto<u8>>::Error: Error {}

impl<G: DisassembleInto<u8>> Failure for WriteFault<G> {
    type Fault = Self;
}

// Required by bounds from Thread<_, std::io::Error>.
impl TryFrom<ConsumeFailure<std::io::Error>> for std::io::Error {
    type Error = ();

    #[inline]
    #[throws(())]
    fn try_from(failure: ConsumeFailure<Self>) -> Self {
        if let ConsumeFailure::Fault(fault) = failure {
            fault
        } else {
            throw!(())
        }
    }
}
