mod withdrawn_demand_fault {
    use market::channel::WithdrawnDemandFault;

    #[test]
    fn error_source() {
        use std::error::Error;

        assert!(matches!(
            WithdrawnDemandFault::new(String::new()).source(),
            None
        ));
    }

    #[test]
    fn new_and_display() {
        assert_eq!(
            format!("{}", WithdrawnDemandFault::new("channel".to_string())),
            "demand of `channel` has withdrawn"
        );
    }

    #[test]
    fn try_from() {
        use {market::ProduceFailure, std::convert::TryFrom};

        assert!(
            WithdrawnDemandFault::try_from(ProduceFailure::Fault(WithdrawnDemandFault::new(
                String::new()
            )))
            .is_ok()
        );
    }
}

mod withdrawn_supply_fault {
    use market::channel::WithdrawnSupplyFault;

    #[test]
    fn error_source() {
        use std::error::Error;

        assert!(matches!(
            WithdrawnSupplyFault::new(String::new()).source(),
            None
        ));
    }

    #[test]
    fn new_and_display() {
        assert_eq!(
            format!("{}", WithdrawnSupplyFault::new("channel".to_string())),
            "supply of `channel` has withdrawn"
        );
    }

    #[test]
    fn try_from() {
        use {market::ConsumeFailure, std::convert::TryFrom};

        assert!(
            WithdrawnSupplyFault::try_from(ConsumeFailure::Fault(WithdrawnSupplyFault::new(
                String::new()
            )))
            .is_ok()
        );
    }
}

mod create {
    use {
        core::convert::TryFrom,
        market::{
            channel::{create, Size, Style},
            Consumer, Failure, Producer,
        },
    };

    struct MockFailure;

    impl Failure for MockFailure {
        type Fault = ();
    }

    impl TryFrom<MockFailure> for () {
        type Error = ();

        fn try_from(_failure: MockFailure) -> Result<Self, Self::Error> {
            Ok(())
        }
    }

    #[derive(Debug, PartialEq)]
    struct MockProducer {
        size: Size,
    }

    impl Producer for MockProducer {
        type Good = ();
        type Failure = MockFailure;

        fn produce(&self, _good: Self::Good) -> Result<(), Self::Failure> {
            Ok(())
        }
    }

    #[derive(Debug, PartialEq)]
    struct MockConsumer {
        size: Size,
    }

    impl Consumer for MockConsumer {
        type Good = ();
        type Failure = MockFailure;

        fn consume(&self) -> Result<Self::Good, Self::Failure> {
            Ok(())
        }
    }
    struct MockStyle;

    impl Style for MockStyle {
        type Producer = MockProducer;
        type Consumer = MockConsumer;

        fn infinite(_description: String) -> (Self::Producer, Self::Consumer) {
            (
                MockProducer {
                    size: Size::Infinite,
                },
                MockConsumer {
                    size: Size::Infinite,
                },
            )
        }

        fn finite(_description: String, size: usize) -> (Self::Producer, Self::Consumer) {
            (
                MockProducer {
                    size: Size::Finite(size),
                },
                MockConsumer {
                    size: Size::Finite(size),
                },
            )
        }
    }

    #[test]
    fn infinite() {
        assert_eq!(
            create::<MockStyle>(String::new(), Size::Infinite),
            (
                MockProducer {
                    size: Size::Infinite
                },
                MockConsumer {
                    size: Size::Infinite
                }
            )
        );
    }

    #[test]
    fn finite() {
        assert_eq!(
            create::<MockStyle>(String::new(), Size::Finite(1)),
            (
                MockProducer {
                    size: Size::Finite(1)
                },
                MockConsumer {
                    size: Size::Finite(1)
                }
            )
        );
    }
}
