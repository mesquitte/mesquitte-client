use std::{future::Future, sync::Arc, task::Poll};

use parking_lot::Mutex;

use mqtt_codec_kit::v5::control::{ConnackProperties, ConnectReasonCode};

use crate::{impl_future, impl_tokenize};

use super::{State, TokenError, Tokenize};

#[derive(Default, Clone)]
pub struct ConnectToken {
    inner: Arc<Mutex<InnerToken>>,
}

struct InnerToken {
    reason_code: ConnectReasonCode,
    session_present: bool,
    connack_properties: ConnackProperties,

    state: State,
    error: Option<TokenError>,
}

impl Default for InnerToken {
    fn default() -> Self {
        Self {
            reason_code: ConnectReasonCode::Success,
            session_present: Default::default(),
            connack_properties: Default::default(),
            state: Default::default(),
            error: Default::default(),
        }
    }
}

impl ConnectToken {
    pub(crate) fn set_reason_code(&mut self, code: ConnectReasonCode) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;

        inner.reason_code = code;
    }

    pub(crate) fn set_session_present(&mut self) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;

        inner.session_present = true;
    }

    pub(crate) fn set_connack_properties(&mut self, properties: ConnackProperties) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;

        inner.connack_properties = properties;
    }

    pub fn reason_code(&self) -> ConnectReasonCode {
        let inner = self.inner.lock();
        let inner = &*inner;

        inner.reason_code
    }

    pub fn session_present(&self) -> bool {
        let inner = self.inner.lock();
        let inner = &*inner;

        inner.session_present
    }

    pub fn connack_properties(&self) -> ConnackProperties {
        let inner = self.inner.lock();
        let inner = &*inner;

        inner.connack_properties.clone()
    }
}

impl_future!(ConnectToken);

impl_tokenize!(ConnectToken);
