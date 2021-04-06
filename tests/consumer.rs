use {
    core::cell::RefCell,
    fehler::{throw, throws},
    market::*,
    std::{
        collections::VecDeque,
        sync::atomic::{AtomicU8, Ordering},
        task::Poll,
    },
};

struct MockConsumer {
    goods: RefCell<VecDeque<u8>>,
    fail_on_call: Option<u8>,
    calls: AtomicU8,
    failure: ConsumeFailure<MockFault>,
}

impl MockConsumer {
    fn new(goods: Vec<u8>) -> Self {
        Self {
            goods: RefCell::new(goods.into()),
            fail_on_call: None,
            calls: AtomicU8::new(0),
            failure: ConsumeFailure::EmptyStock,
        }
    }

    fn fail_on_consume_call(&mut self, call: u8, failure: ConsumeFailure<MockFault>) {
        self.fail_on_call = Some(call);
        self.failure = failure;
    }
}

impl Consumer for MockConsumer {
    type Good = u8;
    type Failure = ConsumeFailure<MockFault>;

    #[throws(Self::Failure)]
    fn consume(&self) -> Self::Good {
        if let Some(call) = self.fail_on_call {
            if call == self.calls.fetch_add(1, Ordering::Relaxed) {
                throw!(self.failure.clone());
            }
        }

        self.goods
            .borrow_mut()
            .pop_front()
            .ok_or(ConsumeFailure::EmptyStock)?
    }
}

#[derive(Default)]
struct MockBuilder;

impl Builder<u8> for MockBuilder {
    type Output = MockComposite;
    type Error = MockComposeError;

    #[throws(Self::Error)]
    fn build(&self, parts: &mut Vec<u8>) -> Poll<Self::Output> {
        match parts.get(0) {
            Some(0) => match parts.get(1) {
                Some(1) => match parts.get(2) {
                    Some(2) => {
                        parts.drain(0..3);
                        Poll::Ready(MockComposite)
                    }
                    Some(_) => throw!(MockComposeError),
                    None => Poll::Pending,
                },
                Some(_) => throw!(MockComposeError),
                None => Poll::Pending,
            },
            Some(_) => throw!(MockComposeError),
            None => Poll::Pending,
        }
    }
}

#[derive(Debug, PartialEq)]
struct MockComposite;

#[derive(Clone, ConsumeFault, Debug, PartialEq)]
struct MockFault;

#[derive(Debug, PartialEq)]
struct MockComposeError;

#[test]
fn compose_success() {
    let consumer = MockConsumer::new(vec![0, 1, 2]);
    let mut composer = Composer::new(MockBuilder);

    assert_eq!(
        consumer.compose(&mut composer),
        Ok(Poll::Ready(MockComposite))
    );
    assert_eq!(consumer.compose(&mut composer), Ok(Poll::Pending));
}

#[test]
fn compose_consume_insufficient_stock() {
    let mut consumer = MockConsumer::new(vec![0, 1, 2]);
    let mut composer = Composer::new(MockBuilder);

    consumer.fail_on_consume_call(0, ConsumeFailure::EmptyStock);

    assert_eq!(consumer.compose(&mut composer), Ok(Poll::Pending));
    assert_eq!(
        consumer.compose(&mut composer),
        Ok(Poll::Ready(MockComposite))
    );
    assert_eq!(consumer.compose(&mut composer), Ok(Poll::Pending));
}

#[test]
fn compose_partial() {
    let mut consumer = MockConsumer::new(vec![0, 1, 2]);
    let mut composer = Composer::new(MockBuilder);

    consumer.fail_on_consume_call(2, ConsumeFailure::EmptyStock);

    assert_eq!(consumer.compose(&mut composer), Ok(Poll::Pending));
    assert_eq!(
        consumer.compose(&mut composer),
        Ok(Poll::Ready(MockComposite))
    );
    assert_eq!(consumer.compose(&mut composer), Ok(Poll::Pending));
}

#[test]
fn compose_consume_fault() {
    let mut consumer = MockConsumer::new(vec![0, 1, 2]);
    let mut composer = Composer::new(MockBuilder);

    consumer.fail_on_consume_call(0, ConsumeFailure::Fault(MockFault));

    assert_eq!(
        consumer.compose(&mut composer),
        Err(ComposeError::Consume(MockFault))
    );
    assert_eq!(
        consumer.compose(&mut composer),
        Ok(Poll::Ready(MockComposite))
    );
    assert_eq!(consumer.compose(&mut composer).unwrap(), Poll::Pending);
}

#[test]
fn compose_consume_fault_partial() {
    let mut consumer = MockConsumer::new(vec![0, 1, 2]);
    let mut composer = Composer::new(MockBuilder);

    consumer.fail_on_consume_call(2, ConsumeFailure::Fault(MockFault));

    assert_eq!(
        consumer.compose(&mut composer),
        Err(ComposeError::Consume(MockFault))
    );
    assert_eq!(
        consumer.compose(&mut composer),
        Ok(Poll::Ready(MockComposite))
    );
    assert_eq!(consumer.compose(&mut composer), Ok(Poll::Pending));
}

#[test]
fn compose_build_error() {
    let mut consumer = MockConsumer::new(vec![0, 1, 2]);
    let mut composer = Composer::new(MockBuilder);

    consumer.fail_on_consume_call(0, ConsumeFailure::Fault(MockFault));

    assert_eq!(
        consumer.compose(&mut composer),
        Err(ComposeError::Consume(MockFault))
    );
    assert_eq!(
        consumer.compose(&mut composer),
        Ok(Poll::Ready(MockComposite))
    );
    assert_eq!(consumer.compose(&mut composer), Ok(Poll::Pending));
}

#[test]
fn compose_build_error_partial() {
    let mut consumer = MockConsumer::new(vec![0, 1, 2]);
    let mut composer = Composer::new(MockBuilder);

    consumer.fail_on_consume_call(2, ConsumeFailure::Fault(MockFault));

    assert_eq!(
        consumer.compose(&mut composer),
        Err(ComposeError::Consume(MockFault))
    );
    assert_eq!(
        consumer.compose(&mut composer),
        Ok(Poll::Ready(MockComposite))
    );
    assert_eq!(consumer.compose(&mut composer), Ok(Poll::Pending));
}

#[test]
fn compose_multiple() {
    let consumer = MockConsumer::new(vec![0, 1, 2, 0, 1, 2]);
    let mut composer = Composer::new(MockBuilder);

    assert_eq!(
        consumer.compose(&mut composer),
        Ok(Poll::Ready(MockComposite))
    );
    assert_eq!(
        consumer.compose(&mut composer),
        Ok(Poll::Ready(MockComposite))
    );
    assert_eq!(consumer.compose(&mut composer), Ok(Poll::Pending));
}

#[test]
fn demand_success() {
    let consumer = MockConsumer::new(vec![0]);

    assert_eq!(consumer.demand(), Ok(0));
}

#[test]
fn demand_insufficient_stock() {
    let mut consumer = MockConsumer::new(vec![0]);

    consumer.fail_on_consume_call(0, ConsumeFailure::EmptyStock);
    assert_eq!(consumer.demand(), Ok(0));
}

#[test]
fn demand_fault() {
    let mut consumer = MockConsumer::new(vec![0]);

    consumer.fail_on_consume_call(0, ConsumeFailure::Fault(MockFault));
    assert_eq!(consumer.demand(), Err(MockFault));
}
