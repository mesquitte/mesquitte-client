use mqtt_codec_kit::common::QualityOfService;

use crate::message::Message;

// Type alias for the handler function signature
pub type OnMessageArrivedHandler = fn(&Message);

#[derive(Debug, Clone)]
pub struct Subscription {
    pub topic: String,
    pub qos: QualityOfService,
    pub handler: OnMessageArrivedHandler,
}

impl Subscription {
    pub fn new<S: Into<String>>(
        topic: S,
        qos: QualityOfService,
        handler: OnMessageArrivedHandler,
    ) -> Self {
        Self {
            topic: topic.into(),
            qos,
            handler,
        }
    }
}

pub trait TopicStore {
    fn add_subscription(&self, sub: Subscription, topic_tokens: Vec<String>);

    fn remove_subscription(&self, topic_tokens: Vec<String>) -> bool;

    fn get_match_subscriptions(&self, topic_tokens: Vec<String>) -> Vec<Subscription>;

    fn clear_subscriptions(&self);
}
