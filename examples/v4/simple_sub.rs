use std::{env, time::Duration};

use mesquitte_client_v4::{
    client::ClientV4, message::Message, options::ClientOptions, transport, Client,
};
use mqtt_codec_kit::common::QualityOfService;

fn handler1(msg: &Message) {
    log::info!(
        "message from handler1, topic: {}, payload: {:?}, qos: {:?}, retain: {}, dup: {}",
        msg.topic(),
        String::from_utf8(msg.payload().to_vec()),
        msg.qos(),
        msg.retain(),
        msg.dup()
    );
}

fn handler2(msg: &Message) {
    log::info!(
        "message from handler2, topic: {}, payload: {:?}, qos: {:?}, retain: {}, dup: {}",
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
        .set_client_id("tcp-sub-client-v4")
        .set_keep_alive(Duration::from_secs(10))
        .set_auto_reconnect(true)
        .set_connect_retry_interval(Duration::from_secs(10));

    let mut cli = ClientV4::new(options, transport);

    let token = cli.connect().await;
    let err = token.await;
    if err.is_some() {
        panic!("{:#?}", err.unwrap());
    }

    let token = cli
        .subscribe("a/#", QualityOfService::Level0, handler1)
        .await;
    let err = token.await;
    if err.is_some() {
        println!("{:#?}", err.unwrap());
    }

    // conflict subscription with a/b
    let token = cli
        .subscribe("a/b", QualityOfService::Level1, handler1)
        .await;
    let err = token.await;
    if err.is_some() {
        println!("{:#?}", err.unwrap());
    }

    let token = cli
        .subscribe("b/#", QualityOfService::Level1, handler2)
        .await;
    let err = token.await;
    if err.is_some() {
        println!("{:#?}", err.unwrap());
    }

    cli.block().await;
}
