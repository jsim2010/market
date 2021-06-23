use {core::cell::RefCell, fehler::throws, market::*, never::Never, std::collections::VecDeque};

struct MockConsumer {
    goods: RefCell<VecDeque<Result<u8, Fault<ConsumptionFlaws<MockDefect>>>>>,
}

impl MockConsumer {
    fn new(goods: Vec<Result<u8, Fault<ConsumptionFlaws<MockDefect>>>>) -> Self {
        Self {
            goods: RefCell::new(goods.into()),
        }
    }
}

impl Agent for MockConsumer {
    type Good = u8;

    fn name(&self) -> String {
        String::from("MockConsumer")
    }
}

impl Consumer for MockConsumer {
    type Flaws = ConsumptionFlaws<MockDefect>;

    #[throws(Failure<Self::Flaws>)]
    fn consume(&self) -> Self::Good {
        self.goods
            .borrow_mut()
            .pop_front()
            .map(|x| x.map_err(|fault| self.failure(fault)))
            .ok_or(self.failure(Fault::Insufficiency(EmptyStock::default())))??
    }
}

#[derive(Debug, PartialEq)]
struct MockMisstep;

#[derive(Clone, Debug, PartialEq)]
struct MockDefect;

impl Flaws for MockDefect {
    type Insufficiency = Never;
    type Defect = Self;
}

#[derive(Debug, PartialEq)]
struct MockComposeError;

#[test]
fn demand_success() {
    let consumer = MockConsumer::new(vec![Ok(0)]);

    assert_eq!(consumer.demand(), Ok(0));
}

#[test]
fn demand_insufficient_stock() {
    let consumer = MockConsumer::new(vec![
        Err(Fault::Insufficiency(EmptyStock::default())),
        Ok(0),
    ]);

    assert_eq!(consumer.demand(), Ok(0));
}

// TODO: Figure out way to test the failure.
#[test]
fn demand_fault() {
    //let consumer = MockConsumer::new(vec![Err(Fault::Defect(MockDefect)), Ok(0)]);

    //assert_eq!(
    //    consumer.demand(),
    //    Err(consumer.failure(Fault::Defect(MockDefect)))
    //);
    //assert_eq!(consumer.demand(), Ok(0));
}
