use std::future::Future;

use mqtt_codec_kit::common::QualityOfService;
use topic_store::OnMessageArrivedHandler;

pub mod client;
pub mod error;
pub(crate) mod net;
pub mod options;
mod pkid;
pub(crate) mod state;
pub mod token;
mod topic_store;
mod topic_trie;

pub trait Client {
    fn connect(&mut self) -> impl Future<Output = token::ConnectToken> + Send;

    fn disconnect(&mut self) -> impl Future<Output = token::DisconnectToken> + Send;

    fn publish<S, V>(
        &mut self,
        topic: S,
        qos: QualityOfService,
        retained: bool,
        payload: V,
    ) -> impl Future<Output = token::PublishToken> + Send
    where
        S: Into<String> + Send,
        V: Into<Vec<u8>> + Send;

    fn subscribe<S: Into<String> + Send>(
        &mut self,
        topic: S,
        qos: QualityOfService,
        callback: OnMessageArrivedHandler,
    ) -> impl Future<Output = token::SubscribeToken> + Send;

    fn subscribe_multiple<S: Into<String> + Clone + Send>(
        &mut self,
        topics: Vec<(S, QualityOfService)>,
        callback: OnMessageArrivedHandler,
    ) -> impl Future<Output = token::SubscribeToken> + Send;

    fn unsubscribe<S: Into<String> + Send>(
        &mut self,
        topics: Vec<S>,
    ) -> impl Future<Output = token::UnsubscribeToken> + Send;
}
