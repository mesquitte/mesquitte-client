use std::{future::Future, sync::Arc, task::Poll};

use parking_lot::Mutex;

use crate::{impl_future, impl_tokenize};

use super::{State, TokenError, Tokenize};

#[derive(Default)]
struct InnerToken {
    topics: Vec<String>,

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

    pub fn topics(&self) -> Vec<String> {
        let inner = self.inner.lock();
        let inner = &*inner;

        inner.topics.clone()
    }
}

impl_future!(UnsubscribeToken);

impl_tokenize!(UnsubscribeToken);
