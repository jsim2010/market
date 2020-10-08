//! Implements `Consumer` for thread functionality.
use {
    crate::{
        channel::{CrossbeamConsumer, CrossbeamProducer},
        ConsumeFailure, Consumer, Producer,
    },
    core::fmt::{Debug, Display},
    fehler::{throw, throws},
    parse_display::Display as ParseDisplay,
    std::{error::Error, thread},
    thiserror::Error as ThisError,
};

/// A wrapper around the `std::thread` functionality.
#[derive(Debug)]
pub struct Thread<O, E>
where
    E: Debug,
    O: Debug,
{
    /// Consumes the output of the thread.
    consumer: CrossbeamConsumer<Output<O, E>>,
}

impl<O, E> Thread<O, E>
where
    E: Clone + Debug + Display + Send + 'static,
    O: Clone + Debug + Display + Send + 'static,
{
    /// Creates a new `Thread`.
    #[allow(unused_results)] // JoinHandle is not used.
    #[inline]
    pub fn new<F>(thread: F) -> Self
    where
        F: FnOnce() -> Result<O, E> + Send + 'static,
    {
        let (tx, rx) = crossbeam_channel::bounded(1);

        thread::spawn(move || {
            #[allow(clippy::expect_used)] // Nothing can be done if production fails.
            CrossbeamProducer::from(tx)
                .force(Output::from((thread)()))
                .expect("unable to send thread result");
        });

        Self {
            consumer: rx.into(),
        }
    }
}

impl<O, E> Consumer for Thread<O, E>
where
    E: Error + 'static,
    O: Debug,
{
    type Good = O;
    type Fault = ConsumeThreadError<E>;

    #[throws(ConsumeFailure<Self::Fault>)]
    #[inline]
    fn consume(&self) -> Self::Good {
        match self.consumer.consume() {
            Ok(output) => match output {
                Output::Success(success) => success,
                Output::Error(error) => throw!(ConsumeThreadError::Error(error)),
            },
            Err(_) => throw!(ConsumeThreadError::Dropped),
        }
    }
}

/// A proxy for Result that implements Display.
#[derive(Clone, Debug, ParseDisplay)]
pub enum Output<O, E> {
    /// The thread returned an output.
    #[display("{0}")]
    Success(O),
    /// An thread threw an error.
    #[display("ERROR: {0}")]
    Error(E),
}

impl<O, E> From<Result<O, E>> for Output<O, E> {
    #[inline]
    fn from(result: Result<O, E>) -> Self {
        match result {
            Ok(success) => Self::Success(success),
            Err(error) => Self::Error(error),
        }
    }
}

/// An error while consuming thread output.
#[derive(Debug, ThisError)]
pub enum ConsumeThreadError<E>
where
    E: Debug + Display + Error + 'static,
{
    /// The thread was dropped.
    #[error("thread was dropped before output could be consumed")]
    Dropped,
    /// The thread threw an error.
    #[error(transparent)]
    Error(E),
}

/// A proxy for () that implements Display.
#[derive(Clone, Debug, ParseDisplay)]
#[display("")]
pub(crate) struct Void;
