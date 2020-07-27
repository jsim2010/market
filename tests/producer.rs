use {
    core::sync::atomic::{AtomicBool, Ordering},
    fehler::{throw, throws},
    market::{ProduceFailure, Producer, Recall},
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
    type Error = MockError;

    #[throws(ProduceFailure<Self::Error>)]
    fn produce(&self, _good: Self::Good) {
        if self.will_fail {
            throw!(ProduceFailure::Error(MockError));
        } else if self.is_full.load(Ordering::Relaxed) {
            self.is_full.store(false, Ordering::Relaxed);
            throw!(ProduceFailure::FullStock);
        }
    }
}

#[derive(Debug, PartialEq)]
struct MockError;

impl Display for MockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MockError")
    }
}

impl Error for MockError {}

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
        Err(Recall::new(GOOD, ProduceFailure::FullStock))
    );
}

/// If `produce` succeeds, `force` also succeeds.
#[test]
fn force_succeeds() {
    let producer = MockProducer::new();

    assert_eq!(producer.force(1), Ok(()));
}

/// If `produce` throws `ProduceFailure::FullStock`, `force` calls `produce` again.
#[test]
fn force_blocks_until_success() {
    let producer = MockProducer::new().mock_full();

    assert_eq!(producer.force(1), Ok(()));
}

/// If `produce` throws `{E}` of type `ProduceFailure::Error`, `force` throws `{E}`.
#[test]
fn force_fails() {
    const GOOD: u8 = 3;
    let producer = MockProducer::new().mock_failure();

    assert_eq!(producer.force(GOOD), Err(MockError));
}
