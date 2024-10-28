use std::{future::Future, task::Waker};

use mqtt_codec_kit::v5::packet::VariablePacket;

pub use self::{
    connect_token::ConnectToken, disconnect_token::DisconnectToken, publish_token::PublishToken,
    subscribe_token::SubscribeToken, unsubscribe_token::UnsubscribeToken,
};

pub mod connect_token;
pub mod disconnect_token;
pub mod publish_token;
pub mod subscribe_token;
pub mod unsubscribe_token;

#[derive(Debug, Clone, thiserror::Error)]
pub enum TokenError {
    #[error("IO Error: {0}")]
    IOError(String),
    #[error("Connect Timeout")]
    ConnectTimeout,
    #[error("Network Unreachable")]
    NetworkUnreachable,
    #[error("Connect Error: {0}")]
    ConnectError(String),
    #[error("Connect Failed, return code: ({0})")]
    ConnectFailed(u8),
    #[error("Connection Lost")]
    ConnectionLost,
    #[error("Client Reconnecting")]
    Reconnecting,
    #[error("Connection Error: {0}")]
    ConnectionError(String),
    #[error("Invalid Topic")]
    InvalidTopic,
    #[error("No Packet ID Available")]
    PacketIdError,
    #[error("Internal Server Error")]
    InternalServerError,
    #[error("Packet Error: {0}")]
    PacketError(String),
}

impl From<std::io::Error> for TokenError {
    fn from(err: std::io::Error) -> Self {
        TokenError::IOError(err.to_string())
    }
}

impl From<s2n_quic::provider::StartError> for TokenError {
    fn from(err: s2n_quic::provider::StartError) -> Self {
        TokenError::ConnectError(err.to_string())
    }
}

impl From<s2n_quic::connection::Error> for TokenError {
    fn from(err: s2n_quic::connection::Error) -> Self {
        TokenError::ConnectionError(err.to_string())
    }
}

#[derive(Clone)]
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

    pub(crate) fn set_error(&mut self, err: TokenError) {
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
    pub fn new(packet: VariablePacket) -> Self {
        Self {
            packet,
            token: None,
        }
    }

    pub fn new_with(packet: VariablePacket, token: Token) -> Self {
        Self {
            packet,
            token: Some(token),
        }
    }
}

pub(crate) trait Tokenize: Future {
    fn set_error(&mut self, error: TokenError);
    fn flow_complete(&mut self);
}

#[derive(Default)]
struct State {
    complete: bool,
    waker: Option<Waker>,
}

#[macro_export]
macro_rules! impl_future {
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
                    let error = inner.error.clone();
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
macro_rules! impl_tokenize {
    ($typ:ident) => {
        impl Tokenize for $typ {
            fn set_error(&mut self, error: TokenError) {
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
