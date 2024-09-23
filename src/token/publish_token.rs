use std::{future::Future, sync::Arc, task::Poll};

use parking_lot::Mutex;

use mqtt_codec_kit::common::QualityOfService;

use crate::{
    enable_future, enable_tokenize,
    error::{MqttError, TokenError},
};

use super::{State, Tokenize};

struct InnerToken {
    qos: QualityOfService,

    state: State,
    error: Option<MqttError>,
}

impl Default for InnerToken {
    fn default() -> Self {
        Self {
            qos: QualityOfService::Level0,
            state: Default::default(),
            error: Default::default(),
        }
    }
}

#[derive(Clone, Default)]
pub struct PublishToken {
    inner: Arc<Mutex<InnerToken>>,
}

impl PublishToken {
    pub(crate) fn set_qos(&mut self, qos: QualityOfService) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;

        inner.qos = qos;
    }

    pub(crate) fn qos(&self) -> QualityOfService {
        let inner = self.inner.lock();
        let inner = &*inner;

        inner.qos
    }
}

enable_future!(PublishToken);

enable_tokenize!(PublishToken);
