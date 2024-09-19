use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
    task::Poll,
};

use mqtt_codec_kit::v4::packet::suback::SubscribeReturnCode;

use crate::{
    error::{MqttError, TokenError},
    topic_store::OnMessageArrivedHandler,
};

use super::{State, Tokenize};

#[derive(Default)]
struct InnerToken {
    error: Option<MqttError>,
    subs: Vec<String>,
    sub_handler: HashMap<String, OnMessageArrivedHandler>,
    sub_result: HashMap<String, SubscribeReturnCode>,

    state: State,
}

#[derive(Clone, Default)]
pub struct SubscribeToken {
    inner: Arc<Mutex<InnerToken>>,
}

impl SubscribeToken {
    pub(crate) fn add_subscription<S: Into<String>>(
        &mut self,
        topic: S,
        handler: OnMessageArrivedHandler,
    ) {
        let mut inner = self.inner.lock().unwrap();
        let inner = &mut *inner;

        let topic: String = topic.into();
        inner.subs.push(topic.to_owned());
        inner.sub_handler.insert(topic.to_owned(), handler);
    }

    pub(crate) fn get_handler<S: Into<String>>(&self, topic: S) -> Option<OnMessageArrivedHandler> {
        let inner = self.inner.lock().unwrap();
        let inner = &*inner;

        inner.sub_handler.get(&topic.into()).copied()
    }

    pub(crate) fn set_result(&mut self, index: usize, ret: SubscribeReturnCode) -> String {
        let mut inner = self.inner.lock().unwrap();
        let inner = &mut *inner;

        let mut ret_topic = String::new();

        if let Some(topic) = inner.subs.get(index) {
            if inner.subs.contains(topic) {
                inner.sub_result.insert(topic.to_owned(), ret);
                ret_topic = topic.to_string();
            }
        }
        ret_topic
    }

    pub fn result(&self) -> Vec<String> {
        let inner = self.inner.lock().unwrap();
        let inner = &*inner;

        let mut ret = vec![];

        for t in inner.subs.clone() {
            ret.push(t);
        }
        ret.to_vec()
    }
}

impl Future for SubscribeToken {
    type Output = Option<TokenError>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut inner = self.inner.lock().unwrap();
        let inner = &mut *inner;

        if inner.state.complete {
            let error = inner.error.as_ref().map(|e| e.into());
            Poll::Ready(error)
        } else {
            inner.state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

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
