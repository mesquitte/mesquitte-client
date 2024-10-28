use std::{collections::HashMap, sync::Arc};

use parking_lot::RwLock;

use crate::topic_store::{Subscription, TopicStore};

#[derive(Debug)]
struct TrieNode {
    children: HashMap<String, Arc<RwLock<TrieNode>>>,
    subscriptions: Vec<Subscription>,
}

impl TrieNode {
    fn new() -> Self {
        TrieNode {
            children: HashMap::new(),
            subscriptions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct MqttTopicTrie {
    root: Arc<RwLock<TrieNode>>,
}

impl MqttTopicTrie {
    fn new() -> Self {
        MqttTopicTrie {
            root: Arc::new(RwLock::new(TrieNode::new())),
        }
    }

    fn insert(&self, sub: Subscription, topic_tokens: Vec<String>) {
        let mut current_node = Arc::clone(&self.root);

        for token in topic_tokens {
            let next_node_arc;
            {
                let mut node = current_node.write();
                next_node_arc = node
                    .children
                    .entry(token)
                    .or_insert_with(|| Arc::new(RwLock::new(TrieNode::new())))
                    .clone();
            }
            current_node = next_node_arc;
        }

        current_node.write().subscriptions.push(sub);
    }

    fn remove(&self, topic_tokens: Vec<String>) -> bool {
        let mut current_node = Arc::clone(&self.root);
        let mut nodes_to_cleanup = Vec::new();

        for token in &topic_tokens {
            let next_node_arc;
            {
                let node = current_node.read();
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
            let mut final_node = current_node.write();
            final_node.subscriptions.clear();
        }

        for (node, token) in nodes_to_cleanup.into_iter().rev() {
            let remove_child;
            {
                let node_read = node.read();
                if let Some(child_node) = node_read.children.get(&token) {
                    let child_read = child_node.read();
                    remove_child =
                        child_read.children.is_empty() && child_read.subscriptions.is_empty();
                } else {
                    remove_child = false;
                }
            }

            if remove_child {
                let mut node_write = node.write();
                node_write.children.remove(&token);
            }
        }

        true
    }

    fn get_matching_subscriptions(&self, topic_tokens: Vec<String>) -> Vec<Subscription> {
        let mut matching_subscriptions = Vec::new();
        self.match_topic(&self.root, &topic_tokens, 0, &mut matching_subscriptions);
        matching_subscriptions
    }

    #[allow(clippy::only_used_in_recursion)]
    fn match_topic(
        &self,
        node: &Arc<RwLock<TrieNode>>,
        tokens: &[String],
        level: usize,
        matching_subscriptions: &mut Vec<Subscription>,
    ) {
        let node_read = node.read();

        if level == tokens.len() {
            matching_subscriptions.extend(node_read.subscriptions.clone());
            return;
        }

        let token = &tokens[level];

        if let Some(next_node) = node_read.children.get(token) {
            self.match_topic(next_node, tokens, level + 1, matching_subscriptions);
        }

        // Match single-level wildcard "+"
        if let Some(plus_node) = node_read.children.get("+") {
            self.match_topic(plus_node, tokens, level + 1, matching_subscriptions);
        }

        // Match multi-level wildcard "#", it matches the rest of the tokens
        if let Some(hash_node) = node_read.children.get("#") {
            matching_subscriptions.extend(hash_node.read().subscriptions.clone());
        }
    }

    // Clear all subscriptions from the Trie
    fn clear(&self) {
        let mut root = self.root.write();
        self.clear_node(&mut root);
    }

    // Helper function to recursively clear subscriptions from nodes
    #[allow(clippy::only_used_in_recursion)]
    fn clear_node(&self, node: &mut TrieNode) {
        // Clear the subscriptions of the current node
        node.subscriptions.clear();

        // Recursively clear all child nodes
        for child in node.children.values() {
            let mut child_node = child.write();
            self.clear_node(&mut child_node);
        }
    }
}

impl TopicStore for MqttTopicTrie {
    fn add_subscription(&self, subscription: Subscription, topic_tokens: Vec<String>) {
        self.insert(subscription, topic_tokens);
    }

    fn remove_subscription(&self, topic_tokens: Vec<String>) -> bool {
        self.remove(topic_tokens)
    }

    fn get_match_subscriptions(&self, topic_tokens: Vec<String>) -> Vec<Subscription> {
        self.get_matching_subscriptions(topic_tokens)
    }

    fn clear_subscriptions(&self) {
        self.clear();
    }
}

#[derive(Clone)]
pub struct TopicManager {
    store: MqttTopicTrie,
}

impl TopicManager {
    pub fn new() -> Self {
        Self {
            store: MqttTopicTrie::new(),
        }
    }

    pub fn add(&self, subscription: Subscription) {
        let topic_tokens = Self::tokenize_topic(&subscription.topic);
        self.store.add_subscription(subscription, topic_tokens);
    }

    pub fn remove<S: Into<String>>(&self, topic: S) -> bool {
        let topic_tokens = Self::tokenize_topic(&topic.into());
        self.store.remove_subscription(topic_tokens)
    }

    pub fn match_topic<S: Into<String>>(&self, topic: S) -> Vec<Subscription> {
        let topic_tokens = Self::tokenize_topic(&topic.into());
        self.store.get_match_subscriptions(topic_tokens)
    }

    pub fn clear(&self) {
        self.store.clear_subscriptions();
    }

    fn tokenize_topic(topic: &str) -> Vec<String> {
        topic.split('/').map(String::from).collect()
    }
}

#[cfg(test)]
mod test {
    use mqtt_codec_kit::v5::{control::SubscribeProperties, packet::subscribe::SubscribeOptions};

    use crate::{message::Message, topic_store::Subscription};

    use super::TopicManager;

    fn handler1(_msg: &Message) {
        println!("handler1");
    }

    fn handler2(_msg: &Message) {
        println!("handler2");
    }

    fn handler3(_msg: &Message) {
        println!("handler3");
    }

    #[test]
    fn test_topic_manager() {
        let manager = TopicManager::new();
        let options = SubscribeOptions::default();
        let properties = SubscribeProperties::default();
        manager.add(Subscription {
            topic: "sport/football".to_owned(),
            options,
            properties: properties.clone(),
            handler: handler1,
        });
        manager.add(Subscription {
            topic: "sport/+/news".to_owned(),
            options,
            properties: properties.clone(),
            handler: handler2,
        });
        manager.add(Subscription {
            topic: "sport/#".to_owned(),
            options,
            properties: properties.clone(),
            handler: handler3,
        });
        manager.remove("sport/#");

        // Test matching topics
        let matched = manager.match_topic("sport/football/news");
        assert!(matched.len() == 1);

        // clear all subscriptions
        manager.store.clear();

        let matched = manager.match_topic("sport/football");
        assert!(matched.is_empty());
    }
}
