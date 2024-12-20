use std::{env, time::Duration};

use mesquitte_client_v5::{
    client::ClientV5, message::Message, options::ClientOptions, transport, Client,
};
use mqtt_codec_kit::{
    common::QualityOfService,
    v5::{control::SubscribeProperties, packet::subscribe::SubscribeOptions},
};

fn handler1(msg: &Message) {
    log::info!(
        "message from handler1, subscription_identifier: {:?}, topic: {}, payload: {:?}, qos: {:?}, retain: {}, dup: {}",
        msg.properties().subscription_identifier(),
        msg.topic(),
        String::from_utf8(msg.payload().to_vec()),
        msg.qos(),
        msg.retain(),
        msg.dup()
    );
}

fn handler2(msg: &Message) {
    log::info!(
        "message from handler2, subscription_identifier: {:?}, topic: {}, payload: {:?}, qos: {:?}, retain: {}, dup: {}",
        msg.properties().subscription_identifier(),
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

    let transport = transport::Tcp {};

    let mut options = ClientOptions::new();
    options
        .set_server("localhost:1883")
        .set_client_id("tcp-sub-client-v5")
        .set_keep_alive(Duration::from_secs(10))
        .set_auto_reconnect(true)
        .set_connect_retry_interval(Duration::from_secs(10));

    let mut cli = ClientV5::new(options, transport);

    let token = cli.connect(None).await;
    let err = token.await;
    if err.is_some() {
        panic!("{:#?}", err.unwrap());
    }

    // conflict subscription with a/b
    let mut handler1_properties = SubscribeProperties::default();
    handler1_properties.set_identifier(Some(1));

    let qos0_options = SubscribeOptions::default();
    let token = cli
        .subscribe(
            "a/#",
            qos0_options,
            Some(handler1_properties.clone()),
            handler1,
        )
        .await;
    let err = token.await;
    if err.is_some() {
        println!("{:#?}", err.unwrap());
    }

    let mut qos1_options = SubscribeOptions::default();
    qos1_options.set_qos(QualityOfService::Level1);
    let token = cli.subscribe("a/b", qos1_options, None, handler2).await;
    let err = token.await;
    if err.is_some() {
        println!("{:#?}", err.unwrap());
    }

    let mut qos2_options = SubscribeOptions::default();
    qos2_options.set_qos(QualityOfService::Level2);
    let token = cli
        .subscribe("b/#", qos2_options, Some(handler1_properties), handler2)
        .await;
    let err = token.await;
    if err.is_some() {
        println!("{:#?}", err.unwrap());
    }

    cli.block().await;
}
