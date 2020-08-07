//! Implements `Consumer` for thread functionality.
use {
    crate::{
        channel::{CrossbeamConsumer, CrossbeamProducer},
        ClosedMarketError, ConsumeFailure, Consumer, Producer,
    },
    core::fmt::{Debug, Display},
    fehler::throws,
    parse_display::Display as ParseDisplay,
    std::thread,
    thiserror::Error as ThisError,
};

/// A wrapper around the `std::thread` functionality.
#[derive(Debug)]
pub struct Thread<T>
where
    T: Debug,
{
    /// Consumes the output of the thread.
    consumer: CrossbeamConsumer<T>,
}

impl<T> Thread<T>
where
    T: Clone + Debug + Display + Send + 'static,
{
    /// Creates a new `Thread`.
    #[allow(unused_results)] // JoinHandle is not used.
    #[inline]
    pub fn new<F>(thread: F) -> Self
    where
        F: FnOnce() -> T + Send + 'static,
    {
        let (tx, rx) = crossbeam_channel::bounded(1);

        thread::spawn(move || {
            #[allow(clippy::expect_used)] // Nothing can be done if production fails.
            CrossbeamProducer::from(tx)
                .force((thread)())
                .expect("unable to send thread result");
        });

        Self {
            consumer: rx.into(),
        }
    }
}

impl<T> Consumer for Thread<T>
where
    T: Debug,
{
    type Good = T;
    type Error = Panic;

    #[throws(ConsumeFailure<Self::Error>)]
    #[inline]
    fn consume(&self) -> Self::Good {
        self.consumer.consume().map_err(ConsumeFailure::map_into)?
    }
}

/// A panic in a thread.
#[derive(Clone, Copy, Debug, ThisError)]
#[error("thread panicked")]
pub struct Panic;

impl From<ClosedMarketError> for Panic {
    #[inline]
    fn from(_: ClosedMarketError) -> Self {
        Self
    }
}

/// An empty item.
// Needed to implement Display.
#[derive(Clone, Debug, ParseDisplay)]
#[display("")]
pub(crate) struct Void;
