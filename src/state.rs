use std::sync::Mutex;

use tokio::sync::mpsc;

use crate::{pkid::PacketIds, token::PacketAndToken, topic_trie::TopicManager};

pub struct State {
    pub pkids: Mutex<PacketIds>,
    pub subscribers: TopicManager,
    pub outgoing_tx: Option<mpsc::Sender<PacketAndToken>>,
}

impl State {
    pub fn new() -> Self {
        Self {
            pkids: Mutex::new(PacketIds::new()),
            subscribers: TopicManager::new(),
            outgoing_tx: None,
        }
    }
}