//! Implements [`Producer`] and [`Consumer`] for the standard I/O streams of a process.
use {
    crate::{ConsumeFailure, Consumer, ProduceFailure, Producer},
    core::{cell::RefCell, convert::TryFrom, fmt::Debug},
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
        let input = Rc::new(crate::io::Writer::new(child.stdin.take().ok_or_else(
            || CreateProcessError::new(&command_string, UncapturedStdioError("stdin".to_string())),
        )?));
        let output = Rc::new(crate::io::Reader::new(child.stdout.take().ok_or_else(
            || CreateProcessError::new(&command_string, UncapturedStdioError("stdout".to_string())),
        )?));
        let error = Rc::new(crate::io::Reader::new(child.stderr.take().ok_or_else(
            || CreateProcessError::new(&command_string, UncapturedStdioError("stderr".to_string())),
        )?));

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

impl<I, O, E> Consumer for Process<I, O, E>
where
    O: conventus::AssembleFrom<u8> + Debug + 'static,
    <O as conventus::AssembleFrom<u8>>::Error: 'static,
{
    type Good = O;
    type Failure = ConsumeFailure<crate::io::ReadFault<O>>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        self.output.consume()?
    }
}

impl<I, O, E> Producer for Process<I, O, E>
where
    I: conventus::DisassembleInto<u8> + Debug,
    <I as conventus::DisassembleInto<u8>>::Error: 'static,
{
    type Good = I;
    type Failure = ProduceFailure<crate::io::WriteError<I>>;

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
    /// The input writer.
    input: Rc<crate::io::Writer<I>>,
    /// The output reader.
    output: Rc<crate::io::Reader<O>>,
    /// The error output reader.
    error: Rc<crate::io::Reader<E>>,
}

impl<I, O, E> Consumer for Waiter<I, O, E> {
    type Good = ExitStatus;
    type Failure = ConsumeFailure<WaitFault>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        if let Some(status) = self
            .child
            .borrow_mut()
            .try_wait()
            .map_err(|error| WaitFault {
                command: self.command.clone(),
                error: error.into(),
            })?
        {
            // Calling wait() is recommended to ensure resources are released. Since try_wait() was successful, wait() should not block.
            #[allow(unused_results)] // Status was already retrieved by try_wait().
            {
                self.child.borrow_mut().wait().map_err(|error| WaitFault {
                    command: self.command.clone(),
                    error: error.into(),
                })?;
            }
            // Terminate the Writer and Reader threads.
            self.input.terminate().map_err(|error| WaitFault {
                command: self.command.clone(),
                error: error.into(),
            })?;
            self.output.terminate().map_err(|error| WaitFault {
                command: self.command.clone(),
                error: error.into(),
            })?;
            self.error.terminate().map_err(|error| WaitFault {
                command: self.command.clone(),
                error: error.into(),
            })?;
            status
        } else {
            throw!(ConsumeFailure::EmptyStock);
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

/// Error thrown while waiting on process.
#[derive(Debug, thiserror::Error)]
pub enum WaitError {
    /// Error thrown by wait call.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Error thrown while terminating a write thread.
    #[error(transparent)]
    Write(#[from] crate::thread::Fault<crate::io::WriteThreadError>),
    /// Error thrown while terminating a read thread.
    #[error(transparent)]
    Read(#[from] crate::thread::Fault<crate::io::ReadThreadError>),
}

/// An error waiting for a `Process` to exit.
#[derive(Debug, thiserror::Error)]
#[error("Failed to wait for `{command}`: {error}")]
pub struct WaitFault {
    /// The command of the process.
    command: String,
    /// The error.
    error: WaitError,
}

impl TryFrom<ConsumeFailure<WaitFault>> for WaitFault {
    type Error = ();

    #[inline]
    #[throws(Self::Error)]
    fn try_from(failure: ConsumeFailure<Self>) -> Self {
        if let ConsumeFailure::Fault(fault) = failure {
            fault
        } else {
            throw!(())
        }
    }
}

impl TryFrom<ProduceFailure<WaitFault>> for WaitFault {
    type Error = ();

    #[inline]
    #[throws(Self::Error)]
    fn try_from(failure: ProduceFailure<Self>) -> Self {
        if let ProduceFailure::Fault(fault) = failure {
            fault
        } else {
            throw!(())
        }
    }
}
