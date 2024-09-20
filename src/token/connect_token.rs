use std::{
    future::Future,
    sync::{Arc, Mutex},
    task::Poll,
};

use mqtt_codec_kit::v4::control::ConnectReturnCode;

use crate::{
    enable_future,
    error::{MqttError, TokenError},
};

use super::{State, Tokenize};

#[derive(Default)]
pub struct ConnectToken {
    inner: Arc<Mutex<InnerToken>>,
}

struct InnerToken {
    error: Option<MqttError>,
    return_code: ConnectReturnCode,
    session_present: bool,

    state: State,
}

impl Default for InnerToken {
    fn default() -> Self {
        Self {
            error: Default::default(),
            return_code: ConnectReturnCode::ConnectionAccepted,
            session_present: Default::default(),
            state: Default::default(),
        }
    }
}

impl ConnectToken {
    pub(crate) fn set_return_code(&mut self, code: ConnectReturnCode) {
        let mut inner = self.inner.lock().unwrap();
        let inner = &mut *inner;

        inner.return_code = code;
    }

    pub(crate) fn set_session_present(&mut self) {
        let mut inner = self.inner.lock().unwrap();
        let inner = &mut *inner;

        inner.session_present = true;
    }

    pub fn return_code(&self) -> ConnectReturnCode {
        let inner = self.inner.lock().unwrap();
        let inner = &*inner;

        inner.return_code
    }

    pub fn session_present(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        let inner = &*inner;

        inner.session_present
    }
}

enable_future!(ConnectToken);

impl Tokenize for ConnectToken {
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
