use std::{future::Future, task::Waker};

use mqtt_codec_kit::v4::packet::VariablePacket;

use crate::error::MqttError;

pub mod connect_token;
pub mod disconnect_token;
pub mod publish_token;
pub mod subscribe_token;
pub mod unsubscribe_token;

pub use self::{
    connect_token::ConnectToken, disconnect_token::DisconnectToken, publish_token::PublishToken,
    subscribe_token::SubscribeToken, unsubscribe_token::UnsubscribeToken,
};

pub enum Token {
    Connect(ConnectToken),
    Disconnect(DisconnectToken),
    Publish(PublishToken),
    Subscribe(SubscribeToken),
    Unsubscribe(UnsubscribeToken),
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

pub trait Tokenize: Future {
    fn set_error(&mut self, error: MqttError);
    fn flow_complete(&mut self);
}

#[derive(Default)]
struct State {
    complete: bool,
    waker: Option<Waker>,
}
