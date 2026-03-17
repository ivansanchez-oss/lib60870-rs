use std::future::Future;

use super::Asdu;

/// Handler for receiving fully parsed ASDUs.
pub trait AsduHandler: Send {
    fn handle_asdu(&mut self, asdu: &Asdu) -> impl Future<Output = ()> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asdu::{AddressedObject, AsduHeader, InformationObjectAddress};
    use crate::info::{InformationObject, SinglePointInformation};
    use crate::types::{CauseOfTransmission, QualityDescriptor, TypeId};

    struct TestHandler {
        count: usize,
    }

    impl AsduHandler for TestHandler {
        async fn handle_asdu(&mut self, asdu: &Asdu) {
            self.count += asdu.objects.len();
        }
    }

    #[tokio::test]
    async fn handle_asdu_receives_objects() {
        let asdu = Asdu {
            header: AsduHeader {
                type_id: TypeId::MSpNa1,
                is_sequence: false,
                num_objects: 2,
                cause: CauseOfTransmission::Spontaneous,
                is_test: false,
                is_negative: false,
                originator_address: 0,
                common_address: 1,
            },
            objects: vec![
                AddressedObject {
                    address: InformationObjectAddress::new(100),
                    object: InformationObject::SinglePoint(SinglePointInformation::new(
                        true,
                        QualityDescriptor::empty(),
                    )),
                },
                AddressedObject {
                    address: InformationObjectAddress::new(101),
                    object: InformationObject::SinglePoint(SinglePointInformation::new(
                        false,
                        QualityDescriptor::empty(),
                    )),
                },
            ],
        };

        let mut handler = TestHandler { count: 0 };
        handler.handle_asdu(&asdu).await;
        assert_eq!(handler.count, 2);
    }
}
