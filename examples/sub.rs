use std::env;

use mesquitte_client::{client::TcpClient, options::ClientOptions, Client};
use mqtt_codec_kit::common::QualityOfService;
use tokio::signal;

fn handler1(topic: &str, payload: &[u8], qos: QualityOfService) {
    log::info!(
        "message from hander1: topic: {topic}, payload: {:?}, qos: {:?}",
        payload,
        qos
    );
}

fn handler2(topic: &str, payload: &[u8], qos: QualityOfService) {
    log::info!(
        "message from hander2: topic: {topic}, payload: {:?}, qos: {:?}",
        payload,
        qos
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
