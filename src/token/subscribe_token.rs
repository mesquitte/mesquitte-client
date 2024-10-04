use std::{collections::HashMap, future::Future, sync::Arc, task::Poll};

use parking_lot::Mutex;

use mqtt_codec_kit::v4::packet::suback::SubscribeReturnCode;

use crate::{impl_future, impl_tokenize, topic_store::Subscription};

use super::{State, TokenError, Tokenize};

#[derive(Default)]
struct InnerToken {
    subscriptions: Vec<Subscription>,
    results: HashMap<String, SubscribeReturnCode>,

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

    pub(crate) fn set_result(&mut self, index: usize, result: SubscribeReturnCode) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;

        if let Some(subscription) = inner.subscriptions.get(index) {
            inner.results.insert(subscription.topic.to_owned(), result);
        }
    }

    pub fn result(&self) -> HashMap<String, SubscribeReturnCode> {
        let inner = self.inner.lock();
        let inner = &*inner;

        inner.results.clone()
    }
}

impl_future!(SubscribeToken);

impl_tokenize!(SubscribeToken);
