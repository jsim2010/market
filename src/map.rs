//! Implements actors that map goods and errors.
use {
    crate::{Consumer, Failure, Producer},
    core::{convert::TryInto, marker::PhantomData},
    fehler::throws,
};

/// A [`Consumer`] that maps the consumption result from `C` to a `Result<G, F>`.
#[derive(Debug)]
pub(crate) struct Adapter<C, G, F> {
    /// The original consumer.
    consumer: C,
    /// The desired type of `Self::Good`.
    good: PhantomData<G>,
    /// The desired type of `Self::Failure`.
    failure: PhantomData<F>,
}

impl<C, G, F> Adapter<C, G, F> {
    /// Creates a new [`Adapter`].
    pub(crate) const fn new(consumer: C) -> Self {
        Self {
            consumer,
            good: PhantomData,
            failure: PhantomData,
        }
    }
}

impl<C, G, F> Consumer for Adapter<C, G, F>
where
    C: Consumer,
    G: From<C::Good>,
    F: Failure + From<C::Failure>,
{
    type Good = G;
    type Failure = F;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        self.consumer
            .consume()
            .map(Self::Good::from)
            .map_err(Self::Failure::from)?
    }
}

/// A [`Producer`] that maps goods from `G` to `P::Good` and maps failures from `P::Failure` to `F`.
#[derive(Debug)]
pub(crate) struct Converter<P, G, F> {
    /// The original producer.
    producer: P,
    /// The desired type of `Self::Good`.
    good: PhantomData<G>,
    /// The desired type of `Self::Failure`.
    failure: PhantomData<F>,
}

impl<P, G, F> Converter<P, G, F> {
    /// Creates a new [`Converter`].
    pub(crate) const fn new(producer: P) -> Self {
        Self {
            producer,
            good: PhantomData,
            failure: PhantomData,
        }
    }
}

impl<P: Producer, G: TryInto<P::Good>, F: Failure + From<P::Failure>> Producer
    for Converter<P, G, F>
{
    type Good = G;
    type Failure = F;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        if let Ok(converted_good) = good.try_into() {
            self.producer
                .produce(converted_good)
                .map_err(Self::Failure::from)?
        }
    }
}
