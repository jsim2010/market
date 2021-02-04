//! Implements [`Producer`] and [`Consumer`] for serializing and deserializing goods.
use {
    crate::{
        queue::{self, Procurer, Supplier},
        ConsumeFailure, Consumer, Failure, InsufficientStockFailure, Producer,
    },
    conventus::{AssembleFailure, AssembleFrom, DisassembleInto},
    core::{
        cell::RefCell,
        convert::{Infallible, TryFrom},
        marker::PhantomData,
    },
    fehler::throws,
};

/// Creates items for assembling parts into composites.
#[inline]
#[must_use]
pub(crate) fn create_assembly_line<P, C: AssembleFrom<P>>() -> (PartsInput<P>, Assembler<P, C>) {
    let (supplier, procurer) = queue::create_supply_chain();
    (
        PartsInput { supplier },
        Assembler {
            procurer,
            buffer: RefCell::new(Vec::new()),
            composite: PhantomData,
        },
    )
}

/// Creates items for disassembling composites into parts.
#[inline]
#[must_use]
pub(crate) fn create_disassembly_line<P, C: DisassembleInto<P>>(
) -> (Disassembler<P, C>, PartsOutput<P>) {
    let (supplier, procurer) = queue::create_supply_chain();
    (
        Disassembler {
            supplier,
            composite: PhantomData,
        },
        PartsOutput { procurer },
    )
}

/// Produces parts to be assembled into a composite.
#[derive(Debug)]
pub(crate) struct PartsInput<P> {
    /// Supplies parts.
    supplier: Supplier<P>,
}

impl<P> Producer for PartsInput<P> {
    type Good = P;
    type Failure = Infallible;

    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: P) {
        self.supplier.produce(good)?
    }
}

/// Consumes composites assembled from parts supplied to a [`PartsInput`].
#[derive(Debug)]
pub(crate) struct Assembler<P, C: AssembleFrom<P>> {
    /// Procures parts.
    procurer: Procurer<P>,
    /// Buffer of parts yet to be assembled.
    buffer: RefCell<Vec<P>>,
    /// The type of the composite good.
    composite: PhantomData<C>,
}

impl<P, C: AssembleFrom<P>> Consumer for Assembler<P, C>
where
    <C as AssembleFrom<P>>::Error: TryFrom<ConsumeFailure<<C as AssembleFrom<P>>::Error>>,
{
    type Good = C;
    type Failure = ConsumeFailure<<C as AssembleFrom<P>>::Error>;

    #[allow(clippy::unwrap_in_result)] // Unwrapping Result<_, Infallible>.
    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        // Collect all parts before processing to avoid processing each part.
        #[allow(clippy::unwrap_used)] // Unwrapping Result<_, Infallible>.
        let mut parts = self
            .procurer
            .goods()
            .collect::<Result<Vec<P>, _>>()
            .unwrap();
        let mut buffer = self.buffer.borrow_mut();
        buffer.append(&mut parts);
        C::assemble_from(&mut buffer).map_err(|error| match error {
            AssembleFailure::Incomplete => ConsumeFailure::EmptyStock,
            AssembleFailure::Error(e) => ConsumeFailure::Fault(e),
        })?
    }
}

/// Produces composites by disassembling them into parts.
#[derive(Debug)]
pub(crate) struct Disassembler<P, C: DisassembleInto<P>> {
    /// Produces disassembled parts.
    supplier: Supplier<P>,
    /// The type of the good to be disassembled.
    composite: PhantomData<C>,
}

impl<P, C: DisassembleInto<P>> Producer for Disassembler<P, C>
where
    <C as DisassembleInto<P>>::Error: Failure,
{
    type Good = C;
    type Failure = <C as DisassembleInto<P>>::Error;

    #[allow(clippy::unwrap_in_result)] // Supplier::produce() returns Result<_, Infallible>.
    #[inline]
    #[throws(Self::Failure)]
    fn produce(&self, good: C) {
        #[allow(clippy::unwrap_used)] // Supplier::produce() returns Result<_, Infallible>.
        self.supplier.produce_all(good.disassemble_into()?).unwrap()
    }
}

/// Consumes parts of a composite produced by a [`Disassembler`].
#[derive(Debug)]
pub(crate) struct PartsOutput<P> {
    /// Produces the parts.
    procurer: Procurer<P>,
}

impl<P> Consumer for PartsOutput<P> {
    type Good = P;
    type Failure = InsufficientStockFailure;

    #[inline]
    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        self.procurer.consume()?
    }
}
