use std::{
    future::Future,
    sync::{Arc, Mutex},
    task::Poll,
};

use crate::{
    enable_future,
    error::{MqttError, TokenError},
};

use super::{State, Tokenize};

#[derive(Default)]
struct InnerToken {
    error: Option<MqttError>,

    state: State,
}

#[derive(Default, Clone)]
pub struct DisconnectToken {
    inner: Arc<Mutex<InnerToken>>,
}

enable_future!(DisconnectToken);

impl Tokenize for DisconnectToken {
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
