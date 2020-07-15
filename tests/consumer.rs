//! Tests functionality of implemented functions in [`Consumer`].
use {
    core::sync::atomic::{AtomicBool, Ordering},
    fehler::{throw, throws},
    market::{ConsumeError, Consumer},
    std::{
        error::Error,
        fmt::{self, Display},
    },
};

struct MockConsumer {
    good: u8,
    is_empty: AtomicBool,
    shall_fail: bool,
}

impl MockConsumer {
    fn new(good: u8) -> Self {
        Self {
            good,
            is_empty: AtomicBool::new(false),
            shall_fail: false,
        }
    }

    fn mock_empty_once(self) -> Self {
        self.is_empty.store(true, Ordering::Relaxed);
        self
    }

    fn mock_failure(mut self) -> Self {
        self.shall_fail = true;
        self
    }
}

impl Consumer for MockConsumer {
    type Good = u8;
    type Failure = MockFailure;

    #[throws(ConsumeError<Self::Failure>)]
    fn consume(&self) -> Self::Good {
        if self.shall_fail {
            throw!(ConsumeError::Failure(MockFailure));
        } else if self.is_empty.load(Ordering::Relaxed) {
            self.is_empty.store(false, Ordering::Relaxed);
            throw!(ConsumeError::EmptyStock);
        } else {
            self.good
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

/// If `consume` returns a good, `demand` returns it.
#[test]
fn demand_returns_current_good() {
    const GOOD: u8 = 1;
    let consumer = MockConsumer::new(GOOD);

    assert_eq!(consumer.demand(), Ok(GOOD));
}

/// If `consume` returns `ConsumeError::EmptyStock`, `demand` shall call `consume` again.
#[test]
fn demand_blocks_until_good_is_found() {
    const GOOD: u8 = 2;
    let consumer = MockConsumer::new(GOOD).mock_empty_once();

    assert_eq!(consumer.demand(), Ok(GOOD));
}

/// If `consume` returns `ConsumeError::Failure({F})`, `demand` shall return `{F}`.
#[test]
fn demand_returns_failure() {
    let consumer = MockConsumer::new(1).mock_failure();

    assert_eq!(consumer.demand(), Err(MockFailure));
}
