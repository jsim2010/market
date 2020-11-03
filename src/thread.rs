//! Implements [`Producer`] and [`Consumer`] for a thread.
use {
    crate::{ConsumeFailure, Consumer, Producer},
    core::fmt::{Debug, Display},
    fehler::{throw, throws},
    log::error,
    std::{any::Any, error::Error, panic::UnwindSafe, thread::JoinHandle},
};

/// The type returned by [`std::panic::catch_unwind()`] when a panic is caught.
type Panic = Box<dyn Any + Send + 'static>;

/// An error while consuming the outcome of a thread.
#[derive(Debug, Eq, PartialEq)]
pub enum Fault<E> {
    /// The thread was killed.
    Killed,
    /// The thread threw an error.
    Error(E),
}

consumer_fault!(Fault<E>);

impl<E: Display> Display for Fault<E> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::Killed => write!(f, "thread was killed"),
            Self::Error(ref error) => write!(f, "thread output error: {}", error),
        }
    }
}

impl<E: Debug + Display> Error for Fault<E> {}

/// The type returned by a thread call which can represent a success of type `S`, an error of type `E`, or a panic.
#[derive(Debug, parse_display::Display)]
enum Outcome<S, E> {
    /// The thread call completed sucessfully.
    #[display("{0}")]
    Success(S),
    /// The thread call threw an error.
    #[display("ERROR: {0}")]
    Error(E),
    /// The thread call panicked.
    #[display("PANIC")]
    Panic(Panic),
}

impl<S, E> From<Result<Result<S, E>, Panic>> for Outcome<S, E> {
    #[inline]
    fn from(result: Result<Result<S, E>, Panic>) -> Self {
        match result {
            Ok(Ok(success)) => Self::Success(success),
            Ok(Err(error)) => Self::Error(error),
            Err(panic) => Self::Panic(panic),
        }
    }
}

/// A wrapper around the `std::thread` functionality.
///
/// Consumes the outcome of running the given closure. Thus, consumption replaces the functionality of `join`.
#[derive(Debug)]
pub struct Thread<S, E> {
    /// Consumes the outcome of the thread.
    consumer: crate::channel::CrossbeamConsumer<Outcome<S, E>>,
    /// The handle to the thread.
    handle: JoinHandle<()>,
}

impl<S, E> Thread<S, E>
where
    S: Send + 'static,
    E: Send + 'static,
{
    /// Creates a new `Thread` and spawns `call`.
    #[inline]
    pub fn new<F>(call: F) -> Self
    where
        F: FnOnce() -> Result<S, E> + Send + UnwindSafe + 'static,
    {
        let (tx, rx) = crossbeam_channel::bounded(1);

        Self {
            handle: std::thread::spawn(move || {
                // Although force is preferable to produce, force requires the outcome impl Clone and the panic value is not bound to impl Clone. Using produce should be fine because produce should never be blocked since this market has a single producer storing a single good.
                if let Err(failure) = crate::channel::CrossbeamProducer::from(tx)
                    .produce(Outcome::from(std::panic::catch_unwind(|| (call)())))
                {
                    error!(
                        "Failed to send outcome of `{}` thread: {}",
                        std::thread::current().name().unwrap_or("{unnamed}"),
                        failure
                    );
                }
            }),
            consumer: rx.into(),
        }
    }
}

impl<S, E> Consumer for Thread<S, E> {
    type Good = S;
    type Failure = ConsumeFailure<Fault<E>>;

    #[throws(Self::Failure)]
    #[inline]
    fn consume(&self) -> Self::Good {
        match self.consumer.consume() {
            Ok(output) => match output {
                Outcome::Success(success) => success,
                Outcome::Error(error) => throw!(Fault::Error(error)),
                #[allow(clippy::panic)]
                // Propogate the panic that occurred in call provided by client.
                Outcome::Panic(panic) => panic!(panic),
            },
            Err(failure) => match failure {
                ConsumeFailure::EmptyStock => throw!(Self::Failure::EmptyStock),
                ConsumeFailure::Fault(_) => throw!(Fault::Killed),
            },
        }
    }
}
