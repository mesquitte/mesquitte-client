use crate::message::Message;

// Type alias for the handler function signature
pub type OnMessageArrivedHandler = fn(&Message);

pub trait TopicStore {
    fn add_subscription(&self, handler: OnMessageArrivedHandler, topic_tokens: Vec<String>);

    fn remove_subscription(&self, topic_tokens: Vec<String>) -> bool;

    fn get_match_subscription(&self, topic_tokens: Vec<String>) -> Vec<OnMessageArrivedHandler>;
}
