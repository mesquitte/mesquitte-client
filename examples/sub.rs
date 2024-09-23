use std::env;

use mesquitte_client::{client::TcpClient, message::Message, options::ClientOptions, Client};
use mqtt_codec_kit::common::QualityOfService;
use tokio::signal;

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

    let mut options = ClientOptions::new();
    options
        .set_server("localhost:1883")
        .set_client_id("simple-client");

    let mut cli = TcpClient::new(options);

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

    let token = cli
        .subscribe("b/#", QualityOfService::Level0, handler2)
        .await;
    let err = token.await;
    if err.is_some() {
        println!("{:#?}", err.unwrap());
    }

    signal::ctrl_c().await.expect("ctrl c failed");
}
