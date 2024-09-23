use mqtt_codec_kit::{common::QualityOfService, v4::packet::PublishPacket};

#[derive(Debug)]
pub struct Message {
    topic: String,
    payload: Vec<u8>,
    qos: QualityOfService,
    packet_id: Option<u16>,
    dup: bool,
    retain: bool,
}

impl From<&PublishPacket> for Message {
    fn from(packet: &PublishPacket) -> Self {
        let (qos, packet_id) = packet.qos().split();
        Self {
            topic: String::from(packet.topic_name().clone()),
            payload: packet.payload().to_vec(),
            qos,
            packet_id,
            dup: packet.dup(),
            retain: packet.retain(),
        }
    }
}

impl Message {
    pub fn topic(&self) -> &str {
        &self.topic
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn qos(&self) -> QualityOfService {
        self.qos
    }

    pub fn packet_id(&self) -> Option<u16> {
        self.packet_id
    }

    pub fn dup(&self) -> bool {
        self.dup
    }

    pub fn retain(&self) -> bool {
        self.retain
    }
}
