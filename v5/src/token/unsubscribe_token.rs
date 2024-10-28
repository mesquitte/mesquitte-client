use std::{future::Future, sync::Arc, task::Poll};

use mqtt_codec_kit::v5::control::UnsubackProperties;
use parking_lot::Mutex;

use crate::{impl_future, impl_tokenize};

use super::{State, TokenError, Tokenize};

#[derive(Default)]
struct InnerToken {
    topics: Vec<String>,
    unsuback_properties: UnsubackProperties,

    state: State,
    error: Option<TokenError>,
}

#[derive(Clone, Default)]
pub struct UnsubscribeToken {
    inner: Arc<Mutex<InnerToken>>,
}

impl UnsubscribeToken {
    pub(crate) fn add_topic<S: Into<String>>(&mut self, topic: S) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;

        inner.topics.push(topic.into());
    }

    pub(crate) fn set_unsuback_properties(&mut self, properties: UnsubackProperties) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;

        inner.unsuback_properties = properties;
    }

    pub fn topics(&self) -> Vec<String> {
        let inner = self.inner.lock();
        let inner = &*inner;

        inner.topics.clone()
    }

    pub fn unsuback_properties(&self) -> UnsubackProperties {
        let inner = self.inner.lock();
        let inner = &*inner;

        inner.unsuback_properties.clone()
    }
}

impl_future!(UnsubscribeToken);

impl_tokenize!(UnsubscribeToken);
