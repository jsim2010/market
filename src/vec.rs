//! Implements [`Producer`] and [`Consumer`] for a [`Vec`] of actors.
use {
    crate::{EmptyStockFailure, map, ConsumeFailure, Consumer, ProduceFailure, Failure, Producer},
    core::{
        convert::{Infallible, TryFrom, TryInto},
        fmt::Debug,
    },
    fehler::throws,
};

/// A [`Consumer`] that consumes goods of type `G` from multiple [`Consumer`]s.
pub struct Collector<G, T> {
    /// The [`Consumer`]s.
    consumers: Vec<Box<dyn Consumer<Good = G, Failure = CollectFailure<T>>>>,
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
        T: TryFrom<CollectFailure<T>> + 'static,
        CollectFailure<T>: From<<C as Consumer>::Failure>,
    {
        self.consumers.push(Box::new(map::Adapter::new(consumer)));
    }
}

impl<G, T> Consumer for Collector<G, T>
where
    T: TryFrom<CollectFailure<T>>,
{
    type Good = G;
    type Failure = CollectFailure<T>;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        let mut result = Err(CollectFailure::EmptyStock);

        for consumer in &self.consumers {
            result = consumer.consume();

            if let Err(CollectFailure::EmptyStock) = result {
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

pub enum CollectFailure<T> {
    EmptyStock,
    Fault(T),
}

impl<T: TryFrom<Self>> Failure for CollectFailure<T> {
    type Fault = T;
}

impl<F, T: From<F>> From<ConsumeFailure<F>> for CollectFailure<T> {
    fn from(failure: ConsumeFailure<F>) -> Self {
        match failure {
            ConsumeFailure::EmptyStock => Self::EmptyStock,
            ConsumeFailure::Fault(fault) => Self::Fault(fault.into())
        }
    }
}

impl<T> From<EmptyStockFailure> for CollectFailure<T> {
    fn from(_: EmptyStockFailure) -> Self {
        CollectFailure::EmptyStock
    }
}

impl<T> From<CollectFailure<T>> for ConsumeFailure<T> {
    fn from(failure: CollectFailure<T>) -> Self {
        match failure {
            CollectFailure::EmptyStock => Self::EmptyStock,
            CollectFailure::Fault(fault) => Self::Fault(fault),
        }
    }
}

/// Distributes goods to multiple producers.
pub struct Distributor<G, T> {
    /// The producers.
    producers: Vec<Box<dyn Producer<Good = G, Failure = DistributeFailure<T>>>>,
}

impl<G, T> Distributor<G, T> {
    /// Creates a new, empty [`Distributor`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `producer` to the end of the [`Producer`]s held by `self`.
    #[inline]
    pub fn push<P: Producer + 'static>(&mut self, producer: P)
    where
        G: TryInto<P::Good> + 'static,
        T: TryFrom<DistributeFailure<T>> + 'static,
        DistributeFailure<T>: From<<P as Producer>::Failure>,
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
    T: TryFrom<DistributeFailure<T>>,
    G: Clone,
{
    type Good = G;
    type Failure = DistributeFailure<T>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        for producer in &self.producers {
            producer.produce(good.clone())?;
        }
    }
}

pub enum DistributeFailure<T> {
    FullStock,
    Fault(T),
}

impl<T: TryFrom<Self>> Failure for DistributeFailure<T> {
    type Fault = T;
}

impl<F, T: From<F>> From<ProduceFailure<F>> for DistributeFailure<T> {
    fn from(failure: ProduceFailure<F>) -> Self {
        match failure {
            ProduceFailure::FullStock => Self::FullStock,
            ProduceFailure::Fault(fault) => Self::Fault(fault.into())
        }
    }
}

impl<T> From<Infallible> for DistributeFailure<T> {
    fn from(infallible: Infallible) -> Self {
        infallible.into()
    }
}

impl<T> From<DistributeFailure<T>> for ProduceFailure<T> {
    fn from(failure: DistributeFailure<T>) -> Self {
        match failure {
            DistributeFailure::FullStock => Self::FullStock,
            DistributeFailure::Fault(fault) => Self::Fault(fault),
        }
    }
}
