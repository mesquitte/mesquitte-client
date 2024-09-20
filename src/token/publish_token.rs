use std::{
    future::Future,
    sync::{Arc, Mutex},
    task::Poll,
};

use mqtt_codec_kit::common::QualityOfService;

use crate::{
    enable_future,
    error::{MqttError, TokenError},
};

use super::{State, Tokenize};

struct InnerToken {
    error: Option<MqttError>,
    qos: QualityOfService,

    state: State,
}

impl Default for InnerToken {
    fn default() -> Self {
        Self {
            error: Default::default(),
            qos: QualityOfService::Level0,
            state: Default::default(),
        }
    }
}

#[derive(Clone, Default)]
pub struct PublishToken {
    inner: Arc<Mutex<InnerToken>>,
}

impl PublishToken {
    pub(crate) fn set_qos(&mut self, qos: QualityOfService) {
        let mut inner = self.inner.lock().unwrap();
        let inner = &mut *inner;

        inner.qos = qos;
    }

    pub(crate) fn qos(&self) -> QualityOfService {
        let inner = self.inner.lock().unwrap();
        let inner = &*inner;

        inner.qos
    }
}

enable_future!(PublishToken);

impl Tokenize for PublishToken {
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
