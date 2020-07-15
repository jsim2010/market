use {
    core::sync::atomic::{AtomicBool, Ordering},
    fehler::{throw, throws},
    market::{ProduceError, Producer, Recall},
    std::{
        error::Error,
        fmt::{self, Display},
    },
};

struct MockProducer {
    is_full: AtomicBool,
    will_fail: bool,
}

impl MockProducer {
    fn new() -> Self {
        Self {
            is_full: AtomicBool::new(false),
            will_fail: false,
        }
    }

    fn mock_full(self) -> Self {
        self.is_full.store(true, Ordering::Relaxed);
        self
    }

    fn mock_failure(mut self) -> Self {
        self.will_fail = true;
        self
    }
}

impl Producer for MockProducer {
    type Good = u8;
    type Failure = MockFailure;

    #[throws(ProduceError<Self::Failure>)]
    fn produce(&self, _good: Self::Good) {
        if self.will_fail {
            throw!(ProduceError::Failure(MockFailure));
        } else if self.is_full.load(Ordering::Relaxed) {
            self.is_full.store(false, Ordering::Relaxed);
            throw!(ProduceError::FullStock);
        }
    }
}

#[derive(Debug, PartialEq)]
struct MockFailure;

impl Display for MockFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MockFailure")
    }
}

impl Error for MockFailure {}

/// If `produce` succeeds, `produce_or_recall` also succeeds.
#[test]
fn produce_or_recall_succeeds() {
    let producer = MockProducer::new();

    assert_eq!(producer.produce_or_recall(1), Ok(()));
}

/// If `produce` fails, `produce_or_recall` also fails.
#[test]
fn produce_or_recall_fails() {
    const GOOD: u8 = 2;
    let producer = MockProducer::new().mock_full();

    assert_eq!(
        producer.produce_or_recall(GOOD),
        Err(Recall::new(GOOD, ProduceError::FullStock))
    );
}

/// If `produce` succeeds, `force` also succeeds.
#[test]
fn force_succeeds() {
    let producer = MockProducer::new();

    assert_eq!(producer.force(1), Ok(()));
}

/// If `produce` throws `ProduceError::FullStock`, `force` calls `produce` again.
#[test]
fn force_blocks_until_success() {
    let producer = MockProducer::new().mock_full();

    assert_eq!(producer.force(1), Ok(()));
}

/// If `produce` throws `{F}` of type `ProduceError::Failure`, `force` throws `{F}`.
#[test]
fn force_fails() {
    const GOOD: u8 = 3;
    let producer = MockProducer::new().mock_failure();

    assert_eq!(producer.force(GOOD), Err(MockFailure));
}
