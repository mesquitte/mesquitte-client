use std::{future::Future, sync::Arc, task::Poll};

use parking_lot::Mutex;

use crate::{
    enable_future,
    error::{MqttError, TokenError},
};

use super::{State, Tokenize};

#[derive(Default)]
struct InnerToken {
    error: Option<MqttError>,
    subs: Vec<String>,

    state: State,
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

impl Tokenize for UnsubscribeToken {
    fn set_error(&mut self, error: MqttError) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;
        inner.error = Some(error);

        inner.state.complete = true;
        if let Some(waker) = inner.state.waker.take() {
            waker.wake();
        }
    }

    fn flow_complete(&mut self) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;

        inner.state.complete = true;
        if let Some(waker) = inner.state.waker.take() {
            waker.wake();
        }
    }
}
