use std::{future::Future, sync::Arc, task::Poll};

use parking_lot::Mutex;

use mqtt_codec_kit::v4::control::ConnectReturnCode;

use crate::{impl_future, impl_tokenize};

use super::{State, TokenError, Tokenize};

#[derive(Default, Clone)]
pub struct ConnectToken {
    inner: Arc<Mutex<InnerToken>>,
}

struct InnerToken {
    return_code: ConnectReturnCode,
    session_present: bool,

    state: State,
    error: Option<TokenError>,
}

impl Default for InnerToken {
    fn default() -> Self {
        Self {
            return_code: ConnectReturnCode::ConnectionAccepted,
            session_present: Default::default(),
            state: Default::default(),
            error: Default::default(),
        }
    }
}

impl ConnectToken {
    pub(crate) fn set_return_code(&mut self, code: ConnectReturnCode) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;

        inner.return_code = code;
    }

    pub(crate) fn set_session_present(&mut self) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;

        inner.session_present = true;
    }

    pub fn return_code(&self) -> ConnectReturnCode {
        let inner = self.inner.lock();
        let inner = &*inner;

        inner.return_code
    }

    pub fn session_present(&self) -> bool {
        let inner = self.inner.lock();
        let inner = &*inner;

        inner.session_present
    }
}

impl_future!(ConnectToken);

impl_tokenize!(ConnectToken);
