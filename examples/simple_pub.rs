use mesquitte_client::{client::MqttClient, options::ClientOptions, transport, Client};
use mqtt_codec_kit::common::QualityOfService;

#[tokio::main]
async fn main() {
    let transport = transport::Tcp {};

    let mut options = ClientOptions::new();
    options
        .set_server("localhost:1883")
        .set_client_id("tcp-pub-client")
        .set_auto_reconnect(false);

    let mut cli = MqttClient::new(options, transport);

    let token = cli.connect().await;
    let err = token.await;
    if err.is_some() {
        panic!("{:#?}", err.unwrap());
    }

    let binding = Vec::from("hello, world!");
    let payload = binding.as_slice();

    let token = cli
        .publish("a/topic", QualityOfService::Level0, false, payload)
        .await;
    let err = token.await;
    if err.is_some() {
        println!("{:#?}", err.unwrap());
    }

    // cli.block().await
}
