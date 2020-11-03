//! Implements [`Producer`] and [`Consumer`] for the standard I/O streams of a process.
use {
    core::{convert::TryFrom, cell::RefCell, fmt::Debug},
    fehler::{throw, throws},
    std::{
        process::{Child, Command, ExitStatus, Stdio},
        rc::Rc,
    },
};

/// Represents a process with piped stdio's.
///
/// stdin is written to by producing to the process.
/// stdout is read by consuming from the process.
/// stderr is read by consuming from `Process::stderr()`.
#[derive(Debug)]
pub struct Process<I, O, E> {
    /// The stdin of the process.
    input: Rc<crate::io::Writer<I>>,
    /// The stdout of the process.
    output: Rc<crate::io::Reader<O>>,
    /// The stderr of the process.
    error: Rc<crate::io::Reader<E>>,
    /// The `Waiter` of the process.
    waiter: Waiter<I, O, E>,
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
        let input = Rc::new(crate::io::Writer::new(child.stdin.take().ok_or_else(|| {
            CreateProcessError::new(
                &command_string,
                UncapturedStdioError("stdin".to_string()),
            )
        })?));
        let output = Rc::new(crate::io::Reader::new(child.stdout.take().ok_or_else(|| {
            CreateProcessError::new(
                &command_string,
                UncapturedStdioError("stdout".to_string()),
            )
        })?));
        let error = Rc::new(crate::io::Reader::new(child.stderr.take().ok_or_else(|| {
            CreateProcessError::new(
                &command_string,
                UncapturedStdioError("stderr".to_string()),
            )
        })?));

        Self {
            input: Rc::clone(&input),
            output: Rc::clone(&output),
            error: Rc::clone(&error),
            waiter: Waiter {
                child: RefCell::new(child),
                command: command_string,
                input,
                output,
                error,
            },
        }
    }

    /// Returns the `Consumer` of the stderr pipe.
    #[inline]
    pub const fn stderr(&self) -> &Rc<crate::io::Reader<E>> {
        &self.error
    }

    /// Returns the `Consumer` of the `ExitStatus` of the process.
    #[inline]
    pub const fn waiter(&self) -> &Waiter<I, O, E> {
        &self.waiter
    }
}

impl<I, O, E> crate::Consumer for Process<I, O, E>
where
    O: conventus::AssembleFrom<u8> + Debug + 'static,
    <O as conventus::AssembleFrom<u8>>::Error: 'static,
{
    type Good = O;
    type Failure = crate::ConsumerFailure<crate::io::ReadFault<O>>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        self.output.consume()?
    }
}

impl<I, O, E> crate::Producer for Process<I, O, E>
where
    I: conventus::DisassembleInto<u8> + Debug,
    <I as conventus::DisassembleInto<u8>>::Error: 'static,
{
    type Good = I;
    type Failure = crate::error::ProducerFailure<crate::io::WriteError<I>>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        self.input.produce(good)?
    }
}

/// Consumes the `ExitStatus` of a process.
#[derive(Debug)]
pub struct Waiter<I, O, E> {
    // Used for providing information to errors.
    /// A printable representation of the command executed by the process.
    command: String,
    // Use RefCell due to try_wait() requiring Child to be mut.
    /// The process.
    child: RefCell<Child>,
    input: Rc<crate::io::Writer<I>>,
    output: Rc<crate::io::Reader<O>>,
    error: Rc<crate::io::Reader<E>>,
}

impl<I, O, E> crate::Consumer for Waiter<I, O, E> {
    type Good = ExitStatus;
    type Failure = crate::ConsumerFailure<WaitFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        if let Some(status) =
            self.child
                .borrow_mut()
                .try_wait()
                .map_err(|error| WaitFault {
                    command: self.command.clone(),
                    error,
                })?
        {
            // Calling wait() is recommended to ensure resources are released. Since try_wait() was successful, wait() should not block.
            self.child.borrow_mut().wait().expect("waiting on child process");
            // Terminate the Writer and Reader threads.
            self.input.terminate().expect("terminating child process input");
            self.output.terminate().expect("terminating child process output");
            self.error.terminate().expect("terminating child process error output");
            status
        } else {
            throw!(crate::ConsumerFailure::EmptyStock);
        }
    }
}

/// An error creating a `Process`.
#[derive(Debug, thiserror::Error)]
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
#[derive(Debug, thiserror::Error)]
pub enum CreateProcessErrorType {
    /// I/O error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Stdio is not captured.
    #[error(transparent)]
    UncapturedStdio(#[from] UncapturedStdioError),
}

/// An error capturing a stdio.
#[derive(Debug, thiserror::Error)]
#[error("`{0}` is not captured")]
pub struct UncapturedStdioError(String);

/// An error waiting for a `Process` to exit.
#[derive(Debug, thiserror::Error)]
#[error("Failed to wait for `{command}`: {error}")]
pub struct WaitFault {
    /// The command of the process.
    command: String,
    /// The error.
    error: std::io::Error,
}

impl TryFrom<crate::ConsumerFailure<WaitFault>> for WaitFault {
    type Error = ();

    #[inline]
    #[throws(Self::Error)]
    fn try_from(failure: crate::ConsumerFailure<Self>) -> Self {
        if let crate::ConsumerFailure::Fault(fault) = failure {
            fault
        } else {
            throw!(())
        }
    }
}

impl TryFrom<crate::error::ProducerFailure<WaitFault>> for WaitFault {
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
