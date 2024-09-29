use futures::{SinkExt, StreamExt};
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use parking_lot::Mutex;
use tokio::{
    net::{
        lookup_host,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::{mpsc, Notify},
    time::{self, timeout},
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
    net::{keep_alive, read_from_server, write_to_server},
    options::ClientOptions,
    state::State,
    token::{
        ConnectToken, DisconnectToken, PacketAndToken, PublishToken, SubscribeToken, Token,
        Tokenize, UnsubscribeToken,
    },
    topic_store::{OnMessageArrivedHandler, Subscription},
    Client,
};

#[derive(Clone, Copy, PartialEq)]
enum ConnectStatus {
    Connected,
    Disconnected,
    Connecting,
}

struct Network {
    pub frame_reader: FramedRead<OwnedReadHalf, MqttDecoder>,
    pub frame_writer: FramedWrite<OwnedWriteHalf, MqttEncoder>,
}

impl Network {
    async fn connect(addr: SocketAddr, connect_timeout: Duration) -> Result<Self, MqttError> {
        match timeout(connect_timeout, TcpStream::connect(addr)).await {
            Ok(res) => match res {
                Ok(s) => {
                    let (rd, wr) = s.into_split();
                    Ok(Self {
                        frame_reader: FramedRead::new(rd, MqttDecoder::new()),
                        frame_writer: FramedWrite::new(wr, MqttEncoder::new()),
                    })
                }
                Err(e) => Err(MqttError::IOError(e)),
            },
            Err(_) => Err(MqttError::ConnectTimeout),
        }
    }
}

pub struct TcpClient {
    options: ClientOptions,
    state: Arc<State>,
    connect_status: Arc<Mutex<ConnectStatus>>,
    notify: Arc<Notify>,

    manual_disconnect: Arc<AtomicBool>,
}

impl TcpClient {
    pub fn new(options: ClientOptions) -> Self {
        Self {
            options,
            state: Arc::new(State::new()),
            connect_status: Arc::new(Mutex::new(ConnectStatus::Disconnected)),
            notify: Arc::new(Notify::new()),

            manual_disconnect: Arc::new(AtomicBool::new(false)),
        }
    }

    fn connect_status(&self) -> ConnectStatus {
        let status = self.connect_status.lock();
        *status
    }

    fn set_connect_status(&mut self, status: ConnectStatus) {
        let mut connect_status = self.connect_status.lock();
        *connect_status = status;
    }

    pub fn is_connected(&self) -> bool {
        let status = self.connect_status.lock();

        *status == ConnectStatus::Connected
    }

    fn build_connect_packet(&self) -> ConnectPacket {
        let mut connect = ConnectPacket::new(self.options.client_id());
        connect.set_clean_session(self.options.clean_session());
        connect.set_keep_alive(self.options.keep_alive().as_secs() as u16);

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

    pub async fn block(&mut self) {
        let mut cnt = 0u16;
        let max_connect_retry_times = self.options.max_connect_retry_times();
        loop {
            if max_connect_retry_times.is_some() && cnt >= max_connect_retry_times.unwrap() {
                log::info!("max connect retry times reached, block exit.",);
                return;
            }
            tokio::select! {
                _ = time::sleep(self.options.connect_retry_interval()) => {
                    match self.connect_status() {
                        ConnectStatus::Connected => continue,
                        ConnectStatus::Disconnected => {
                            if !self.manual_disconnect.load(Ordering::SeqCst) {
                                if self.options.auto_reconnect() {
                                    self.notify.notify_one();
                                } else {
                                    log::info!("client disconnect, program exit.");
                                    return;
                                }
                            } else {
                                continue;
                            }
                        },
                        ConnectStatus::Connecting => continue,
                    }
                },

                _ = self.notify.notified() => {
                    cnt += 1;
                    log::info!("reconnecting @ {cnt}.");
                    let token = self.connect().await;
                    let err = token.clone().await;
                    if err.is_some() {
                        continue;
                    }
                    cnt = 0;
                    log::info!("reconnect success.");
                    if !token.session_present() {
                        log::info!("session present flag is false, start resubscribe job.");
                        {
                            let mut packet_ids = self.state.packet_ids.lock();
                            packet_ids.clean_up();
                        }
                        // resubscribe
                        let subs;
                        {
                            let state = self.state.clone();
                            let subscriptions = state.subscriptions.lock();

                            subs = subscriptions.clone();
                        }
                        self.state.topic_manager.clear();

                        for (topic, sub) in subs {
                            let token = self.subscribe(topic.to_owned(), sub.qos, sub.handler).await;
                            if token.await.is_none() {
                                log::info!("resubscribe topic {} success.", topic);
                            } else {
                                log::error!("resubscribe topic {} failed.", topic);
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Client for TcpClient {
    async fn connect(&mut self) -> ConnectToken {
        let mut token = ConnectToken::default();

        match self.connect_status() {
            ConnectStatus::Connected => {
                token.set_return_code(ConnectReturnCode::ConnectionAccepted);
                return token;
            }
            ConnectStatus::Disconnected => self.set_connect_status(ConnectStatus::Connecting),
            ConnectStatus::Connecting => {
                token.set_error(MqttError::Reconnecting);
                return token;
            }
        }

        let addrs = lookup_host(self.options.server()).await.unwrap();

        let mut network: Option<Network> = None;
        let mut connect_error = MqttError::NetworkUnreachable;

        for addr in addrs {
            match Network::connect(addr, self.options.connect_timeout()).await {
                Ok(n) => {
                    network = Some(n);
                    break;
                }
                Err(e) => {
                    network = None;
                    connect_error = e;
                }
            }
        }

        if network.is_none() {
            self.set_connect_status(ConnectStatus::Disconnected);
            token.set_error(connect_error);
            return token;
        }

        let packet: VariablePacket = self.build_connect_packet().into();

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

                                {
                                    let packet_ids = self.state.packet_ids.lock();
                                    state.packet_ids = (*packet_ids).clone().into();
                                }

                                {
                                    let subscribes = self.state.subscriptions.lock();
                                    state.subscriptions = (*subscribes).clone().into();
                                }

                                state.topic_manager = self.state.topic_manager.clone();
                                state.pending_packets = self.state.pending_packets.clone();

                                state.outgoing_tx = Some(outgoing_tx);

                                self.state = Arc::new(state);

                                let state = self.state.clone();
                                let connect_status = self.connect_status.clone();
                                let keep_alive_duration = self.options.keep_alive();

                                tokio::spawn(async move {
                                    let exit = Arc::new(AtomicBool::new(false));
                                    let (msg_tx, msg_rx) = mpsc::channel(8);

                                    let ping_state = state.clone();
                                    let exit_ping = exit.clone();

                                    tokio::spawn(keep_alive(
                                        keep_alive_duration,
                                        ping_state,
                                        exit_ping,
                                    ));

                                    let mut read_task = tokio::spawn(read_from_server(
                                        network.frame_reader,
                                        msg_tx,
                                    ));

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
                                        log::error!("read_task/write_task terminated.");
                                        read_task.abort();
                                    };

                                    let mut connect_status = connect_status.lock();
                                    *connect_status = ConnectStatus::Disconnected;
                                    exit.store(true, Ordering::SeqCst);
                                });

                                {
                                    let status = self.connect_status.clone();
                                    let mut status = status.lock();
                                    *status = ConnectStatus::Connected;
                                }
                                self.manual_disconnect.store(false, Ordering::SeqCst);

                                if p.connack_flags().session_present {
                                    token.set_session_present();
                                }

                                token.flow_complete();
                            }
                            code => {
                                token.set_return_code(code);
                                self.set_connect_status(ConnectStatus::Disconnected);
                                token.set_error(MqttError::MqttConnectFailed(code.to_u8()))
                            }
                        }
                    } else {
                        self.set_connect_status(ConnectStatus::Disconnected);
                        token.set_error(MqttError::ProtocolError)
                    }
                }
                Err(e) => {
                    self.set_connect_status(ConnectStatus::Disconnected);
                    token.set_error(MqttError::VariablePacketError(e))
                }
            },
            None => {
                self.set_connect_status(ConnectStatus::Disconnected);
                token.set_error(MqttError::MqttConnectFailed(0))
            }
        }

        token
    }

    async fn disconnect(&mut self) -> DisconnectToken {
        let mut token = DisconnectToken::default();

        log::debug!("start disconnect.");

        if !self.is_connected() {
            token.set_error(MqttError::ConnectionLost);
            return token;
        }

        let packet = DisconnectPacket::new().into();

        self.manual_disconnect.store(true, Ordering::SeqCst);

        log::debug!("send disconnect packet.");
        if self
            .state
            .outgoing_tx
            .clone()
            .unwrap()
            .send(PacketAndToken::new_with(
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

        log::debug!("start publish.");

        if !self.is_connected() {
            token.set_error(MqttError::ConnectionLost);
            return token;
        }

        let qos = match qos {
            QualityOfService::Level0 => QoSWithPacketIdentifier::Level0,
            QualityOfService::Level1 => {
                let mut pkids = self.state.packet_ids.lock();
                let pkid = pkids.get_id(Token::Publish(token.clone()));

                if pkid == 0 {
                    token.set_error(MqttError::PacketIdError);
                    return token;
                }

                QoSWithPacketIdentifier::Level1(pkid)
            }
            QualityOfService::Level2 => {
                let mut pkids = self.state.packet_ids.lock();
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

        let packet = packet.into();

        log::debug!("send publish packet.");
        if self
            .state
            .outgoing_tx
            .clone()
            .unwrap()
            .send(PacketAndToken::new_with(
                packet,
                Token::Publish(token.clone()),
            ))
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

        log::debug!("start subscribe.");

        if !self.is_connected() {
            token.set_error(MqttError::ConnectionLost);
            return token;
        }

        let topic: String = topic.into();

        let matched = self.state.topic_manager.match_topic(topic.to_owned());
        if !matched.is_empty() {
            let mut topics = vec![];
            for subscription in matched {
                topics.push(subscription.topic);
            }
            let s: String = topics.join(", ");
            log::warn!(
                "the current topic [{}] conflicts with the existing subscribed topic [{}].",
                topic,
                s
            )
        }

        let pkid;
        {
            let mut pkids = self.state.packet_ids.lock();
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

        let packet = SubscribePacket::new(pkid, subscribes).into();

        log::debug!("send subscribe packet.");
        if self
            .state
            .outgoing_tx
            .clone()
            .unwrap()
            .send(PacketAndToken::new_with(
                packet,
                Token::Subscribe(token.clone()),
            ))
            .await
            .is_err()
        {
            token.set_error(MqttError::InternalChannelError);
        }

        // add subscriptions after send packet to outgoing
        token.add_subscriptions(vec![Subscription::new(topic, qos, callback)]);

        token
    }

    async fn subscribe_multiple<S: Into<String> + Clone + Send>(
        &mut self,
        topics: Vec<(S, QualityOfService)>,
        callback: OnMessageArrivedHandler,
    ) -> SubscribeToken {
        let mut token = SubscribeToken::default();

        log::debug!("start subscribe.");

        if !self.is_connected() {
            token.set_error(MqttError::ConnectionLost);
            return token;
        }

        let pkid;
        {
            let mut pkids = self.state.packet_ids.lock();
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
            subscriptions.push(Subscription::new(topic, qos, callback));
        }

        let packet = SubscribePacket::new(pkid, subscribes).into();

        log::debug!("send subscribe packet.");
        if self
            .state
            .outgoing_tx
            .clone()
            .unwrap()
            .send(PacketAndToken::new_with(
                packet,
                Token::Subscribe(token.clone()),
            ))
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

        log::debug!("start unsubscribe.");

        if !self.is_connected() {
            token.set_error(MqttError::ConnectionLost);
            return token;
        }

        let pkid;
        {
            let mut pkids = self.state.packet_ids.lock();
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

        let packet = UnsubscribePacket::new(pkid, unsubscribes).into();

        if self
            .state
            .outgoing_tx
            .clone()
            .unwrap()
            .send(PacketAndToken::new_with(
                packet,
                Token::Unsubscribe(token.clone()),
            ))
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

    use crate::{client::Client, message::Message};

    use super::{ClientOptions, TcpClient};

    fn handler(msg: &Message) {
        log::debug!(
            "topic: {}, payload: {:?}, qos: {:?}, retain: {}, dup: {}",
            msg.topic(),
            String::from_utf8(msg.payload().to_vec()),
            msg.qos(),
            msg.retain(),
            msg.dup()
        );
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

        cli.block().await;
    }
}
