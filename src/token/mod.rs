use std::{future::Future, task::Waker};

use mqtt_codec_kit::v4::packet::VariablePacket;

use crate::error::MqttError;

pub use self::{
    connect_token::ConnectToken, disconnect_token::DisconnectToken, publish_token::PublishToken,
    subscribe_token::SubscribeToken, unsubscribe_token::UnsubscribeToken,
};

pub mod connect_token;
pub mod disconnect_token;
pub mod publish_token;
pub mod subscribe_token;
pub mod unsubscribe_token;

pub enum Token {
    Connect(ConnectToken),
    Disconnect(DisconnectToken),
    Publish(PublishToken),
    Subscribe(SubscribeToken),
    Unsubscribe(UnsubscribeToken),
}

impl Token {
    pub(crate) fn flow_complete(&mut self) {
        match self {
            Token::Connect(connect_token) => connect_token.flow_complete(),
            Token::Disconnect(disconnect_token) => disconnect_token.flow_complete(),
            Token::Publish(publish_token) => publish_token.flow_complete(),
            Token::Subscribe(subscribe_token) => subscribe_token.flow_complete(),
            Token::Unsubscribe(unsubscribe_token) => unsubscribe_token.flow_complete(),
        }
    }

    pub(crate) fn set_error(&mut self, err: MqttError) {
        match self {
            Token::Connect(connect_token) => connect_token.set_error(err),
            Token::Disconnect(disconnect_token) => disconnect_token.set_error(err),
            Token::Publish(publish_token) => publish_token.set_error(err),
            Token::Subscribe(subscribe_token) => subscribe_token.set_error(err),
            Token::Unsubscribe(unsubscribe_token) => unsubscribe_token.set_error(err),
        }
    }
}

pub struct PacketAndToken {
    pub packet: VariablePacket,
    pub token: Option<Token>,
}

impl PacketAndToken {
    pub fn new(packet: VariablePacket, token: Token) -> Self {
        Self {
            packet,
            token: Some(token),
        }
    }
}

pub(crate) trait Tokenize: Future {
    fn set_error(&mut self, error: MqttError);
    fn flow_complete(&mut self);
}

#[derive(Default)]
struct State {
    complete: bool,
    waker: Option<Waker>,
}

#[macro_export]
macro_rules! enable_future {
    ($typ:ident) => {
        impl Future for $typ {
            type Output = Option<TokenError>;

            fn poll(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Self::Output> {
                let mut inner = self.inner.lock();
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
    };
}

#[macro_export]
macro_rules! enable_tokenize {
    ($typ:ident) => {
        impl Tokenize for $typ {
            fn set_error(&mut self, error: MqttError) {
                let mut inner = self.inner.lock();
                let inner = &mut *inner;
                inner.error = Some(error);

                inner.state.complete = true;
                if let Some(waker) = inner.state.waker.take() {
                    waker.wake();
                }
            }

            fn flow_complete(&mut self) {
                let mut inner = self.inner.lock();
                let inner = &mut *inner;

                inner.state.complete = true;
                if let Some(waker) = inner.state.waker.take() {
                    waker.wake();
                }
            }
        }
    };
}
