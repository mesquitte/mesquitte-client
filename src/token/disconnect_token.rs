use std::{future::Future, sync::Arc, task::Poll};

use parking_lot::Mutex;

use crate::{
    impl_future, impl_tokenize,
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

impl_future!(DisconnectToken);

impl_tokenize!(DisconnectToken);
