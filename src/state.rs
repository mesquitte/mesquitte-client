use std::time::Duration;

use moka::future::Cache;
use parking_lot::Mutex;
use tokio::{sync::mpsc, time::Instant};

use mqtt_codec_kit::v4::packet::PublishPacket;

use crate::{
    pkid::{self, PacketIds},
    token::PacketAndToken,
    topic_store::Subscription,
    topic_trie::TopicManager,
};

pub struct State {
    pub packet_ids: Mutex<PacketIds>,
    pub topic_manager: TopicManager,
    pub subscriptions: Mutex<Vec<Subscription>>,
    pub pending_packets: Cache<u16, PublishPacket>,
    pub outgoing_tx: Option<mpsc::Sender<PacketAndToken>>,
    pub last_sent_packet_at: Mutex<Instant>,
}

impl State {
    pub fn new() -> Self {
        Self {
            packet_ids: Mutex::new(PacketIds::new()),
            topic_manager: TopicManager::new(),
            subscriptions: Mutex::new(Vec::new()),
            pending_packets: Cache::builder()
                .time_to_live(Duration::from_secs(60))
                .max_capacity(pkid::PKID_MAX.into())
                .build(),
            outgoing_tx: None,
            last_sent_packet_at: Mutex::new(Instant::now()),
        }
    }
}
