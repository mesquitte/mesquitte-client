use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::topic_store::{OnMessageArrivedHandler, TopicStore};

#[derive(Debug)]
struct TrieNode {
    children: HashMap<String, Arc<RwLock<TrieNode>>>,
    handlers: Vec<OnMessageArrivedHandler>,
}

impl TrieNode {
    fn new() -> Self {
        TrieNode {
            children: HashMap::new(),
            handlers: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct MqttTopicTrie {
    root: Arc<RwLock<TrieNode>>,
}

impl MqttTopicTrie {
    fn new() -> Self {
        MqttTopicTrie {
            root: Arc::new(RwLock::new(TrieNode::new())),
        }
    }

    fn insert(&self, handler: OnMessageArrivedHandler, topic_tokens: Vec<String>) {
        let mut current_node = Arc::clone(&self.root);

        for token in topic_tokens {
            let next_node_arc;
            {
                let mut node = current_node.write().unwrap();
                next_node_arc = node
                    .children
                    .entry(token)
                    .or_insert_with(|| Arc::new(RwLock::new(TrieNode::new())))
                    .clone();
            }
            current_node = next_node_arc;
        }

        current_node.write().unwrap().handlers.push(handler);
    }

    fn remove(&self, topic_tokens: Vec<String>) -> bool {
        let mut current_node = Arc::clone(&self.root);
        let mut nodes_to_cleanup = Vec::new();

        for token in &topic_tokens {
            let next_node_arc;
            {
                let node = current_node.read().unwrap();
                match node.children.get(token) {
                    Some(next_node) => {
                        nodes_to_cleanup.push((Arc::clone(&current_node), token.clone()));
                        next_node_arc = Arc::clone(next_node);
                    }
                    None => return false,
                }
            }
            current_node = next_node_arc;
        }

        {
            let mut final_node = current_node.write().unwrap();
            final_node.handlers.clear();
        }

        for (node, token) in nodes_to_cleanup.into_iter().rev() {
            let remove_child;
            {
                let node_read = node.read().unwrap();
                if let Some(child_node) = node_read.children.get(&token) {
                    let child_read = child_node.read().unwrap();
                    remove_child = child_read.children.is_empty() && child_read.handlers.is_empty();
                } else {
                    remove_child = false;
                }
            }

            if remove_child {
                let mut node_write = node.write().unwrap();
                node_write.children.remove(&token);
            }
        }

        true
    }

    fn get_matching_handlers(&self, topic_tokens: Vec<String>) -> Vec<OnMessageArrivedHandler> {
        let mut matching_handlers = Vec::new();
        self.match_topic(&self.root, &topic_tokens, 0, &mut matching_handlers);
        matching_handlers
    }

    fn match_topic(
        &self,
        node: &Arc<RwLock<TrieNode>>,
        tokens: &[String],
        level: usize,
        matching_handlers: &mut Vec<OnMessageArrivedHandler>,
    ) {
        let node_read = node.read().unwrap();

        if level == tokens.len() {
            matching_handlers.extend(node_read.handlers.clone());
            return;
        }

        let token = &tokens[level];

        if let Some(next_node) = node_read.children.get(token) {
            self.match_topic(next_node, tokens, level + 1, matching_handlers);
        }

        // Match single-level wildcard "+"
        if let Some(plus_node) = node_read.children.get("+") {
            self.match_topic(plus_node, tokens, level + 1, matching_handlers);
        }

        // Match multi-level wildcard "#", it matches the rest of the tokens
        if let Some(hash_node) = node_read.children.get("#") {
            matching_handlers.extend(hash_node.read().unwrap().handlers.clone());
        }
    }
}

impl TopicStore for MqttTopicTrie {
    fn add_subscription(&self, handler: OnMessageArrivedHandler, topic_tokens: Vec<String>) {
        self.insert(handler, topic_tokens);
    }

    fn remove_subscription(&self, topic_tokens: Vec<String>) -> bool {
        self.remove(topic_tokens)
    }

    fn get_match_subscription(&self, topic_tokens: Vec<String>) -> Vec<OnMessageArrivedHandler> {
        self.get_matching_handlers(topic_tokens)
    }
}

pub struct TopicManager {
    store: MqttTopicTrie,
}

impl TopicManager {
    pub fn new() -> Self {
        Self {
            store: MqttTopicTrie::new(),
        }
    }

    pub fn add<S: Into<String>>(&self, sub: (S, OnMessageArrivedHandler)) {
        let topic_tokens = Self::tokenize_topic(&sub.0.into());
        self.store.add_subscription(sub.1, topic_tokens);
    }

    pub fn remove<S: Into<String>>(&self, topic: S) -> bool {
        let topic_tokens = Self::tokenize_topic(&topic.into());
        self.store.remove_subscription(topic_tokens)
    }

    pub fn match_topic<S: Into<String>>(&self, topic: S) -> Vec<OnMessageArrivedHandler> {
        let topic_tokens = Self::tokenize_topic(&topic.into());
        self.store.get_match_subscription(topic_tokens)
    }

    fn tokenize_topic(topic: &str) -> Vec<String> {
        topic.split('/').map(String::from).collect()
    }
}

#[cfg(test)]
mod test {
    use mqtt_codec_kit::common::QualityOfService;

    use super::TopicManager;

    fn handler1(_topic: &str, _payload: &[u8], _qos: QualityOfService) {
        println!("handler1");
    }

    fn handler2(_topic: &str, _payload: &[u8], _qos: QualityOfService) {
        println!("handler2");
    }

    fn handler3(_topic: &str, _payload: &[u8], _qos: QualityOfService) {
        println!("handler3");
    }

    #[test]
    fn test_topic_manager() {
        let manager = TopicManager::new();
        manager.add(("sport/football", handler1));
        manager.add(("sport/+/news", handler2));
        manager.add(("sport/#", handler3));
        manager.remove("sport/#");

        // Test matching topics
        let matched = manager.match_topic("sport/football/news");
        assert!(matched.len() == 1)
    }
}
