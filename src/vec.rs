//! Implements [`Producer`] and [`Consumer`] for a [`Vec`] of actors.
use {
    core::{fmt::Debug, convert::{TryInto, TryFrom}},
    fehler::throws,
};

// TODO: Collector and Distributor will need to be generic based on how they choose the order of actors.
/// A [`Consumer`] that consumes goods of type `G` from multiple [`Consumer`]s.
#[derive(Default)]
pub struct Collector<G, T> 
{
    /// The [`Consumer`]s.
    consumers: Vec<Box<dyn crate::Consumer<Good = G, Failure = crate::ConsumerFailure<T>>>>,
}

impl<G, T> Collector<G, T>
{
    /// Creates a new, empty [`Collector`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            consumers: Vec::new(),
        }
    }

    /// Adds `consumer` to the end of the [`Consumer`]s held by `self`.
    #[inline]
    pub fn push<C>(&mut self, consumer: std::rc::Rc<C>)
    where
        C: crate::Consumer + 'static,
        G: From<C::Good> + 'static,
        T: TryFrom<crate::ConsumerFailure<T>> + 'static,
        crate::ConsumerFailure<T>: From<C::Failure>,
    {
        self.consumers.push(Box::new(crate::map::Adapter::new(consumer)));
    }
}

impl<G, T> crate::Consumer for Collector<G, T>
where
    T: TryFrom<crate::ConsumerFailure<T>>,
{
    type Good = G;
    type Failure = crate::ConsumerFailure<T>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        let mut result = Err(crate::ConsumerFailure::EmptyStock);

        for consumer in &self.consumers {
            result = consumer.consume();

            if let Err(crate::ConsumerFailure::EmptyStock) = result {
                // Nothing good or bad was found, continue searching.
            } else {
                break;
            }
        }

        result?
    }
}

impl<G, E> Debug for Collector<G, E>
where
    E: Debug,
{
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Collector {{ .. }}")
    }
}

/// Distributes goods to multiple producers.
pub struct Distributor<G, T> {
    /// The producers.
    producers: Vec<Box<dyn crate::Producer<Good = G, Failure = crate::error::ProducerFailure<T>>>>,
}

impl<G, T> Distributor<G, T>
{
    /// Creates a new, empty [`Distributor`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `producer` to the end of the [`Producers`]s held by `self`.
    #[inline]
    pub fn push<P>(&mut self, producer: std::rc::Rc<P>)
    where
        P: crate::Producer + 'static,
        G: TryInto<P::Good> + 'static,
        crate::error::ProducerFailure<T>: From<P::Failure>,
        T: TryFrom<crate::error::ProducerFailure<T>> + 'static,
    {
        self.producers.push(Box::new(crate::map::Converter::new(producer)));
    }
}

impl<G, T> Debug for Distributor<G, T> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Distributor {{ .. }}")
    }
}

impl<G, T> Default for Distributor<G, T> {
    #[inline]
    fn default() -> Self {
        Self {
            producers: Vec::new(),
        }
    }
}

impl<G, T> crate::Producer for Distributor<G, T>
where
    T: TryFrom<crate::error::ProducerFailure<T>>,
    G: Clone,
{
    type Good = G;
    type Failure = crate::error::ProducerFailure<T>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        for producer in &self.producers {
            producer.produce(good.clone())?;
        }
    }
}
