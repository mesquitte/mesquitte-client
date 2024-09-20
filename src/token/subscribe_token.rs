use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
    task::Poll,
};

use mqtt_codec_kit::v4::packet::suback::SubscribeReturnCode;

use crate::{
    enable_future,
    error::{MqttError, TokenError},
    topic_store::OnMessageArrivedHandler,
};

use super::{State, Tokenize};

#[derive(Default)]
struct InnerToken {
    error: Option<MqttError>,
    topics: Vec<String>,
    handlers: HashMap<String, OnMessageArrivedHandler>,
    results: HashMap<String, SubscribeReturnCode>,

    state: State,
}

#[derive(Clone, Default)]
pub struct SubscribeToken {
    inner: Arc<Mutex<InnerToken>>,
}

impl SubscribeToken {
    pub(crate) fn add_subscriptions<S: Into<String>>(
        &mut self,
        subscriptions: Vec<(S, OnMessageArrivedHandler)>,
    ) {
        let mut inner = self.inner.lock().unwrap();
        let inner = &mut *inner;

        for (topic, handler) in subscriptions {
            let topic: String = topic.into();
            inner.topics.push(topic.to_owned());
            inner.handlers.insert(topic.to_owned(), handler);
        }
    }

    pub(crate) fn get_handler<S: Into<String>>(&self, topic: S) -> Option<OnMessageArrivedHandler> {
        let inner = self.inner.lock().unwrap();
        let inner = &*inner;

        inner.handlers.get(&topic.into()).copied()
    }

    pub(crate) fn set_result(&mut self, index: usize, result: SubscribeReturnCode) -> String {
        let mut inner = self.inner.lock().unwrap();
        let inner = &mut *inner;

        let mut return_topic = String::new();

        if let Some(topic) = inner.topics.get(index) {
            if inner.topics.contains(topic) {
                inner.results.insert(topic.to_owned(), result);
                return_topic = topic.to_string();
            }
        }
        return_topic
    }

    pub fn result(&self) -> HashMap<String, SubscribeReturnCode> {
        let inner = self.inner.lock().unwrap();
        let inner = &*inner;

        inner.results.clone()
    }
}

enable_future!(SubscribeToken);

impl Tokenize for SubscribeToken {
    fn set_error(&mut self, error: MqttError) {
        let mut inner = self.inner.lock().unwrap();
        let inner = &mut *inner;

        inner.error = Some(error);
        inner.state.complete = true;
        if let Some(waker) = inner.state.waker.take() {
            waker.wake();
        }
    }

    fn flow_complete(&mut self) {
        let mut inner = self.inner.lock().unwrap();
        let inner = &mut *inner;

        inner.state.complete = true;
        if let Some(waker) = inner.state.waker.take() {
            waker.wake();
        }
    }
}
