//! Implements actors that map goods and errors.
use {
    crate::{ConsumeFailure, Consumer, ProduceFailure, Producer},
    core::{convert::TryInto, marker::PhantomData},
    fehler::throws,
    std::{error::Error, rc::Rc},
};

/// A [`Consumer`] that maps the consumed good to a new good.
#[derive(Debug)]
pub(crate) struct Adapter<C, G, T> {
    /// The original consumer.
    consumer: Rc<C>,
    /// The desired type of `Self::Good`.
    good: PhantomData<G>,
    /// The desired type of `ConsumerFault<Self>`.
    fault: PhantomData<T>,
}

impl<C, G, T> Adapter<C, G, T> {
    /// Creates a new [`Adapter`].
    pub(crate) const fn new(consumer: Rc<C>) -> Self {
        Self {
            consumer,
            good: PhantomData,
            fault: PhantomData,
        }
    }
}

impl<C, G, T> Consumer for Adapter<C, G, T>
where
    C: Consumer<Structure = crate::ClassicConsumer<T>>,
    G: From<C::Good>,
    T: From<crate::ConsumerFault<C>> + core::convert::TryFrom<ConsumeFailure<T>> + Error + 'static,
{
    type Good = G;
    type Structure = crate::ClassicConsumer<T>;

    #[inline]
    #[throws(crate::ConsumerFailure<Self>)]
    fn consume(&self) -> Self::Good {
        self.consumer
            .consume()
            .map(Self::Good::from)
            .map_err(ConsumeFailure::map_into)?
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
    P: Producer,
    <P as Producer>::Fault: 'static,
    G: TryInto<P::Good>,
    T: From<P::Fault> + Error,
{
    type Good = G;
    type Fault = T;

    #[inline]
    #[throws(ProduceFailure<Self::Fault>)]
    fn produce(&self, good: Self::Good) {
        if let Ok(converted_good) = good.try_into() {
            self.producer
                .produce(converted_good)
                .map_err(ProduceFailure::map_into)?
        }
    }
}
