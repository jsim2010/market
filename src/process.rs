//! Implements `Consumer` and `Producer` for the stdio's of a process.
use {
    crate::{
        io::{Reader, Writer},
        ConsumeFailure, Consumer, ProduceFailure, Producer,
    },
    conventus::{AssembleFrom, DisassembleInto},
    core::{cell::RefCell, fmt::Debug},
    fehler::{throw, throws},
    std::{
        io,
        process::{Child, Command, ExitStatus, Stdio},
    },
    thiserror::Error as ThisError,
};

/// Represents a process with piped stdio's.
///
/// stdin is written to by producing to the process.
/// stdout is read by consuming from the process.
/// stderr is read by consuming from `Process::stderr()`.
#[derive(Debug)]
pub struct Process<I, O, E> {
    /// The stdin of the process.
    input: Writer<I>,
    /// The stdout of the process.
    output: Reader<O>,
    /// The stderr of the process.
    error: Reader<E>,
    /// The `Waiter` of the process.
    waiter: Waiter,
}

impl<I, O, E> Process<I, O, E> {
    /// Creates a new `Process` that exectues `command`.
    #[inline]
    #[throws(CreateProcessError)]
    pub fn new(mut command: Command) -> Self {
        let command_string = format!("{:?}", command);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| CreateProcessError::new(&command_string, error))?;

        Self {
            input: Writer::new(child.stdin.take().ok_or_else(|| {
                CreateProcessError::new(&command_string, UncapturedStdioError("stdin".to_string()))
            })?),
            output: Reader::new(child.stdout.take().ok_or_else(|| {
                CreateProcessError::new(&command_string, UncapturedStdioError("stdout".to_string()))
            })?),
            error: Reader::new(child.stderr.take().ok_or_else(|| {
                CreateProcessError::new(&command_string, UncapturedStdioError("stderr".to_string()))
            })?),
            waiter: Waiter {
                child: RefCell::new(child),
                command: command_string,
            },
        }
    }

    /// Returns the `Consumer` of the stderr pipe.
    #[inline]
    pub const fn stderr(&self) -> &Reader<E> {
        &self.error
    }

    /// Returns the `Consumer` of the `ExitStatus` of the process.
    #[inline]
    pub const fn waiter(&self) -> &Waiter {
        &self.waiter
    }
}

impl<I, O, E> Consumer for Process<I, O, E>
where
    O: AssembleFrom<u8> + Debug + 'static,
    <O as AssembleFrom<u8>>::Error: 'static,
{
    type Good = O;
    type Fault = <Reader<O> as Consumer>::Fault;

    #[inline]
    #[throws(ConsumeFailure<Self::Fault>)]
    fn consume(&self) -> Self::Good {
        self.output.consume()?
    }
}

impl<I, O, E> Producer for Process<I, O, E>
where
    I: DisassembleInto<u8> + Debug,
    <I as DisassembleInto<u8>>::Error: 'static,
{
    type Good = I;
    type Fault = <Writer<I> as Producer>::Fault;

    #[inline]
    #[throws(ProduceFailure<Self::Fault>)]
    fn produce(&self, good: Self::Good) {
        self.input.produce(good)?
    }
}

/// Consumes the `ExitStatus` of a process.
#[derive(Debug)]
pub struct Waiter {
    // Used for providing information to errors.
    /// A printable representation of the command executed by the process.
    command: String,
    // Use RefCell due to try_wait() requiring Child to be mut.
    /// The process.
    child: RefCell<Child>,
}

impl Consumer for Waiter {
    type Good = ExitStatus;
    type Fault = WaitProcessError;

    #[inline]
    #[throws(ConsumeFailure<Self::Fault>)]
    fn consume(&self) -> Self::Good {
        if let Some(status) =
            self.child
                .borrow_mut()
                .try_wait()
                .map_err(|error| WaitProcessError {
                    command: self.command.clone(),
                    error,
                })?
        {
            status
        } else {
            throw!(ConsumeFailure::EmptyStock);
        }
    }
}

/// An error creating a `Process`.
#[derive(Debug, ThisError)]
#[error("Failed to create `{command}`: {error}")]
pub struct CreateProcessError {
    /// The command attempting to be created.
    command: String,
    /// The error.
    error: CreateProcessErrorType,
}

impl CreateProcessError {
    /// Creates a new `CreateProcessError`.
    fn new<T>(command: &str, error: T) -> Self
    where
        T: Into<CreateProcessErrorType>,
    {
        Self {
            command: command.to_string(),
            error: error.into(),
        }
    }
}

/// An type of error creating a `Process`.
#[derive(Debug, ThisError)]
pub enum CreateProcessErrorType {
    /// I/O error.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Stdio is not captured.
    #[error(transparent)]
    UncapturedStdio(#[from] UncapturedStdioError),
}

/// An error capturing a stdio.
#[derive(Debug, ThisError)]
#[error("`{0}` is not captured")]
pub struct UncapturedStdioError(String);

/// An error waiting for a `Process` to exit.
#[derive(Debug, ThisError)]
#[error("Failed to wait for `{command}`: {error}")]
pub struct WaitProcessError {
    /// The command of the process.
    command: String,
    /// The error.
    error: io::Error,
}
