trait Flaws {
    type Defect;
    type Insufficiency;
}

// Provides the same functionality as core::convert::Into without a generic implementation of T: Blame<T>, which causes conflicts. This will be deprecated when specialization is stabilized in favor of core::convert::From.
trait Blame<T> {
    fn blame(self) -> T;
}

// Provides the same functionality as core::convert::TryInto without a generic implementation of T: TryBlame<T>, which causes conflicts. This will be deprecated when specialization is stabilized in favor of core::convert::TryFrom.
trait TryBlame<T> {
    type Error;

    fn try_blame(self) -> core::result::Result<T, Self::Error>;
}

#[non_exhaustive]
enum FaultConversionError<F, W>
where
    W: Flaws,
    F: Flaws<
        Insufficiency: core::convert::TryFrom<W::Insufficiency>,
        Defect: core::convert::TryFrom<W::Defect>
    >,
{
    Insufficiency(<F::Insufficiency as core::convert::TryFrom<W::Insufficiency>>::Error),
    Defect(<F::Defect as core::convert::TryFrom<W::Defect>>::Error),
}

impl<F, W> core::fmt::Debug for FaultConversionError<F, W>
where
    W: Flaws,
    F: Flaws<
        Insufficiency: core::convert::TryFrom<W::Insufficiency><Error: core::fmt::Debug>,
        Defect: core::convert::TryFrom<W::Defect><Error: core::fmt::Debug>,
    >,
{}

impl<F, W> core::fmt::Display for FaultConversionError<F, W>
where
    W: Flaws,
    F: Flaws<
        Insufficiency: core::convert::TryFrom<W::Insufficiency><Error: core::fmt::Display>,
        Defect: core::convert::TryFrom<W::Defect><Error: core::fmt::Display>,
    >,
{}

#[cfg(feature = "std")]
impl<F, W> std::error::Error for FaultConversionError<F, W>
where
    W: Flaws,
    F: Flaws<
        Insufficiency: core::convert::TryFrom<W::Insufficiency><
            Error: core::fmt::Debug + core::fmt::Display
        >,
        Defect: core::convert::TryFrom<W::Defect><
            Error: core::fmt::Debug + core::fmt::Display
        >,
    >,
{}

struct FailureConversionError<F, W>
where
    W: Flaws,
    F: Flaws<
        Insufficiency: core::convert::TryFrom<W::Insufficiency>,
        Defect: core::convert::TryFrom<W::Defect>;
    >,

impl<F, W> core::fmt::Debug for FailureConversionError<F, W>
where
    W: Flaws,
    F: Flaws<
        Insufficiency: core::convert::TryFrom<W::Insufficiency><
            Error: core::fmt::Debug,
        >,
        Defect: core::convert::TryFrom<W::Defect><
            Error: core::fmt::Debug,
        >,
    >,
{}

impl<F, W> core::fmt::Display for FailureConversionError<F, W>
where
    W: Flaws,
    F: Flaws<
        Insufficiency: core::convert::TryFrom<W::Insufficiency><
            Error: core::fmt::Display,
        >,
        Defect: core::convert::TryFrom<W::Defect><
            Error: core::fmt::Display,
        >,
    >,
{}

#[cfg(feature = "std")]
impl<F, W> std::error::Error for FailureConversionError<F, W>
where
    W: Flaws,
    F: Flaws<
        Insufficiency: core::convert::TryFrom<W::Insufficiency><
            Error: core::fmt::Debug + core::fmt::Display,
        >,
        Defect: core::convert::TryFrom<W::Defect>,
            Error: core::fmt::Debug + core::fmt::Display,
        >,
    >,
{}

struct RecallConversionError<F, W, G>
where
    W: Flaws,
    F: Flaws<
        Insufficiency: TryFrom<W::Insufficiency>,
        Defect: TryFrom<W::Defect>,
    >;

impl<F, W, G> core::fmt::Debug for RecallConversionError<F, W, G>
where
    W: Flaws,
    F: Flaws<
        Insufficiency: core::convert::TryFrom<W::Insufficiency><
            Error: core::fmt::Debug,
        >,
        Defect: TryFrom<W::Defect><
            Error: core::fmt::Debug,
        >,
    >,
    G: core::fmt::Debug,
{}

impl<F, W, G> core::fmt::Display for RecallConversionError<F, W, G>
where
    W: Flaws,
    F: Flaws<
        Insufficiency: core::convert::TryFrom<W::Insufficiency><
            Error: core::fmt::Display,
        >,
        Defect: TryFrom<W::Defect><
            Error: core::fmt::Display,
        >,
    >,
    G: core::fmt::Display,
{}

#[cfg(feature = "std")]
impl<F, W, G> std::error::Error for RecallConversionError<F, W, G>
where
    W: Flaws,
    F: Flaws<
        Insufficiency: core::convert::TryFrom<W::Insufficiency><
            Error: core::fmt::Debug + core::fmt::Display,
        >,
        Defect: TryFrom<W::Defect><
            Error: core::fmt::Debug + core::fmt::Display,
        >,
    >,
    G: core::fmt::Debug + core::fmt::Display,
{}

#[non_exhaustive]
enum Fault<F>
where
    F: Flaws,
{
    Defect(F::Defect),
    Insufficiency(F::Insufficiency),
}

impl<F> Fault<F>
where
    F: Flaws,
{
    fn is_defect(&self) -> bool;
}

impl<F, W> Blame<Fault<W>> for Fault<F>
where
    F: Flaws,
    W: Flaws<
        Insufficiency: core::convert::From<F::Insufficiency>,
        Defect: core::convert::From<F::Defect>,
    >,
{}

impl<F> core::clone::Clone for Fault<F>
where
    F: Flaws<
        Insufficiency: core::clone::Clone,
        Defect: core::clone::Clone,
    >,
{}

impl<F> core::marker::Copy for Fault<F>
where
    F: Flaws<
        Insufficiency: core::marker::Copy,
        Defect: core::marker::Copy,
    >,
{}

impl<F> core::fmt::Debug for Fault<F>
where
    F: Flaws<
        Insufficiency: core::fmt::Debug,
        Defect: core::fmt::Debug,
    >,
{}

impl<F> core::fmt::Display for Fault<F>
where
    F: Flaws<
        Insufficiency: core::fmt::Display,
        Defect: core::fmt::Display,
    >,
{}

impl<F> core::cmp::PartialEq for Fault<F>
where
    F: Flaws<
        Insufficiency: core::cmp::PartialEq,
        Defect: core::cmp::PartialEq,
    >,
{}

impl<F, W> TryBlame<Fault<W>> for Fault<F>
where
    F: Flaws,
    W: Flaws<
        Insufficiency: core::convert::TryFrom<F::Insufficiency>,
        Defect: core::convert::TryFrom<F::Defect>,
    >,
{
    type Error = FaultConversionError<W, F>;
}

struct Failure<F>
where
    F: Flaws;

impl<F> Failure<F>
where
    F: Flaws,
{
    fn is_defect(&self) -> bool;
}

impl<F, W> Blame<Failure<W>> for Failure<F>
where
    F: Flaws,
    W: Flaws<
        Insufficiency: core::convert::From<F::Insufficiency>,
        Defect: core::convert::From<F::Defect>,
    >,
{}

impl<F> core::fmt::Debug for Failure<F>
where
    F: Flaws<
        Insufficiency: core::fmt::Debug,
        Defect: core::fmt::Debug,
    >,
{}

impl<F> core::fmt::Display for Failure<F>
where
    F: Flaws<
        Insufficiency: core::fmt::Display,
        Defect: core::fmt::Display,
    >,
{}

#[cfg(feature = "std")]
impl<F> std::error::Error for Failure<F>
where
    F: Flaws<
        Insufficiency: core::fmt::Debug + core::fmt::Display,
        Defect: core::fmt::Debug + core::fmt::Display,
    >,
{}

impl<F> core::cmp::PartialEq for Failure<F>
where
    F: Flaws<
        Insufficiency: core::cmp::PartialEq,
        Defect: core::cmp::PartialEq,
    >,
{}

impl<F, W> TryBlame<Failure<W>> for Failure<F>
where
    F: Flaws,
    W: Flaws<
        Insufficiency: core::convert::TryFrom<F::Insufficiency>,
        Defect: core::convert::TryFrom<F::Defect>,
    >,
{
    type Error = FailureConversionError<W, F>;
}

struct Recall<F, G>
where
    F: Flaws;

impl<F, G, W, T> Blame<Recall<W, T>> for Recall<F, G>
where
    F: Flaws,
    W: Flaws<
        Insufficiency: core::convert::From<F::Insufficiency>,
        Defect: core::convert::From<F::Defect>,
    >,
    T: core::convert::From<G>,
{}

impl<F, G> core::fmt::Debug for Recall<F, G>
where
    F: Flaws<
        Insufficiency: core::fmt::Debug,
        Defect: core::fmt::Debug,
    >,
    G: core::fmt::Debug,
{}

impl<F, G> core::fmt::Display for Recall<F, G>
where
    F: Flaws<
        Insufficiency: core::fmt::Display,
        Defect: core::fmt::Display,
    >,
    G: core::fmt::Display,
{}

#[cfg(feature = "std")]
impl<F, G> std::error::Error for Recall<F, G>
where
    F: Flaws<
        Insufficiency: core::fmt::Debug + core::fmt::Display,
        Defect: core::fmt::Debug + core::fmt::Display,
    >,
    G: core::fmt::Debug + core::fmt::Display,
{}

impl<F, G> core::cmp::PartialEq for Recall<F, G>
where
    F: Flaws<
        Insufficiency: core::cmp::PartialEq,
        Defect: core::cmp::PartialEq,
    >,
    G: core::cmp::PartialEq,
{}

impl<F, G, W, T> TryBlame<Recall<W, T>> for Recall<F, G>
where
    F: Flaws,
    W: Flaws<
        Insufficiency: core::convert::From<F::Insufficiency>,
        Defect: core::convert::From<F::Defect>,
    >,
    T: core::convert::From<G>,
{
    type Error = RecallConversionError<W, F, G>;
}

// Provides the same functionality as core::iter::Once with the additional implementation of LoneGoodIter<G>: Blame<LoneGoodIter<D> where D: From<G>.
struct LoneGoodIter<G>;

impl<D, G> Blame<LoneGoodIter<D>> for LoneGoodIter<G>
where
    D: From<G>,
{}

impl<G> core::clone::Clone for LoneGoodIter<G>
where
    G: core::clone::Clone,
{}

impl<G> core::fmt::Debug for LoneGoodIter<G>
where
    G: core::fmt::Debug,
{}

impl<G> core::convert::From<G> for LoneGoodIter<G> {}

impl<G> core::iter::Iterator for LoneGoodIter<G> {
    type Item = G;
}

type Flawless = Never;

impl Flaws for Flawless {
    type Insufficiency = Self;
    type Defect = Self;
}

impl TryFrom<EmptyStock> for Flawless {
    type Error = ();
}

impl TryFrom<FullStock> for Flawless {
    type Error = ();
}

struct EmptyStock;

impl core::clone::Clone for EmptyStock {}

impl core::marker::Copy for EmptyStock {}

impl core::fmt::Debug for EmptyStock {}

impl core::default::Default for EmptyStock {}

impl core::fmt::Display for EmptyStock {}

impl Flaws for EmptyStock {
    type Insufficiency = Self;
    type Defect = Flawless;
}

impl core::ops::PartialEq for EmptyStock {}

struct FullStock;

impl core::clone::Clone for FullStock {}

impl core::marker::Copy for FullStock {}

impl core::fmt::Debug for FullStock {}

impl core::default::Default for FullStock {}

impl core::fmt::Display for FullStock {}

impl Flaws for FullStock {
    type Insufficiency = Self;
    type Defect = Flawless;
}

impl core::ops::PartialEq for FullStock {}

struct ProductionFlaws<D>;

impl<D> core::fmt::Debug for ProductionFlaws<D> {}

impl<D> Flaws for ProductionFlaws<D> {
    type Insufficiency = FullStock;
    type Defect = D;
}

struct ConsumptionFlaws<D>;

impl<D> core::fmt::Debug for ConsumptionFlaws<D> {}

impl<D> Flaws for ConsumptionFlaws<D> {
    type Insufficiency = EmptyStock;
    type Defect = D;
}

trait Agent {
    type Good;

    fn failure<F>(&self, fault: Fault<F>) -> Failure<F>
    where
        F: Flaws;
    
    fn name(&self) -> alloc::string::String;
}

trait Producer: Agent {
    type Flaws: Flaws;

    fn force(&self, good: Self::Good) -> core::result::Result<(), Recall<<Self::Flaws as Flaws>::Defect, LoneGoodIter<Self::Good>>>
    where
        <Self::Flaws as Flaws>::Defect: Flaws<
            Insufficiency: TryFrom<<Self::Flaws as Flaws>::Insufficiency,
            Defect = <Self::Flaws as Flaws>::Defect,
        >;


    fn force_all<N>(&self, goods: N) -> core::result::Result<(), Recall<<Self::Flaws as Flaws>::Defect, Chain<LoneGoodIter<Self::Good>, N::IntoIter>>>
    where
        // Required for Producer to be object safe: See https://doc.rust-lang.org/reference/items/traits.html#object-safety.
        Self: Sized,
        N: IntoIterator<Item = Self::Good>,
        <Self::Flaws as Flaws>::Defect: Flaws<
            Insufficiency: TryFrom<<Self::Flaws as Flaws>::Insufficiency,
            Defect = <Self::Flaws as Flaws>::Defect,
        >;

    fn produce(&self, good: Self::Good) -> core::result::Result<(), Recall<Self::Flaws, LoneGoodIter<Self::Good>>>;

    fn produce_all<N>(&self, goods: N) -> core::result::Result<(), Recall<Self::Flaws, Chain<LoneGoodIter<Self::Good>, N::IntoIter>>>
    where
        // Required for Producer to be object safe: See https://doc.rust-lang.org/reference/items/traits.html#object-safety.
        Self: Sized,
        N: IntoIterator<Item = Self::Good>;

    fn recall_with_lone_good(&self, fault: Fault<Self::Flaws>, good: Self::Good) -> Recall<Self::Flaws, LoneGoodIter<Self::Good>>;
}

trait Consumer: Agent {
    type Flaws: Flaws;

    fn consume(&self) -> core::result::Result<Self::Good, Failure<Self::Flaws>>;

    fn demand(&self) -> core::result::Result<Self::Good, Failure<<Self::Flaws as Flaws>::Defect>>
    where
        <Self::Flaws as Flaws>::Defect: Flaws<
            Insufficiency: TryFrom<<Self::Flaws as Flaws>::Insufficiency,
            Defect = <Self::Flaws as Flaws>::Defect,
        >;

    fn goods(&self) -> Goods<'_, Self>
    where
        Self: Sized;
}
