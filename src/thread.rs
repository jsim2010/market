//! Implements `Consumer` for thread functionality.
use {
    crate::{
        channel::{CrossbeamConsumer, CrossbeamProducer},
        ClassicalConsumerFailure, Consumer, Producer,
    },
    core::fmt::Debug,
    fehler::{throw, throws},
    log::error,
    parse_display::Display as ParseDisplay,
    std::{
        any::Any,
        error::Error,
        panic::{self, UnwindSafe},
        thread::{self, JoinHandle},
    },
    thiserror::Error as ThisError,
};

/// The type returned by a panic.
type Panic = Box<dyn Any + Send + 'static>;

/// An error while consuming the outcome of a thread.
#[derive(Debug, ThisError)]
pub enum Fault<E>
where
    E: Debug + Error + 'static,
{
    /// The thread was dropped.
    #[error("thread was dropped before output could be consumed")]
    Dropped,
    /// The thread threw an error.
    #[error(transparent)]
    Error(E),
}

impl<E> core::convert::TryFrom<ClassicalConsumerFailure<Fault<E>>> for Fault<E>
where
    E: Debug + Error + 'static,
{
    type Error = ();

    #[throws(<Self as core::convert::TryFrom<ClassicalConsumerFailure<Self>>>::Error)]
    fn try_from(failure: ClassicalConsumerFailure<Fault<E>>) -> Self {
        if let ClassicalConsumerFailure::Fault(fault) = failure {
            fault
        } else {
            throw!(())
        }
    }
}

/// A wrapper around the `std::thread` functionality.
///
/// Consumption replaces the functionality of `join`.
#[derive(Debug)]
pub struct Thread<T, E>
where
    E: Debug,
    T: Debug,
{
    /// Consumes the outcome of the thread.
    consumer: CrossbeamConsumer<Outcome<T, E>>,
    /// The handle to the thread.
    handle: JoinHandle<()>,
}

impl<T, E> Thread<T, E>
where
    E: Clone + Debug + Send + 'static,
    T: Clone + Debug + Send + 'static,
{
    /// Creates a new `Thread` and spawns `call`.
    #[inline]
    pub fn new<F>(call: F) -> Self
    where
        F: FnOnce() -> Result<T, E> + Send + UnwindSafe + 'static,
    {
        let (tx, rx) = crossbeam_channel::bounded(1);

        Self {
            handle: thread::spawn(move || {
                // Although force is preferable to produce, force requires the good impl Clone and the panic value is not bound to impl Clone. Using produce should be fine because produce should never be blocked since this market has a single producer storing a single good.
                if let Err(fault) = CrossbeamProducer::from(tx)
                    .produce(Outcome::from(panic::catch_unwind(|| (call)())))
                {
                    error!(
                        "Failed to send outcome of `{}` thread: {}",
                        thread::current().name().unwrap_or("{unnamed}"),
                        fault
                    );
                }
            }),
            consumer: rx.into(),
        }
    }
}

impl<O, E> Consumer for Thread<O, E>
where
    E: core::convert::TryFrom<ClassicalConsumerFailure<E>> + Error + 'static,
    O: Debug,
{
    type Good = O;
    type Failure = ClassicalConsumerFailure<Fault<E>>;

    #[throws(Self::Failure)]
    #[inline]
    fn consume(&self) -> Self::Good {
        match self.consumer.consume() {
            Ok(output) => match output {
                Outcome::Success(success) => success,
                Outcome::Error(error) => throw!(ClassicalConsumerFailure::Fault(Fault::Error(error))),
                #[allow(clippy::panic)]
                // Propogating the panic that occurred in call provided by third-party.
                Outcome::Panic(panic) => panic!(panic),
            },
            Err(failure) => match failure {
                ClassicalConsumerFailure::EmptyStock => throw!(ClassicalConsumerFailure::EmptyStock),
                ClassicalConsumerFailure::Fault(_) => throw!(Fault::Dropped),
            },
        }
    }
}

/// A `Result` with the additional possibility of a caught panic.
#[derive(Debug, ParseDisplay)]
enum Outcome<T, E> {
    /// The thread call completed sucessfully.
    #[display("{0}")]
    Success(T),
    /// The thread call threw an error.
    #[display("ERROR: {0}")]
    Error(E),
    /// The thread call panicked.
    #[display("PANIC")]
    Panic(Panic),
}

impl<T, E> From<Result<Result<T, E>, Panic>> for Outcome<T, E> {
    #[inline]
    fn from(result: Result<Result<T, E>, Panic>) -> Self {
        match result {
            Ok(Ok(success)) => Self::Success(success),
            Ok(Err(error)) => Self::Error(error),
            Err(panic) => Self::Panic(panic),
        }
    }
}
