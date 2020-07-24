//! Implements `Consumer` and `Producer` for the stdio's of a process.
use {
    crate::{
        io::{Reader, Writer},
        ComposeFrom, ConsumeError, Consumer, ProduceError, Producer, StripFrom,
    },
    core::fmt::{Debug, Display},
    fehler::throws,
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
    // Used for providing information to errors.
    /// A printable representation of the command executed by the process.
    command: String,
    /// The handle of the process.
    handle: Child,
    /// The stdin of the process.
    input: Writer<I>,
    /// The stdout of the process.
    output: Reader<O>,
    /// The stderr of the process.
    error: Reader<E>,
}

impl<I, O, E> Process<I, O, E> {
    /// Creates a new `Process` that exectues `command`.
    #[inline]
    #[throws(CreateProcessError)]
    pub fn new(mut command: Command) -> Self {
        let command_string = format!("{:?}", command);
        let mut handle = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| CreateProcessError::new(&command_string, error))?;

        Self {
            input: Writer::new(handle.stdin.take().ok_or_else(|| {
                CreateProcessError::new(&command_string, UncapturedStdioError("stdin".to_string()))
            })?),
            output: Reader::new(handle.stdout.take().ok_or_else(|| {
                CreateProcessError::new(&command_string, UncapturedStdioError("stdout".to_string()))
            })?),
            error: Reader::new(handle.stderr.take().ok_or_else(|| {
                CreateProcessError::new(&command_string, UncapturedStdioError("stderr".to_string()))
            })?),
            command: command_string,
            handle,
        }
    }

    /// Returns the `Consumer` of the stderr pipe.
    #[inline]
    pub const fn stderr(&self) -> &Reader<E> {
        &self.error
    }

    /// Waits for the process to exit.
    #[inline]
    #[throws(WaitProcessError)]
    pub fn wait(&mut self) -> ExitStatus {
        self.handle.wait().map_err(|error| WaitProcessError {
            command: self.command.clone(),
            error,
        })?
    }
}

impl<I, O, E> Consumer for Process<I, O, E>
where
    O: ComposeFrom<u8> + Display,
{
    type Good = O;
    type Failure = <Reader<O> as Consumer>::Failure;

    #[inline]
    #[throws(ConsumeError<Self::Failure>)]
    fn consume(&self) -> Self::Good {
        self.output.consume()?
    }
}

impl<I, O, E> Producer for Process<I, O, E>
where
    u8: StripFrom<I>,
    I: Debug + Display,
{
    type Good = I;
    type Failure = <Writer<I> as Producer>::Failure;

    #[inline]
    #[throws(ProduceError<Self::Failure>)]
    fn produce(&self, good: Self::Good) {
        self.input.produce(good)?
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
