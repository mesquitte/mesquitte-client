use futures::{SinkExt, StreamExt};
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use tokio::{
    net::{
        lookup_host,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::mpsc,
};
use tokio_util::codec::{FramedRead, FramedWrite};

use mqtt_codec_kit::{
    common::{qos::QoSWithPacketIdentifier, QualityOfService, TopicFilter, TopicName},
    v4::{
        control::ConnectReturnCode,
        packet::{
            connect::LastWill, ConnectPacket, DisconnectPacket, MqttDecoder, MqttEncoder,
            PublishPacket, SubscribePacket, UnsubscribePacket, VariablePacket,
        },
    },
};

use crate::{
    error::MqttError,
    net::{read_from_server, write_to_server},
    options::ClientOptions,
    state::State,
    token::{
        ConnectToken, DisconnectToken, PacketAndToken, PublishToken, SubscribeToken, Token,
        Tokenize, UnsubscribeToken,
    },
    topic_store::OnMessageArrivedHandler,
    Client,
};

struct Network {
    pub frame_reader: FramedRead<OwnedReadHalf, MqttDecoder>,
    pub frame_writer: FramedWrite<OwnedWriteHalf, MqttEncoder>,
}

impl Network {
    async fn connect(addr: SocketAddr) -> Result<Self, MqttError> {
        match TcpStream::connect(addr).await {
            Ok(s) => {
                let (rd, wr) = s.into_split();
                Ok(Self {
                    frame_reader: FramedRead::new(rd, MqttDecoder::new()),
                    frame_writer: FramedWrite::new(wr, MqttEncoder::new()),
                })
            }
            Err(e) => Err(MqttError::IOError(e)),
        }
    }
}

pub struct TcpClient {
    options: ClientOptions,
    state: Arc<State>,
    connected: Arc<AtomicBool>,
}

impl TcpClient {
    pub fn new(options: ClientOptions) -> Self {
        Self {
            options,
            state: Arc::new(State::new()),
            connected: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn build_connect_packet(&self) -> ConnectPacket {
        let mut connect = ConnectPacket::new(self.options.client_id());
        connect.set_clean_session(self.options.clean_session());

        if self.options.will_enabled() {
            connect.set_will_qos(self.options.will_qos() as u8);
            connect.set_will_retain(self.options.will_retained());

            let will_msg = LastWill::new(
                self.options.will_topic(),
                self.options.will_payload().to_vec(),
            );

            connect.set_will(Some(will_msg.unwrap()));
        }

        if !self.options.username().is_empty() {
            connect.set_username(Some(self.options.username().into()));
        }
        if !self.options.password().is_empty() {
            connect.set_password(Some(self.options.password().into()));
        }

        connect
    }
}

impl Client for TcpClient {
    async fn connect(&mut self) -> ConnectToken {
        let addrs = lookup_host(self.options.server()).await.unwrap();

        let mut network: Option<Network> = None;

        let mut token = ConnectToken::default();

        for addr in addrs {
            match Network::connect(addr).await {
                Ok(n) => {
                    network = Some(n);
                    break;
                }
                Err(_) => network = None,
            }
        }

        if network.is_none() {
            token.set_error(MqttError::NetworkUnreachable);
            return token;
        }

        let packet = VariablePacket::new(self.build_connect_packet());

        let mut network = network.unwrap();

        let _ = network.frame_writer.send(packet).await;

        match network.frame_reader.next().await {
            Some(packet) => match packet {
                Ok(packet) => {
                    if let VariablePacket::ConnackPacket(p) = packet {
                        match p.connect_return_code() {
                            ConnectReturnCode::ConnectionAccepted => {
                                let (outgoing_tx, outgoing_rx) = mpsc::channel(8);

                                let mut state = State::new();

                                state.outgoing_tx = Some(outgoing_tx);

                                self.state = Arc::new(state);

                                let state = self.state.clone();

                                let connected = self.connected.clone();

                                tokio::spawn(async move {
                                    let (msg_tx, msg_rx) = mpsc::channel(8);
                                    let mut read_task = tokio::spawn(async move {
                                        read_from_server(network.frame_reader, msg_tx).await;
                                    });

                                    let mut write_task = tokio::spawn(async move {
                                        if let Err(err) = write_to_server(
                                            network.frame_writer,
                                            msg_rx,
                                            outgoing_rx,
                                            state,
                                        )
                                        .await
                                        {
                                            log::error!("write to server: {err}");
                                        }
                                    });

                                    if tokio::try_join!(&mut read_task, &mut write_task).is_err() {
                                        log::error!("read_task/write_task terminated");
                                        read_task.abort();
                                    };

                                    connected.store(false, Ordering::SeqCst);
                                });

                                self.connected.store(true, Ordering::SeqCst);

                                if p.connack_flags().session_present {
                                    token.set_session_present();
                                }

                                token.flow_complete();
                            }
                            code => {
                                token.set_return_code(code);
                                token.set_error(MqttError::MqttConnectFailed(code.to_u8()))
                            }
                        }
                    } else {
                        token.set_error(MqttError::ProtocolError)
                    }
                }
                Err(e) => token.set_error(MqttError::VariablePacketError(e)),
            },
            None => token.set_error(MqttError::MqttConnectFailed(0)),
        }

        token
    }

    async fn disconnect(&mut self) -> DisconnectToken {
        let mut token = DisconnectToken::default();

        log::debug!("start disconnect");

        if !self.connected.load(Ordering::SeqCst) {
            token.set_error(MqttError::ConnectionLost);
            return token;
        }

        let packet = VariablePacket::new(DisconnectPacket::new());

        log::debug!("send disconnect packet");
        if self
            .state
            .outgoing_tx
            .clone()
            .unwrap()
            .send(PacketAndToken::new(
                packet,
                Token::Disconnect(token.clone()),
            ))
            .await
            .is_err()
        {
            token.set_error(MqttError::InternalChannelError);
        }

        token
    }

    async fn publish<S, V>(
        &mut self,
        topic: S,
        qos: QualityOfService,
        retained: bool,
        payload: V,
    ) -> PublishToken
    where
        S: Into<String> + Send,
        V: Into<Vec<u8>> + Send,
    {
        let mut token = PublishToken::default();
        token.set_qos(qos);

        log::debug!("start publish");

        if !self.connected.load(Ordering::SeqCst) {
            token.set_error(MqttError::ConnectionLost);
            return token;
        }

        let qos = match qos {
            QualityOfService::Level0 => QoSWithPacketIdentifier::Level0,
            QualityOfService::Level1 => {
                let mut pkids = self.state.pkids.lock();
                let pkid = pkids.get_id(Token::Publish(token.clone()));

                if pkid == 0 {
                    token.set_error(MqttError::PacketIdError);
                    return token;
                }

                QoSWithPacketIdentifier::Level1(pkid)
            }
            QualityOfService::Level2 => {
                let mut pkids = self.state.pkids.lock();
                let pkid = pkids.get_id(Token::Publish(token.clone()));

                if pkid == 0 {
                    token.set_error(MqttError::PacketIdError);
                    return token;
                }

                QoSWithPacketIdentifier::Level2(pkid)
            }
        };

        let topic: String = topic.into();

        let topic_name = match TopicName::new(topic.to_owned()) {
            Ok(tn) => tn,
            Err(_) => {
                token.set_error(MqttError::InvalidTopic);
                return token;
            }
        };

        let mut packet = PublishPacket::new(topic_name, qos, payload);
        packet.set_retain(retained);

        let packet = VariablePacket::new(packet);

        log::debug!("send publish packet");
        if self
            .state
            .outgoing_tx
            .clone()
            .unwrap()
            .send(PacketAndToken::new(packet, Token::Publish(token.clone())))
            .await
            .is_err()
        {
            token.set_error(MqttError::InternalChannelError);
        }

        token
    }

    async fn subscribe<S: Into<String> + Send>(
        &mut self,
        topic: S,
        qos: QualityOfService,
        callback: OnMessageArrivedHandler,
    ) -> SubscribeToken {
        let mut token = SubscribeToken::default();

        log::debug!("start subscribe");

        if !self.connected.load(Ordering::SeqCst) {
            token.set_error(MqttError::ConnectionLost);
            return token;
        }

        let topic: String = topic.into();

        let pkid;
        {
            let mut pkids = self.state.pkids.lock();
            pkid = pkids.get_id(Token::Subscribe(token.clone()));
        }

        if pkid == 0 {
            token.set_error(MqttError::PacketIdError);
            return token;
        }

        let topic_filter = match TopicFilter::new(topic.to_owned()) {
            Ok(tf) => tf,
            Err(_) => {
                token.set_error(MqttError::InvalidTopic);
                return token;
            }
        };

        let subscribes = vec![(topic_filter, qos)];

        let packet = VariablePacket::new(SubscribePacket::new(pkid, subscribes));

        log::debug!("send subscribe packet");
        if self
            .state
            .outgoing_tx
            .clone()
            .unwrap()
            .send(PacketAndToken {
                packet,
                token: Some(Token::Subscribe(token.clone())),
            })
            .await
            .is_err()
        {
            token.set_error(MqttError::InternalChannelError);
        }

        // add subscriptions after send packet to outgoing
        token.add_subscriptions(vec![(topic.to_owned(), callback)]);

        token
    }

    async fn subscribe_multiple<S: Into<String> + Clone + Send>(
        &mut self,
        topics: Vec<(S, QualityOfService)>,
        callback: OnMessageArrivedHandler,
    ) -> SubscribeToken {
        let mut token = SubscribeToken::default();

        log::debug!("start subscribe");

        if !self.connected.load(Ordering::SeqCst) {
            token.set_error(MqttError::ConnectionLost);
            return token;
        }

        let pkid;
        {
            let mut pkids = self.state.pkids.lock();
            pkid = pkids.get_id(Token::Subscribe(token.clone()));
        }

        if pkid == 0 {
            token.set_error(MqttError::PacketIdError);
            return token;
        }

        let mut subscribes = vec![];
        let mut subscriptions = vec![];

        for (topic, qos) in topics {
            let topic: String = topic.into();

            let topic_filter = match TopicFilter::new(topic.to_owned()) {
                Ok(tf) => tf,
                Err(_) => {
                    token.set_error(MqttError::InvalidTopic);
                    return token;
                }
            };

            subscribes.push((topic_filter, qos));

            subscriptions.push((topic, callback));
        }

        let packet = VariablePacket::new(SubscribePacket::new(pkid, subscribes));

        log::debug!("send subscribe packet");
        if self
            .state
            .outgoing_tx
            .clone()
            .unwrap()
            .send(PacketAndToken {
                packet,
                token: Some(Token::Subscribe(token.clone())),
            })
            .await
            .is_err()
        {
            token.set_error(MqttError::InternalChannelError);
        }

        // add subscriptions after send packet to outgoing
        token.add_subscriptions(subscriptions);

        token
    }

    async fn unsubscribe<S: Into<String> + Send>(&mut self, topics: Vec<S>) -> UnsubscribeToken {
        let mut token = UnsubscribeToken::default();

        log::debug!("start unsubscribe");

        if !self.connected.load(Ordering::SeqCst) {
            token.set_error(MqttError::ConnectionLost);
            return token;
        }

        let pkid;
        {
            let mut pkids = self.state.pkids.lock();
            pkid = pkids.get_id(Token::Unsubscribe(token.clone()));
        }

        if pkid == 0 {
            token.set_error(MqttError::PacketIdError);
            return token;
        }

        let mut unsubscribes = vec![];
        for topic in topics {
            let topic: String = topic.into();

            let topic_filter = match TopicFilter::new(topic.to_owned()) {
                Ok(tf) => tf,
                Err(_) => {
                    token.set_error(MqttError::InvalidTopic);
                    return token;
                }
            };

            unsubscribes.push(topic_filter);
            token.add_topic(topic);
        }

        let packet = VariablePacket::new(UnsubscribePacket::new(pkid, unsubscribes));

        if self
            .state
            .outgoing_tx
            .clone()
            .unwrap()
            .send(PacketAndToken {
                packet,
                token: Some(Token::Unsubscribe(token.clone())),
            })
            .await
            .is_err()
        {
            token.set_error(MqttError::InternalChannelError);
        }

        token
    }
}

#[cfg(test)]
mod test {
    use std::env;

    use mqtt_codec_kit::common::QualityOfService;
    use tokio::signal;

    use crate::client::Client;

    use super::{ClientOptions, TcpClient};

    fn handler(topic: &str, payload: &[u8], qos: QualityOfService) {
        log::debug!("topic: {topic}, payload: {:?}, qos: {:?}", payload, qos);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tcp_client() {
        env::set_var("RUST_LOG", "debug");
        env_logger::init();

        let mut options = ClientOptions::new();
        options
            .set_server("localhost:1883")
            .set_client_id("test_qx")
            .set_username("qinxin")
            .set_password("111");

        let mut cli = TcpClient::new(options);

        let token = cli.connect().await;

        let err = token.await;
        if err.is_some() {
            println!("{:#?}", err.unwrap());
        }

        let binding = Vec::from("hello, world!");
        let payload = binding.as_slice();

        let token = cli
            .subscribe("sport/football", QualityOfService::Level0, handler)
            .await;
        let err = token.await;
        if err.is_some() {
            println!("{:#?}", err.unwrap());
        }

        let token = cli
            .subscribe("sport/basketball", QualityOfService::Level0, handler)
            .await;
        let err = token.await;
        if err.is_some() {
            println!("{:#?}", err.unwrap());
        }

        let token = cli
            .publish("sport/football", QualityOfService::Level2, false, payload)
            .await;
        let err = token.await;
        if err.is_some() {
            println!("{:#?}", err.unwrap());
        }

        let token = cli.unsubscribe(vec!["sport/basketball"]).await;
        let err = token.await;
        if err.is_some() {
            println!("{:#?}", err.unwrap());
        }

        signal::ctrl_c().await.expect("ctrl c failed");
    }
}
