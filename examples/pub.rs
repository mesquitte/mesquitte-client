use mesquitte_client::{client::TcpClient, options::ClientOptions, Client};
use mqtt_codec_kit::common::QualityOfService;

#[tokio::main]
async fn main() {
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

    let binding = Vec::from("hello, world!");
    let payload = binding.as_slice();

    let token = cli
        .publish("test/topic", QualityOfService::Level0, false, payload)
        .await;
    let err = token.await;
    if err.is_some() {
        println!("{:#?}", err.unwrap());
    }
}
