use std::{env, time::Duration};

use mesquitte_client_v5::{
    client::ClientV5, message::Message, options::ClientOptions, transport, Client,
};
use mqtt_codec_kit::{common::QualityOfService, v5::packet::subscribe::SubscribeOptions};

fn handler(msg: &Message) {
    log::info!(
        "topic: {}, payload: {:?}, qos: {:?}, retain: {}, dup: {}",
        msg.topic(),
        String::from_utf8(msg.payload().to_vec()),
        msg.qos(),
        msg.retain(),
        msg.dup()
    );
}

#[tokio::main]
async fn main() {
    env::set_var("RUST_LOG", "info");
    env_logger::init();

    let transport = transport::Ws {};

    let mut options = ClientOptions::new();
    options
        .set_server("ws://localhost:8083/mqtt")
        .set_client_id("ws-client-v5")
        .set_keep_alive(Duration::from_secs(10))
        .set_auto_reconnect(true)
        .set_connect_retry_interval(Duration::from_secs(10));

    let mut cli = ClientV5::new(options, transport);

    let token = cli.connect(None).await;
    let err = token.await;
    if err.is_some() {
        panic!("{:#?}", err.unwrap());
    }

    let subscribe_options = SubscribeOptions::default();

    let token = cli
        .subscribe("test/topic", subscribe_options, None, handler)
        .await;
    let err = token.await;
    if err.is_some() {
        println!("{:#?}", err.unwrap());
    }

    let binding = Vec::from("hello, world!");
    let payload = binding.as_slice();

    let token = cli
        .publish("test/topic", QualityOfService::Level0, false, payload, None)
        .await;
    let err = token.await;
    if err.is_some() {
        println!("{:#?}", err.unwrap());
    }

    cli.block().await;
}
