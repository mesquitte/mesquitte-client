use std::future::Future;

use mqtt_codec_kit::{
    common::QualityOfService,
    v5::{
        control::{PublishProperties, SubscribeProperties},
        packet::{connect::ConnectProperties, subscribe::SubscribeOptions},
    },
};

use topic_store::OnMessageArrivedHandler;

pub mod client;
pub mod message;
mod net;
pub mod options;
mod pkid;
mod state;
pub mod token;
mod topic_store;
mod topic_trie;
pub mod transport;

pub trait Client {
    fn connect(
        &mut self,
        properties: ConnectProperties,
    ) -> impl Future<Output = token::ConnectToken> + Send;

    fn disconnect(&mut self) -> impl Future<Output = token::DisconnectToken> + Send;

    fn publish<S, V>(
        &mut self,
        topic: S,
        qos: QualityOfService,
        retained: bool,
        payload: V,
        properties: PublishProperties,
    ) -> impl Future<Output = token::PublishToken> + Send
    where
        S: Into<String> + Send,
        V: Into<Vec<u8>> + Send;

    fn subscribe<S: Into<String> + Send>(
        &mut self,
        topic: S,
        options: SubscribeOptions,
        properties: SubscribeProperties,
        callback: OnMessageArrivedHandler,
    ) -> impl Future<Output = token::SubscribeToken> + Send;

    fn subscribe_multiple<S: Into<String> + Clone + Send>(
        &mut self,
        subscriptions: Vec<(S, SubscribeOptions, SubscribeProperties)>,
        callback: OnMessageArrivedHandler,
    ) -> impl Future<Output = token::SubscribeToken> + Send;

    fn unsubscribe<S: Into<String> + Send>(
        &mut self,
        topics: Vec<S>,
    ) -> impl Future<Output = token::UnsubscribeToken> + Send;
}
