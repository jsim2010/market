use {
    core::{cell::RefCell, fmt::Debug, iter::Once},
    fehler::{throw, throws},
    market::*,
    never::Never,
    std::sync::atomic::{AtomicU8, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq)]
struct MockDefect;

impl Flaws for MockDefect {
    type Insufficiency = Never;
    type Defect = Self;
}

#[derive(Default)]
struct MockProducer {
    goods: RefCell<Vec<u8>>,
    fail_on_call: Option<(u8, Fault<ProductionFlaws<MockDefect>>)>,
    calls: AtomicU8,
}

impl MockProducer {
    fn fail_on_produce_call(&mut self, call: u8, fault: Fault<ProductionFlaws<MockDefect>>) {
        self.fail_on_call = Some((call, fault));
    }
}

impl Agent for MockProducer {
    type Good = u8;

    fn name(&self) -> String {
        String::from("MockProducer")
    }
}

impl Producer for MockProducer {
    type Flaws = ProductionFlaws<MockDefect>;

    #[throws(Recall<Self::Flaws, Once<Self::Good>>)]
    fn produce(&self, good: Self::Good) {
        if let Some((call, fault)) = self.fail_on_call {
            if call == self.calls.fetch_add(1, Ordering::Relaxed) {
                throw!(self.lone_recall(fault, good));
            }
        }

        self.goods.borrow_mut().push(good);
    }
}

// TODO: Figure out way to test Recall.
fn cmp_recall<F: Flaws, I: Iterator<Item = u8> + Clone + Debug>(
    recall: Recall<F, I>,
    goods: Vec<u8>,
    fault: Fault<F>,
) where
    F::Insufficiency: Debug + PartialEq,
    F::Defect: Debug + PartialEq,
{
    drop(recall);
    drop(goods);
    drop(fault);
    //assert_eq!(
    //    recall,
    //    Recall::new(Failure::new(fault, String::new()), goods)
    //)
}

#[test]
fn produce_all_success() {
    let producer = MockProducer::default();
    let goods = vec![0, 1, 2];

    assert_eq!(producer.produce_all(goods).unwrap(), ());
    assert_eq!(producer.goods, RefCell::new(vec![0, 1, 2]))
}

#[test]
fn produce_all_insufficient_stock() {
    let mut producer = MockProducer::default();
    let goods = vec![0, 1, 2];

    producer.fail_on_produce_call(0, Fault::Insufficiency(FullStock::default()));

    cmp_recall(
        producer.produce_all(goods).unwrap_err(),
        vec![0, 1, 2],
        Fault::Insufficiency(FullStock::default()),
    );
    assert_eq!(producer.goods, RefCell::new(vec![]));
}

#[test]
fn produce_all_fault() {
    let mut producer = MockProducer::default();
    let goods = vec![0, 1, 2];
    let fault = Fault::Defect(MockDefect);

    producer.fail_on_produce_call(0, fault.clone());

    cmp_recall(
        producer.produce_all(goods).unwrap_err(),
        vec![0, 1, 2],
        fault,
    );
    assert_eq!(producer.goods, RefCell::new(vec![]));
}

#[test]
fn produce_all_fault_middle() {
    let mut producer = MockProducer::default();
    let goods = vec![0, 1, 2];
    let fault = Fault::Defect(MockDefect);

    producer.fail_on_produce_call(1, fault.clone());

    cmp_recall(producer.produce_all(goods).unwrap_err(), vec![1, 2], fault);
    assert_eq!(producer.goods, RefCell::new(vec![0]));
}

#[test]
fn force_success() {
    let producer = MockProducer::default();

    assert_eq!(producer.force(0).unwrap(), ());
    assert_eq!(producer.goods, RefCell::new(vec![0]));
}

#[test]
fn force_insufficient_stock() {
    let mut producer = MockProducer::default();

    producer.fail_on_produce_call(0, Fault::Insufficiency(FullStock::default()));

    assert_eq!(producer.force(0).unwrap(), ());
    assert_eq!(producer.goods, RefCell::new(vec![0]));
}

// TODO: Figure out way to test Recall.
#[test]
fn force_fault() {
    //let mut producer = MockProducer::default();

    //producer.fail_on_produce_call(0, Fault::Defect(MockDefect));

    //assert_eq!(
    //    producer.force(0).unwrap_err(),
    //    Recall::new(Failure::new(Fault::Defect(MockDefect), String::new()), iter::once(0))
    //);
}

#[test]
fn force_all_success() {
    let producer = MockProducer::default();
    let goods = vec![0, 1, 2];

    assert_eq!(producer.force_all(goods).unwrap(), ());
    assert_eq!(producer.goods, RefCell::new(vec![0, 1, 2]));
}

#[test]
fn force_all_insufficient_stock() {
    let mut producer = MockProducer::default();
    let goods = vec![0, 1, 2];

    producer.fail_on_produce_call(0, Fault::Insufficiency(FullStock::default()));

    assert_eq!(producer.force_all(goods).unwrap(), ());
    assert_eq!(producer.goods, RefCell::new(vec![0, 1, 2]));
}

#[test]
fn force_all_insufficient_stock_middle() {
    let mut producer = MockProducer::default();
    let goods = vec![0, 1, 2];

    producer.fail_on_produce_call(1, Fault::Insufficiency(FullStock::default()));

    assert_eq!(producer.force_all(goods).unwrap(), ());
    assert_eq!(producer.goods, RefCell::new(vec![0, 1, 2]));
}

#[test]
fn force_all_fault() {
    let mut producer = MockProducer::default();
    let goods = vec![0, 1, 2];

    producer.fail_on_produce_call(0, Fault::Defect(MockDefect));

    cmp_recall(
        producer.force_all(goods).unwrap_err(),
        vec![0, 1, 2],
        Fault::Defect(MockDefect),
    );
    assert_eq!(producer.goods, RefCell::new(vec![]));
}

#[test]
fn force_all_fault_middle() {
    let mut producer = MockProducer::default();
    let goods = vec![0, 1, 2];

    producer.fail_on_produce_call(1, Fault::Defect(MockDefect));

    cmp_recall(
        producer.force_all(goods).unwrap_err(),
        vec![1, 2],
        Fault::Defect(MockDefect),
    );
    assert_eq!(producer.goods, RefCell::new(vec![0]));
}
