use {
    core::{cell::RefCell, fmt::Debug},
    fehler::{throw, throws},
    market::*,
    std::{
        sync::atomic::{AtomicU8, Ordering},
        vec::IntoIter,
    },
};

#[derive(Clone, Debug, PartialEq, ProduceFault)]
struct MockFault;

#[derive(Default)]
struct MockProducer {
    goods: RefCell<Vec<u8>>,
    fail_on_call: Option<u8>,
    calls: AtomicU8,
    failure: ProduceFailure<MockFault>,
}

impl MockProducer {
    fn fail_on_produce_call(&mut self, call: u8, failure: ProduceFailure<MockFault>) {
        self.fail_on_call = Some(call);
        self.failure = failure;
    }
}

impl Producer for MockProducer {
    type Good = u8;
    type Failure = ProduceFailure<MockFault>;

    #[throws(Return<Self::Good, Self::Failure>)]
    fn produce(&self, good: Self::Good) {
        if let Some(call) = self.fail_on_call {
            if call == self.calls.fetch_add(1, Ordering::Relaxed) {
                throw!(Return::new(good, self.failure.clone()));
            }
        }

        self.goods.borrow_mut().push(good);
    }
}

fn cmp_recall<E: Debug + PartialEq>(recall: Recall<u8, IntoIter<u8>, E>, goods: Vec<u8>, error: E) {
    let (mut good_iter, err) = recall.redeem();

    assert_eq!(err, error);

    for good in goods {
        assert_eq!(good_iter.next(), Some(good));
    }
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

    producer.fail_on_produce_call(0, ProduceFailure::FullStock);

    cmp_recall(
        producer.produce_all(goods).unwrap_err(),
        vec![0, 1, 2],
        ProduceFailure::FullStock,
    );
    assert_eq!(producer.goods, RefCell::new(vec![]));
}

#[test]
fn produce_all_fault() {
    let mut producer = MockProducer::default();
    let goods = vec![0, 1, 2];

    producer.fail_on_produce_call(0, ProduceFailure::Fault(MockFault));

    cmp_recall(
        producer.produce_all(goods).unwrap_err(),
        vec![0, 1, 2],
        ProduceFailure::Fault(MockFault),
    );
    assert_eq!(producer.goods, RefCell::new(vec![]));
}

#[test]
fn produce_all_fault_middle() {
    let mut producer = MockProducer::default();
    let goods = vec![0, 1, 2];

    producer.fail_on_produce_call(1, ProduceFailure::Fault(MockFault));

    cmp_recall(
        producer.produce_all(goods).unwrap_err(),
        vec![1, 2],
        ProduceFailure::Fault(MockFault),
    );
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

    producer.fail_on_produce_call(0, ProduceFailure::FullStock);

    assert_eq!(producer.force(0).unwrap(), ());
    assert_eq!(producer.goods, RefCell::new(vec![0]));
}

#[test]
fn force_fault() {
    let mut producer = MockProducer::default();

    producer.fail_on_produce_call(0, ProduceFailure::Fault(MockFault));
    let (good, fault) = producer.force(0).unwrap_err().redeem();

    assert_eq!(good, 0);
    assert_eq!(fault, MockFault);
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

    producer.fail_on_produce_call(0, ProduceFailure::FullStock);

    assert_eq!(producer.force_all(goods).unwrap(), ());
    assert_eq!(producer.goods, RefCell::new(vec![0, 1, 2]));
}

#[test]
fn force_all_insufficient_stock_middle() {
    let mut producer = MockProducer::default();
    let goods = vec![0, 1, 2];

    producer.fail_on_produce_call(1, ProduceFailure::FullStock);

    assert_eq!(producer.force_all(goods).unwrap(), ());
    assert_eq!(producer.goods, RefCell::new(vec![0, 1, 2]));
}

#[test]
fn force_all_fault() {
    let mut producer = MockProducer::default();
    let goods = vec![0, 1, 2];

    producer.fail_on_produce_call(0, ProduceFailure::Fault(MockFault));

    cmp_recall(
        producer.force_all(goods).unwrap_err(),
        vec![0, 1, 2],
        MockFault,
    );
    assert_eq!(producer.goods, RefCell::new(vec![]));
}

#[test]
fn force_all_fault_middle() {
    let mut producer = MockProducer::default();
    let goods = vec![0, 1, 2];

    producer.fail_on_produce_call(1, ProduceFailure::Fault(MockFault));

    cmp_recall(
        producer.force_all(goods).unwrap_err(),
        vec![1, 2],
        MockFault,
    );
    assert_eq!(producer.goods, RefCell::new(vec![0]));
}
