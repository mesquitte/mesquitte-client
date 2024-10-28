use std::{collections::HashMap, future::Future, sync::Arc, task::Poll};

use parking_lot::Mutex;

use mqtt_codec_kit::v5::{control::SubackProperties, packet::suback::SubscribeReasonCode};

use crate::{impl_future, impl_tokenize, topic_store::Subscription};

use super::{State, TokenError, Tokenize};

#[derive(Default)]
struct InnerToken {
    subscriptions: Vec<Subscription>,
    results: HashMap<String, SubscribeReasonCode>,
    suback_properties: SubackProperties,

    state: State,
    error: Option<TokenError>,
}

#[derive(Clone, Default)]
pub struct SubscribeToken {
    inner: Arc<Mutex<InnerToken>>,
}

impl SubscribeToken {
    pub(crate) fn add_subscriptions(&mut self, subscriptions: Vec<Subscription>) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;

        inner.subscriptions.extend(subscriptions);
    }

    pub(crate) fn get_subscription(&self, index: usize) -> Option<Subscription> {
        let inner = self.inner.lock();
        let inner = &*inner;

        inner.subscriptions.get(index).cloned()
    }

    pub(crate) fn set_result(&mut self, index: usize, result: SubscribeReasonCode) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;

        if let Some(subscription) = inner.subscriptions.get(index) {
            inner.results.insert(subscription.topic.to_owned(), result);
        }
    }

    pub(crate) fn set_suback_properties(&mut self, properties: SubackProperties) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;

        inner.suback_properties = properties;
    }

    pub fn result(&self) -> HashMap<String, SubscribeReasonCode> {
        let inner = self.inner.lock();
        let inner = &*inner;

        inner.results.clone()
    }

    pub fn suback_properties(&self) -> SubackProperties {
        let inner = self.inner.lock();
        let inner = &*inner;

        inner.suback_properties.clone()
    }
}

impl_future!(SubscribeToken);

impl_tokenize!(SubscribeToken);
