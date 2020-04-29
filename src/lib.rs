//! A library to standardize the traits of producing and consuming.
//!
//! The core purpose of this library is to define the traits of items that interact with markets. A market stores goods in its stock. Producers add goods to the stock while consumers remove goods from the stock.
//!
//! One important characteristic of producers and consumers is that they are not modified by their respective action (producing or consuming). In other words, the signature of the action function starts with `fn action(&self`, not `fn action(&mut self`.

pub mod channel;
pub mod io;

use {
    core::{
        cell::RefCell,
        fmt::{self, Debug, Display},
        marker::PhantomData,
        sync::atomic::{AtomicBool, Ordering},
    },
    crossbeam_queue::SegQueue,
    fehler::{throw, throws},
    std::{error::Error, sync::mpsc},
    thiserror::Error as ThisError,
};

/// Consumes goods from the stock of a market.
///
/// The order in which goods are consumed is defined by the implementer.
pub trait Consumer {
    /// The type of the item being consumed.
    type Good;
    /// The type of the failure that could be thrown during consumption.
    type Failure;

    /// Attempts to consume the next good from the market without blocking.
    #[throws(ConsumeError<Self::Failure>)]
    fn consume(&self) -> Self::Good;

    /// Consumes the next good from the market, blocking until a good is available.
    #[inline]
    #[throws(Self::Failure)]
    fn demand(&self) -> Self::Good {
        let good;

        loop {
            match self.consume() {
                Ok(consumed_good) => {
                    good = consumed_good;
                    break;
                }
                Err(error) => {
                    if let ConsumeError::Failure(failure) = error {
                        throw!(failure);
                    }
                }
            }
        }

        good
    }

    /// Creates a [`Consumer`] that calls the appropriate map function on each consume.
    #[inline]
    fn map<M, F>(self, map: M, map_failure: F) -> MappedConsumer<Self, M, F>
    where
        Self: Sized,
    {
        MappedConsumer {
            consumer: self,
            map,
            map_failure,
        }
    }
}

/// Produces goods to be stored in the stock of a market.
///
/// Only goods which are desirable will be added to the market. The determination if a good is desirable is defined by the implementer.
#[allow(clippy::missing_inline_in_public_items)] // current issue with fehler for `fn produce()`; see https://github.com/withoutboats/fehler/issues/39
pub trait Producer {
    /// The type of the item being produced.
    type Good: Debug + Display;
    /// The type of the failure that could be thrown during production.
    type Failure: Error;

    /// Attempts to produce `good` to the market without blocking.
    ///
    /// An error is thrown if and only if `good` was desirable but was not stored.
    #[allow(redundant_semicolons, unused_variables)] // current issue with fehler; see https://github.com/withoutboats/fehler/issues/39
    #[throws(ProduceError<Self::Failure>)]
    fn produce(&self, good: Self::Good);

    /// Attempts to produce `good` without blocking and returns the good on failure.
    #[throws(RecallGood<Self::Good, ProduceError<Self::Failure>>)]
    fn produce_or_recall(&self, good: Self::Good)
    where
        Self::Good: Clone,
    {
        self.produce(good.clone())
            .map_err(|error| RecallGood { good, error })?
    }

    /// Produces `good` to the market, blocking if needed.
    ///
    /// An error is thrown if and only if `good` was desirable but was not stored.
    #[inline]
    #[throws(Self::Failure)]
    fn force(&self, good: Self::Good)
    where
        Self::Good: Clone,
    {
        let mut force_good = good;

        loop {
            match self.produce_or_recall(force_good) {
                Ok(()) => break,
                Err(RecallGood { good, error }) => match error {
                    ProduceError::FullStock => {
                        force_good = good;
                    }
                    ProduceError::Failure(failure) => throw!(failure),
                },
            }
        }
    }
}

/// A failure while consuming a good due to the market being closed.
#[derive(Clone, Copy, Debug, ThisError)]
#[error("market is closed")]
pub struct ClosedMarketFailure;

/// Describes why a good was not consumed.
#[derive(Debug)]
pub enum ConsumeError<F> {
    /// The stock of the market is empty.
    EmptyStock,
    /// A failure to consume a good.
    ///
    /// Indicates the [`Consumer`] will not consume any more goods in the current state.
    Failure(F),
}

// TODO: Have to impl here due to ConsumeError being declared here. Would make more sense in channel.
impl From<mpsc::TryRecvError> for ConsumeError<ClosedMarketFailure> {
    #[inline]
    fn from(value: mpsc::TryRecvError) -> Self {
        match value {
            mpsc::TryRecvError::Empty => Self::EmptyStock,
            mpsc::TryRecvError::Disconnected => Self::Failure(ClosedMarketFailure),
        }
    }
}

// TODO: Have to impl here due to ConsumeError being declared here. Would make more sense in channel.
impl From<crossbeam_channel::TryRecvError> for ConsumeError<ClosedMarketFailure> {
    #[inline]
    fn from(value: crossbeam_channel::TryRecvError) -> Self {
        match value {
            crossbeam_channel::TryRecvError::Empty => Self::EmptyStock,
            crossbeam_channel::TryRecvError::Disconnected => Self::Failure(ClosedMarketFailure),
        }
    }
}

/// An error thrown while producing a good.
#[derive(Debug, ThisError)]
#[error("unable to produce good `{good}`: {error}")]
pub struct RecallGood<G, F>
where
    G: Debug + Display,
    F: Error,
{
    /// The good that was not produced.
    good: G,
    /// The error.
    error: F,
}

impl<G, F> RecallGood<G, F>
where
    G: Debug + Display,
    F: Error,
{
    /// Creates a new [`RecallGood`].
    #[inline]
    pub fn new(good: G, error: F) -> Self {
        Self { good, error }
    }
}

// TODO: Have to impl here due to RecallGood being declared here. Would make more sense in channel.
impl<G> From<crossbeam_channel::TrySendError<G>> for ProduceError<ClosedMarketFailure> {
    #[inline]
    fn from(value: crossbeam_channel::TrySendError<G>) -> Self {
        match value {
            crossbeam_channel::TrySendError::Full(_) => Self::FullStock,
            crossbeam_channel::TrySendError::Disconnected(_) => Self::Failure(ClosedMarketFailure),
        }
    }
}

/// An error thrown while forcing a good.
#[derive(Debug)]
pub struct ForceGoodError<G, F> {
    /// The good that was not produced.
    good: G,
    /// The failure.
    failure: F,
}

impl<G, F> ForceGoodError<G, F> {
    /// Returns a pointer to the good that was not produced when `self` was thrown.
    #[inline]
    pub const fn good(&self) -> &G {
        &self.good
    }

    /// Returns a pointer to the failure that caused `self` to be thrown.
    #[inline]
    pub const fn failure(&self) -> &F {
        &self.failure
    }
}

/// An error thrown while producing.
#[derive(Debug, ThisError)]
pub enum ProduceError<F: Error> {
    /// An error to produce due to the stock of the market not having any room.
    #[error("stock is full")]
    FullStock,
    /// An error to produce due to an invalid state.
    #[error("failure: {0}")]
    Failure(F),
}

/// A [`Consumer`] that maps the consumed good to a new good.
pub struct MappedConsumer<C, M, F> {
    /// The original consumer.
    consumer: C,
    /// The [`Fn`] to map from `C::Good` to `Self::Good`.
    map: M,
    /// The [`Fn`] to map from `C::Failure` to `Self::Failure`.
    map_failure: F,
}

impl<G, E, C, M, F> Consumer for MappedConsumer<C, M, F>
where
    C: Consumer,
    M: Fn(C::Good) -> G,
    F: Fn(C::Failure) -> E,
{
    type Good = G;
    type Failure = E;

    #[inline]
    #[throws(ConsumeError<Self::Failure>)]
    fn consume(&self) -> Self::Good {
        self.consumer
            .consume()
            .map_err(|error| match error {
                ConsumeError::EmptyStock => ConsumeError::EmptyStock,
                ConsumeError::Failure(failure) => {
                    ConsumeError::Failure((self.map_failure)(failure))
                }
            })
            .map(|good| (self.map)(good))?
    }
}

impl<C, M, F> Debug for MappedConsumer<C, M, F>
where
    C: Debug,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MappedConsumer {{ consumer: {:?} }}", self.consumer)
    }
}

/// A [`Consumer`] that consumes goods of type `G` from multiple [`Consumer`]s.
#[derive(Default)]
pub struct Collector<G, E> {
    /// The [`Consumer`]s.
    consumers: Vec<Box<dyn Consumer<Good = G, Failure = E>>>,
}

impl<G, E> Collector<G, E> {
    /// Creates a new, empty [`Collector`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            consumers: Vec::new(),
        }
    }

    /// Converts `consumer` to an appropriate type then pushes it.
    #[inline]
    pub fn convert_into_and_push<C>(&mut self, consumer: C)
    where
        C: Consumer + 'static,
        G: From<C::Good> + 'static,
        E: From<C::Failure> + 'static,
    {
        self.push(consumer.map(G::from, E::from));
    }

    /// Adds `consumer` to the end of the [`Consumer`]s held by `self`.
    #[inline]
    pub fn push<C>(&mut self, consumer: C)
    where
        C: Consumer<Good = G, Failure = E> + 'static,
    {
        self.consumers.push(Box::new(consumer));
    }
}

impl<G, E> Consumer for Collector<G, E> {
    type Good = G;
    type Failure = E;

    #[inline]
    #[throws(ConsumeError<Self::Failure>)]
    fn consume(&self) -> Self::Good {
        let mut result = Err(ConsumeError::EmptyStock);

        for consumer in &self.consumers {
            result = match consumer.consume() {
                Ok(good) => Ok(good),
                Err(error) => match error {
                    ConsumeError::EmptyStock => continue,
                    ConsumeError::Failure(_) => Err(error),
                },
            };

            break;
        }

        result?
    }
}

impl<G, E> Debug for Collector<G, E> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Collector {{ .. }}")
    }
}

/// Converts a single good into parts.
pub trait StripFrom<G>
where
    Self: Sized,
{
    /// Converts `good` into [`Vec`] of parts.
    fn strip_from(good: &G) -> Vec<Self>;
}

/// A [`Producer`] of type `P` that produces parts stripped from goods of type `G`.
#[derive(Debug)]
pub struct StrippingProducer<G, P>
where
    P: Producer,
    <P as Producer>::Good: Debug,
{
    #[doc(hidden)]
    phantom: PhantomData<G>,
    /// The producer of parts.
    producer: P,
    /// Parts stripped from a composite good yet to be produced.
    parts: RefCell<Vec<<P as Producer>::Good>>,
}

impl<G, P> StrippingProducer<G, P>
where
    P: Producer,
    <P as Producer>::Good: Debug,
{
    /// Creates a new [`StrippingProducer`].
    #[inline]
    pub fn new(producer: P) -> Self {
        Self {
            producer,
            phantom: PhantomData,
            parts: RefCell::new(Vec::new()),
        }
    }
}

impl<G, P> Producer for StrippingProducer<G, P>
where
    P: Producer,
    G: Debug + Display,
    <P as Producer>::Good: StripFrom<G> + Clone + Debug,
    <P as Producer>::Failure: Error,
{
    type Good = G;
    type Failure = <P as Producer>::Failure;

    #[inline]
    #[throws(ProduceError<Self::Failure>)]
    fn produce(&self, good: Self::Good) {
        let parts = <<P as Producer>::Good>::strip_from(&good);

        for part in parts {
            if let Err(error) = self.producer.produce(part) {
                throw!(error);
            }
        }
    }
}

/// Consumes parts from a [`Consumer`] of composite goods.
#[derive(Debug)]
pub struct StrippingConsumer<C, P> {
    /// The consumer of composite goods.
    consumer: C,
    /// The queue of stripped parts.
    parts: SegQueue<P>,
}

impl<C, P> StrippingConsumer<C, P>
where
    C: Consumer,
    P: StripFrom<<C as Consumer>::Good>,
{
    /// Creates a new [`StrippingConsumer`]
    #[inline]
    pub fn new(consumer: C) -> Self {
        Self {
            consumer,
            parts: SegQueue::new(),
        }
    }

    /// Consumes all stocked composite goods and strips them into parts.
    ///
    /// Runs until a [`ConsumerError`] is thrown.
    fn strip(&self) -> ConsumeError<<C as Consumer>::Failure> {
        let error;

        loop {
            match self.consumer.consume() {
                Ok(composite) => {
                    for part in P::strip_from(&composite) {
                        self.parts.push(part);
                    }
                }
                Err(e) => {
                    error = e;
                    break;
                }
            }
        }

        error
    }
}

impl<C, P> Consumer for StrippingConsumer<C, P>
where
    C: Consumer,
    P: StripFrom<<C as Consumer>::Good> + Debug,
{
    type Good = P;
    type Failure = <C as Consumer>::Failure;

    #[inline]
    #[throws(ConsumeError<Self::Failure>)]
    fn consume(&self) -> Self::Good {
        // Store result of strip because all stocked parts should be consumed prior to failing.
        let error = self.strip();

        if let Ok(part) = self.parts.pop() {
            part
        } else {
            throw!(error);
        }
    }
}

/// The Error thrown when failing to compose parts into a composite good.
#[derive(Copy, Clone, Debug)]
pub struct NonComposible;

impl<T> From<NonComposible> for ConsumeError<T> {
    #[inline]
    fn from(_: NonComposible) -> Self {
        Self::EmptyStock
    }
}

/// Converts an array of parts into a composite good.
pub trait ComposeFrom<G>
where
    Self: Sized,
{
    #[throws(NonComposible)]
    /// Converts `parts` into a composite good.
    fn compose_from(parts: &mut Vec<G>) -> Self;
}

/// Consumes composite goods of type `G` from a parts [`Consumer`] of type `C`.
#[derive(Debug)]
pub struct ComposingConsumer<C, G>
where
    C: Consumer,
    <C as Consumer>::Good: Debug,
{
    /// The consumer.
    consumer: C,
    /// The current buffer of parts.
    buffer: RefCell<Vec<<C as Consumer>::Good>>,
    #[doc(hidden)]
    phantom: PhantomData<G>,
}

impl<C, G> ComposingConsumer<C, G>
where
    C: Consumer,
    <C as Consumer>::Good: Debug,
{
    /// Creates a new [`ComposingConsumer`].
    #[inline]
    pub fn new(consumer: C) -> Self {
        Self {
            consumer,
            buffer: RefCell::new(Vec::new()),
            phantom: PhantomData,
        }
    }
}

impl<C, G> Consumer for ComposingConsumer<C, G>
where
    C: Consumer,
    G: ComposeFrom<<C as Consumer>::Good> + Debug,
    <C as Consumer>::Good: Debug,
{
    type Good = G;
    type Failure = <C as Consumer>::Failure;

    #[inline]
    #[throws(ConsumeError<Self::Failure>)]
    fn consume(&self) -> Self::Good {
        let good = self.consumer.consume()?;
        let mut buffer = self.buffer.borrow_mut();
        buffer.push(good);
        G::compose_from(&mut buffer)?
    }
}

/// Inspects if goods meet defined requirements.
pub trait Inspector {
    /// The good to be inspected.
    type Good;

    /// Returns if `good` meets requirements.
    fn allows(&self, good: &Self::Good) -> bool;
}

/// Consumes goods that an [`Inspector`] has allowed.
#[derive(Debug)]
pub struct VigilantConsumer<C, I> {
    /// The consumer.
    consumer: C,
    /// The inspector.
    inspector: I,
}

impl<C, I> VigilantConsumer<C, I> {
    /// Creates a new [`VigilantConsumer`].
    #[inline]
    pub const fn new(consumer: C, inspector: I) -> Self {
        Self {
            consumer,
            inspector,
        }
    }
}

impl<C, I> Consumer for VigilantConsumer<C, I>
where
    C: Consumer,
    I: Inspector<Good = <C as Consumer>::Good> + Debug,
{
    type Good = <C as Consumer>::Good;
    type Failure = <C as Consumer>::Failure;

    #[inline]
    #[throws(ConsumeError<Self::Failure>)]
    fn consume(&self) -> Self::Good {
        let mut input;

        loop {
            input = self.consumer.consume()?;

            if self.inspector.allows(&input) {
                break;
            }
        }

        input
    }
}

/// Produces goods that an [`Inspector`] has allowed.
#[derive(Debug)]
pub struct ApprovedProducer<P, I> {
    /// The producer.
    producer: P,
    /// The inspector.
    inspector: I,
}

impl<P, I> ApprovedProducer<P, I> {
    /// Creates a new [`ApprovedProducer`].
    #[inline]
    pub const fn new(producer: P, inspector: I) -> Self {
        Self {
            producer,
            inspector,
        }
    }
}

impl<P, I> Producer for ApprovedProducer<P, I>
where
    P: Producer,
    <P as Producer>::Good: Debug + Display,
    <P as Producer>::Failure: Error,
    I: Inspector<Good = <P as Producer>::Good>,
{
    type Good = <P as Producer>::Good;
    type Failure = <P as Producer>::Failure;

    #[inline]
    #[throws(ProduceError<Self::Failure>)]
    fn produce(&self, good: Self::Good) {
        if self.inspector.allows(&good) {
            self.producer.produce(good)?
        }
    }
}

/// An error that will never occur, equivalent to `!`.
#[derive(Copy, Clone, Debug, ThisError)]
pub enum NeverErr {}

/// Defines a queue with unlimited size that implements [`Consumer`] and [`Producer`].
///
/// An [`UnlimitedQueue`] can be closed, which prevents the [`Producer`] from producing new goods while allowing the [`Consumer`] to consume only the remaining goods on the queue.
#[derive(Debug)]
pub struct UnlimitedQueue<G> {
    /// The queue.
    queue: SegQueue<G>,
    /// If the queue is closed.
    is_closed: AtomicBool,
}

impl<G> UnlimitedQueue<G> {
    /// Creates a new empty [`UnlimitedQueue`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Closes `self`.
    #[inline]
    pub fn close(&self) {
        self.is_closed.store(true, Ordering::Relaxed);
    }
}

impl<G> Consumer for UnlimitedQueue<G>
where
    G: Debug,
{
    type Good = G;
    type Failure = ClosedMarketFailure;

    #[inline]
    #[throws(ConsumeError<Self::Failure>)]
    fn consume(&self) -> Self::Good {
        match self.queue.pop() {
            Ok(good) => good,
            Err(_) => {
                if self.is_closed.load(Ordering::Relaxed) {
                    throw!(ConsumeError::Failure(ClosedMarketFailure));
                } else {
                    throw!(ConsumeError::EmptyStock);
                }
            }
        }
    }
}

impl<G> Default for UnlimitedQueue<G> {
    #[inline]
    fn default() -> Self {
        Self {
            queue: SegQueue::new(),
            is_closed: AtomicBool::new(false),
        }
    }
}

impl<G> Producer for UnlimitedQueue<G>
where
    G: Debug + Display,
{
    type Good = G;
    type Failure = ClosedMarketFailure;

    #[inline]
    #[throws(ProduceError<Self::Failure>)]
    fn produce(&self, good: Self::Good) {
        if self.is_closed.load(Ordering::Relaxed) {
            throw!(ProduceError::Failure(ClosedMarketFailure));
        } else {
            self.queue.push(good);
        }
    }
}

/// An unlimited queue with a producer and consumer that are always functional.
#[derive(Debug, Default)]
pub struct PermanentQueue<G> {
    /// The queue.
    queue: SegQueue<G>,
}

impl<G> PermanentQueue<G> {
    /// Creates a new [`PermanentQueue`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            queue: SegQueue::new(),
        }
    }
}

impl<G> Consumer for PermanentQueue<G>
where
    G: Debug,
{
    type Good = G;
    type Failure = NeverErr;

    #[inline]
    #[throws(ConsumeError<Self::Failure>)]
    fn consume(&self) -> Self::Good {
        self.queue.pop().map_err(|_| ConsumeError::EmptyStock)?
    }
}

impl<G> Producer for PermanentQueue<G>
where
    G: Debug + Display,
{
    type Good = G;
    type Failure = NeverErr;

    #[inline]
    #[throws(ProduceError<Self::Failure>)]
    fn produce(&self, good: Self::Good) {
        self.queue.push(good);
    }
}
