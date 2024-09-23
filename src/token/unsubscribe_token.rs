use std::{future::Future, sync::Arc, task::Poll};

use parking_lot::Mutex;

use crate::{
    enable_future, enable_tokenize,
    error::{MqttError, TokenError},
};

use super::{State, Tokenize};

#[derive(Default)]
struct InnerToken {
    subs: Vec<String>,

    state: State,
    error: Option<MqttError>,
}

#[derive(Clone, Default)]
pub struct UnsubscribeToken {
    inner: Arc<Mutex<InnerToken>>,
}

impl UnsubscribeToken {
    pub(crate) fn add_topic<S: Into<String>>(&mut self, topic: S) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;

        inner.subs.push(topic.into());
    }

    pub fn topics(&self) -> Vec<String> {
        let inner = self.inner.lock();
        let inner = &*inner;

        inner.subs.clone()
    }
}

enable_future!(UnsubscribeToken);

enable_tokenize!(UnsubscribeToken);
