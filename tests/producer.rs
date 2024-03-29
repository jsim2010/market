use {
    core::{
        cell::RefCell,
        fmt::{self, Debug, Display, Formatter},
    },
    fehler::{throw, throws},
    market::*,
    never::Never,
    std::{
        collections::VecDeque,
        sync::atomic::{AtomicU8, Ordering},
    },
};

struct U8Consumer {
    goods: RefCell<VecDeque<u8>>,
}

impl Agent for U8Consumer {
    type Good = u8;
}

impl Consumer for U8Consumer {
    type Flaws = ConsumptionFlaws<Never>;

    #[throws(Failure<Self::Flaws>)]
    fn consume(&self) -> Self::Good {
        self.goods
            .borrow_mut()
            .pop_front()
            .ok_or(self.failure(Fault::Insufficiency(EmptyStock::default())))?
    }
}

impl Display for U8Consumer {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "U8Consumer")
    }
}

impl From<Vec<u8>> for U8Consumer {
    fn from(goods: Vec<u8>) -> Self {
        Self {
            goods: RefCell::new(goods.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MockDefect;

impl Flaws for MockDefect {
    type Insufficiency = Never;
    type Defect = Self;
}

#[derive(Default)]
struct U8Producer {
    goods: RefCell<Vec<u8>>,
    fail_on_call: Option<(u8, Fault<ProductionFlaws<MockDefect>>)>,
    calls: AtomicU8,
}

impl U8Producer {
    fn fail_on_produce_call(&mut self, call: u8, fault: Fault<ProductionFlaws<MockDefect>>) {
        self.fail_on_call = Some((call, fault));
    }
}

impl Agent for U8Producer {
    type Good = u8;
}

impl Display for U8Producer {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "U8Producer")
    }
}

impl Producer for U8Producer {
    type Flaws = ProductionFlaws<MockDefect>;

    #[throws(Recall<Self::Flaws, Self::Good>)]
    fn produce(&self, good: Self::Good) {
        if let Some((call, fault)) = self.fail_on_call {
            if call == self.calls.fetch_add(1, Ordering::Relaxed) {
                throw!(self.recall(fault, good));
            }
        }

        self.goods.borrow_mut().push(good);
    }
}

#[test]
fn produce_goods_success() {
    let producer = U8Producer::default();
    let goods = U8Consumer::from(vec![0, 1, 2]);

    assert_eq!(producer.produce_goods(&goods).unwrap(), ());
    assert_eq!(producer.goods, RefCell::new(vec![0, 1, 2]))
}

#[test]
fn produce_goods_insufficient_stock() {
    let mut producer = U8Producer::default();
    let goods = U8Consumer::from(vec![0, 1, 2]);

    producer.fail_on_produce_call(0, Fault::Insufficiency(FullStock::default()));

    assert_eq!(
        producer.produce_goods(&goods).unwrap_err(),
        Blockage::Production(producer.recall(Fault::Insufficiency(FullStock::default()), 0))
    );
    assert_eq!(producer.goods, RefCell::new(vec![]));
}

#[test]
fn produce_goods_fault() {
    let mut producer = U8Producer::default();
    let goods = U8Consumer::from(vec![0, 1, 2]);
    let fault = Fault::Defect(MockDefect);

    producer.fail_on_produce_call(0, fault.clone());

    assert_eq!(
        producer.produce_goods(&goods).unwrap_err(),
        Blockage::Production(producer.recall(fault, 0))
    );
    assert_eq!(producer.goods, RefCell::new(vec![]));
}

#[test]
fn produce_goods_fault_middle() {
    let mut producer = U8Producer::default();
    let goods = U8Consumer::from(vec![0, 1, 2]);
    let fault = Fault::Defect(MockDefect);

    producer.fail_on_produce_call(1, fault.clone());

    assert_eq!(
        producer.produce_goods(&goods).unwrap_err(),
        Blockage::Production(producer.recall(fault, 1))
    );
    assert_eq!(producer.goods, RefCell::new(vec![0]));
}

#[test]
fn force_success() {
    let producer = U8Producer::default();

    assert_eq!(producer.force(0).unwrap(), ());
    assert_eq!(producer.goods, RefCell::new(vec![0]));
}

#[test]
fn force_insufficient_stock() {
    let mut producer = U8Producer::default();

    producer.fail_on_produce_call(0, Fault::Insufficiency(FullStock::default()));

    assert_eq!(producer.force(0).unwrap(), ());
    assert_eq!(producer.goods, RefCell::new(vec![0]));
}

#[test]
fn force_fault() {
    let mut producer = U8Producer::default();
    let fault = Fault::Defect(MockDefect);

    producer.fail_on_produce_call(0, fault.clone());

    assert_eq!(
        producer.force(0).unwrap_err(),
        producer.recall(fault, 0).try_blame().unwrap()
    );
}

#[test]
fn force_goods_fault() {
    let mut producer = U8Producer::default();
    let goods = U8Consumer::from(vec![0, 1, 2]);
    let fault = Fault::Defect(MockDefect);

    producer.fail_on_produce_call(0, fault.clone());

    assert_eq!(
        producer.force_goods(&goods).unwrap_err(),
        Blockage::Production(producer.recall(fault, 0).try_blame().unwrap())
    );
    assert_eq!(producer.goods, RefCell::new(vec![]));
}

#[test]
fn force_goods_fault_middle() {
    let mut producer = U8Producer::default();
    let goods = U8Consumer::from(vec![0, 1, 2]);
    let fault = Fault::Defect(MockDefect);

    producer.fail_on_produce_call(1, fault.clone());

    assert_eq!(
        producer.force_goods(&goods).unwrap_err(),
        Blockage::Production(producer.recall(fault, 1).try_blame().unwrap())
    );
    assert_eq!(producer.goods, RefCell::new(vec![0]));
}
