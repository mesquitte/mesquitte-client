use std::{
    future::Future,
    sync::{Arc, Mutex},
    task::Poll,
};

use crate::error::{MqttError, TokenError};

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

impl Future for DisconnectToken {
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
