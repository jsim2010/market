//! Implements [`Consumer`] for a thread.
//!
//! A thread consists of code that is executed separately. When the code is completed, the status can be consumed.
use {
    crate::{
        sync::{create_delivery, create_lock, Accepter, Deliverer, Trigger},
        ConsumeFailure, Consumer, Producer,
    },
    core::convert::TryFrom,
    fehler::{throw, throws},
    std::{
        any::Any,
        panic::{catch_unwind, AssertUnwindSafe, RefUnwindSafe},
        thread::spawn,
    },
};

/// The type returned by [`catch_unwind()`] when a panic is caught.
type Panic = Box<dyn Any + Send + 'static>;

/// The type returned by a thread call which can represent a success of type `S`, an error of type `E`, or a panic.
#[derive(Debug, parse_display::Display)]
enum Status<S, E> {
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

impl<S, E> Status<S, E> {
    /// If `self` represents a success.
    const fn is_success(&self) -> bool {
        matches!(*self, Self::Success(_))
    }
}

/// Describes the kind of thread.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    /// Runs the call a single time.
    Single,
    /// Will continue running call until cancelled.
    Cancelable,
}

/// A wrapper around the [`std::thread`] functionality.
///
/// Consumes the status of running the given closure. Thus, consumption replaces the functionality of [`std::thread::JoinHandle::join()`].
#[derive(Debug)]
pub struct Thread<S, E> {
    /// Consumes the status of the call.
    consumer: Accepter<Status<S, E>>,
    /// [`Trigger`] to cancel a cancelable thread.
    trigger: Option<Trigger>,
}

impl<S: Send + 'static, E: TryFrom<ConsumeFailure<E>> + Send + 'static> Thread<S, E> {
    /// Creates a new [`Thread`] and spawns `call`.
    #[inline]
    pub fn new<
        P: Send + 'static,
        F: FnMut(&mut P) -> Result<S, E> + RefUnwindSafe + Send + 'static,
    >(
        kind: Kind,
        mut parameters: P,
        mut call: F,
    ) -> Self {
        let (producer, consumer) = create_delivery::<Status<S, E>>();

        match kind {
            Kind::Single => {
                let _ = spawn(move || {
                    Self::produce_outcome(Self::run(&mut parameters, &mut call), &producer);
                });

                Self {
                    consumer,
                    trigger: None,
                }
            }
            Kind::Cancelable => {
                let (trigger, hammer) = create_lock();
                let _ = spawn(move || {
                    let mut status = Self::run(&mut parameters, &mut call);

                    while hammer.consume().is_err() && status.is_success() {
                        status = Self::run(&mut parameters, &mut call);
                    }

                    Self::produce_outcome(status, &producer);
                });

                Self {
                    consumer,
                    trigger: Some(trigger),
                }
            }
        }
    }

    /// Runs `call` and catches any panics.
    fn run<P, F: FnMut(&mut P) -> Result<S, E> + RefUnwindSafe + Send + 'static>(
        mut parameters: &mut P,
        call: &mut F,
    ) -> Status<S, E> {
        match catch_unwind(AssertUnwindSafe(|| (call)(&mut parameters))) {
            Ok(Ok(success)) => Status::Success(success),
            Ok(Err(error)) => Status::Error(error),
            Err(panic) => Status::Panic(panic),
        }
    }

    /// Produces `status` via `producer`.
    fn produce_outcome(status: Status<S, E>, producer: &Deliverer<Status<S, E>>) {
        // Although force is preferable to produce, force requires status impl Clone and the panic value is not bound to impl Clone. Using produce should be fine because produce should never be blocked since this market has a single producer storing a single good.
        #[allow(clippy::unwrap_used)]
        // Passer::produce() can only fail when the stock is full. Since we only call this once, this should never happen.
        producer.produce(status).unwrap();
    }

    /// Requests that `self` be canceled.
    #[inline]
    pub fn cancel(&self) {
        if let Some(trigger) = self.trigger.as_ref() {
            #[allow(clippy::unwrap_used)] // Trigger::produce() cannot fail.
            trigger.produce(()).unwrap();
        }
    }
}

impl<S, E: TryFrom<ConsumeFailure<E>>> Consumer for Thread<S, E> {
    type Good = S;
    type Failure = ConsumeFailure<E>;

    #[allow(clippy::panic_in_result_fn)] // Propogate the panic that occurred in call provided by client.
    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        match self.consumer.consume() {
            Ok(status) => match status {
                Status::Success(success) => success,
                Status::Error(error) => throw!(error),
                #[allow(clippy::panic)]
                // Propogate the panic that occurred in call provided by client.
                Status::Panic(panic) => panic!(panic),
            },
            // Accepter::Failure is FaultlessFailure so a failure means the stock is empty.
            Err(_) => throw!(ConsumeFailure::EmptyStock),
        }
    }
}
