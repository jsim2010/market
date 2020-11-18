//! Implements [`Producer`] and [`Consumer`] for a [`Vec`] of actors.
use {
    crate::{map, ConsumeFailure, Consumer, Fault, ProduceFailure, Producer},
    core::{
        convert::{TryFrom, TryInto},
        fmt::Debug,
    },
    fehler::throws,
};

/// A [`Consumer`] that consumes goods of type `G` from multiple [`Consumer`]s.
pub struct Collector<G, T> {
    /// The [`Consumer`]s.
    consumers: Vec<Box<dyn Consumer<Good = G, Failure = ConsumeFailure<T>>>>,
}

impl<G, T> Collector<G, T> {
    /// Creates a new, empty [`Collector`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `consumer` to the end of the [`Consumer`]s held by `self`.
    #[inline]
    pub fn push<C>(&mut self, consumer: C)
    where
        C: Consumer + 'static,
        G: From<C::Good> + 'static,
        T: From<Fault<C::Failure>> + TryFrom<ConsumeFailure<T>> + 'static,
    {
        self.consumers.push(Box::new(map::Adapter::new(consumer)));
    }
}

impl<G, T> Consumer for Collector<G, T>
where
    T: TryFrom<ConsumeFailure<T>>,
{
    type Good = G;
    type Failure = ConsumeFailure<T>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        let mut result = Err(ConsumeFailure::EmptyStock);

        for consumer in &self.consumers {
            result = consumer.consume();

            if let Err(ConsumeFailure::EmptyStock) = result {
                // Nothing good or bad was found, continue searching.
            } else {
                break;
            }
        }

        result?
    }
}

impl<G, T> Debug for Collector<G, T> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Collector {{ .. }}")
    }
}

// Manually impl Default as derive macro requires G and T be Default.
impl<G, T> Default for Collector<G, T> {
    #[inline]
    fn default() -> Self {
        Self {
            consumers: Vec::new(),
        }
    }
}

/// Distributes goods to multiple producers.
pub struct Distributor<G, T> {
    /// The producers.
    producers: Vec<Box<dyn Producer<Good = G, Failure = ProduceFailure<T>>>>,
}

impl<G, T> Distributor<G, T> {
    /// Creates a new, empty [`Distributor`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `producer` to the end of the [`Producers`]s held by `self`.
    #[inline]
    pub fn push<P: Producer + 'static>(&mut self, producer: P)
    where
        G: TryInto<P::Good> + 'static,
        T: From<Fault<P::Failure>> + TryFrom<ProduceFailure<T>> + 'static,
    {
        self.producers.push(Box::new(map::Converter::new(producer)));
    }
}

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
    T: TryFrom<ProduceFailure<T>>,
    G: Clone,
{
    type Good = G;
    type Failure = ProduceFailure<T>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        for producer in &self.producers {
            producer.produce(good.clone())?;
        }
    }
}
