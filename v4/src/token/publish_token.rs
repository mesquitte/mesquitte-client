use std::{future::Future, sync::Arc, task::Poll};

use parking_lot::Mutex;

use mqtt_codec_kit::common::QualityOfService;

use crate::{impl_future, impl_tokenize};

use super::{State, TokenError, Tokenize};

struct InnerToken {
    qos: QualityOfService,

    state: State,
    error: Option<TokenError>,
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

impl_future!(PublishToken);

impl_tokenize!(PublishToken);
