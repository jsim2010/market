//! Implements [`Producer`] and [`Consumer`] for a [`Vec`] of actors.
use {
    crate::{Consumer, ConsumerFailure, ProducerFailure, Producer},
    core::{fmt::Debug, convert::{TryInto, TryFrom}},
    fehler::throws,
};

// TODO: Collector and Distributor will need to be generic based on how they choose the order of actors.
/// A [`Consumer`] that consumes goods of type `G` from multiple [`Consumer`]s.
pub struct Collector<G, T> 
{
    /// The [`Consumer`]s.
    consumers: Vec<Box<dyn Consumer<Good = G, Failure = ConsumerFailure<T>>>>,
}

impl<G, T> Collector<G, T>
{
    /// Creates a new, empty [`Collector`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `consumer` to the end of the [`Consumer`]s held by `self`.
    #[inline]
    pub fn push<C>(&mut self, consumer: std::rc::Rc<C>)
    where
        C: Consumer + 'static,
        G: From<C::Good> + 'static,
        T: TryFrom<ConsumerFailure<T>> + 'static,
        ConsumerFailure<T>: From<C::Failure>,
    {
        self.consumers.push(Box::new(crate::map::Adapter::new(consumer)));
    }
}

impl<G, T> Consumer for Collector<G, T>
where
    T: TryFrom<ConsumerFailure<T>>,
{
    type Good = G;
    type Failure = ConsumerFailure<T>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        let mut result = Err(ConsumerFailure::EmptyStock);

        for consumer in &self.consumers {
            result = consumer.consume();

            if let Err(ConsumerFailure::EmptyStock) = result {
                // Nothing good or bad was found, continue searching.
            } else {
                break;
            }
        }

        result?
    }
}

// TODO: Should attempt to output debug of consumers if possible.
impl<G, T> Debug for Collector<G, T>
{
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Collector {{ .. }}")
    }
}

// Manually impl Default as derive macro requires G and T be Default.
impl<G, T> Default for Collector<G, T> {
    fn default() -> Self {
        Self {
            consumers: Vec::new(),
        }
    }
}

/// Distributes goods to multiple producers.
pub struct Distributor<G, T> {
    /// The producers.
    producers: Vec<Box<dyn Producer<Good = G, Failure = ProducerFailure<T>>>>,
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
        P: Producer + 'static,
        G: TryInto<P::Good> + 'static,
        ProducerFailure<T>: From<P::Failure>,
        T: TryFrom<ProducerFailure<T>> + 'static,
    {
        self.producers.push(Box::new(crate::map::Converter::new(producer)));
    }
}

// TODO: Should attempt to output debug of producers if possible.
impl<G, T> Debug for Distributor<G, T> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Distributor {{ .. }}")
    }
}

// Manually impl Default as derive macro requires G and T be Default.
impl<G, T> Default for Distributor<G, T> {
    #[inline]
    fn default() -> Self {
        Self {
            producers: Vec::new(),
        }
    }
}

impl<G, T> Producer for Distributor<G, T>
where
    T: TryFrom<ProducerFailure<T>>,
    G: Clone,
{
    type Good = G;
    type Failure = ProducerFailure<T>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        for producer in &self.producers {
            producer.produce(good.clone())?;
        }
    }
}
