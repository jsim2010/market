//! Implements actors that map goods and errors.
use {
    crate::{error, Consumer, ClassicalProducerFailure, Producer},
    core::{convert::TryInto, marker::PhantomData},
    fehler::throws,
    std::{error::Error, rc::Rc},
};

/// A [`Consumer`] that maps the consumed good to a new good.
#[derive(Debug)]
pub(crate) struct Adapter<C, G, F> {
    /// The original consumer.
    consumer: Rc<C>,
    /// The desired type of `Self::Good`.
    good: PhantomData<G>,
    /// The desired type of `Self::Failure`.
    failure: PhantomData<F>,
}

impl<C, G, F> Adapter<C, G, F> {
    /// Creates a new [`Adapter`].
    pub(crate) const fn new(consumer: Rc<C>) -> Self {
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
    F: From<C::Failure> + error::Failure,
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

/// A [`Producer`] that maps the produced good to a new good.
#[derive(Debug)]
pub(crate) struct Converter<P, G, T> {
    /// The original producer.
    producer: Rc<P>,
    /// The desired type of `Self::Good`.
    good: PhantomData<G>,
    /// The desired type of `Self::Error`.
    fault: PhantomData<T>,
}

impl<P, G, T> Converter<P, G, T> {
    /// Creates a new [`Converter`].
    pub(crate) const fn new(producer: Rc<P>) -> Self {
        Self {
            producer,
            good: PhantomData,
            fault: PhantomData,
        }
    }
}

impl<P, G, T> Producer for Converter<P, G, T>
where
    P: Producer<Failure = ClassicalProducerFailure<T>>,
    G: TryInto<P::Good>,
    T: core::convert::TryFrom<ClassicalProducerFailure<T>> + From<crate::error::Fault<P::Failure>> + Eq + Error,
{
    type Good = G;
    type Failure = ClassicalProducerFailure<T>;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: Self::Good) {
        if let Ok(converted_good) = good.try_into() {
            self.producer
                .produce(converted_good)
                .map_err(ClassicalProducerFailure::map_into)?
        }
    }
}
