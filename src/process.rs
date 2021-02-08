//! Implements [`Producer`] and [`Consumer`] for the standard I/O streams of a process.
#[cfg(doc)]
use crate::Producer;

use {
    crate::{
        io::{Reader, Writer},
        ConsumeFailure, ConsumeFault, Consumer,
    },
    conventus::{AssembleFrom, DisassembleInto},
    core::{cell::RefCell, fmt::Debug},
    fehler::throws,
    std::process::{Child, Command, ExitStatus, Stdio},
};

/// Represents a process with piped stdio's.
///
/// stdin is written to by producing to the process.
/// stdout is read by consuming from the process.
/// stderr is read by consuming from `Process::stderr()`.
#[derive(Debug)]
pub struct Process<I: DisassembleInto<u8>, O: AssembleFrom<u8>, E: AssembleFrom<u8>> {
    // Used for providing information to errors.
    /// A printable representation of the command executed by the process.
    command_str: String,
    // Use RefCell due to try_wait() requiring Child to be mut.
    /// The process.
    child: RefCell<Child>,
    /// The stdin of the process.
    input: Writer<I>,
    /// The stdout of the process.
    output: Reader<O>,
    /// The stderr of the process.
    error: Reader<E>,
}

impl<I: DisassembleInto<u8>, O: AssembleFrom<u8>, E: AssembleFrom<u8>> Process<I, O, E> {
    /// Creates a new `Process` that exectues `command`.
    #[allow(clippy::unwrap_in_result)] // Guaranteed that Results are Ok.
    #[inline]
    #[throws(CreateProcessError)]
    pub fn new(mut command: Command) -> Self {
        let command_str = format!("{:?}", command);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| CreateProcessError {
                command: command_str.clone(),
                error,
            })?;

        #[allow(clippy::unwrap_used)] // Guaranteed that these values exist and have not been taken.
        Self {
            input: Writer::new(format!("{} IN", command_str), child.stdin.take().unwrap()),
            output: Reader::new(format!("{} OUT", command_str), child.stdout.take().unwrap()),
            error: Reader::new(format!("{} ERR", command_str), child.stderr.take().unwrap()),
            child: RefCell::new(child),
            command_str,
        }
    }

    /// Returns the [`Writer`] of the stdin pipe.
    #[inline]
    pub fn input(&self) -> &Writer<I> {
        &self.input
    }

    /// Returns the [`Reader`] of the stdout pipe.
    #[inline]
    pub fn output(&self) -> &Reader<O> {
        &self.output
    }

    /// Returns the [`Reader`} of the stderr pipe.
    #[inline]
    pub fn error(&self) -> &Reader<E> {
        &self.error
    }
}

impl<I: DisassembleInto<u8>, O: AssembleFrom<u8>, E: AssembleFrom<u8>> Consumer
    for Process<I, O, E>
{
    type Good = ExitStatus;
    type Failure = ConsumeFailure<WaitFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        let status = self
            .child
            .borrow_mut()
            .try_wait()
            .map_err(|error| WaitFault {
                command: self.command_str.clone(),
                error,
            })?
            .ok_or(ConsumeFailure::EmptyStock)?;

        // Child has exited; now need to cancel the process IO threads.
        self.input.cancel();
        self.output.cancel();
        self.error.cancel();

        status
    }
}

/// An error creating a `Process`.
#[derive(Debug, thiserror::Error)]
#[error("Failed to create `{command}`: {error}")]
pub struct CreateProcessError {
    /// The command attempting to be created.
    command: String,
    /// The error.
    error: std::io::Error,
}

/// An error waiting for a `Process` to exit.
#[derive(Debug, ConsumeFault, thiserror::Error)]
#[error("Failed to wait for `{command}`: {error}")]
pub struct WaitFault {
    /// The command of the process.
    command: String,
    /// The error.
    error: std::io::Error,
}
