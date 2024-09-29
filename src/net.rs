use futures::{SinkExt, StreamExt};
use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc,
    time::{sleep, sleep_until, Instant},
};
use tokio_util::codec::{Decoder, Encoder, FramedRead, FramedWrite};

use mqtt_codec_kit::{
    common::QualityOfService,
    v4::packet::{
        suback::SubscribeReturnCode, PingreqPacket, PubackPacket, PubcompPacket, PublishPacket,
        PubrecPacket, PubrelPacket, SubackPacket, VariablePacket, VariablePacketError,
    },
};

use crate::{
    error::MqttError,
    message::Message,
    state::State,
    token::{PacketAndToken, Token, Tokenize},
};

pub async fn read_from_server<R, D>(
    mut reader: FramedRead<R, D>,
    msg_tx: mpsc::Sender<VariablePacket>,
) where
    R: AsyncRead + Unpin,
    D: Decoder<Item = VariablePacket, Error = VariablePacketError>,
{
    log::info!("start read loop.");
    loop {
        match reader.next().await {
            None => {
                log::info!("connection closed.");
                break;
            }
            Some(Err(e)) => {
                log::warn!("read from server: {}", e);
                break;
            }
            Some(Ok(packet)) => {
                log::debug!("read from server: {:?}", packet);
                if let Err(err) = msg_tx.send(packet).await {
                    log::error!("receiver closed: {}", err);
                    break;
                }
            }
        }
    }
    log::info!("read loop exited.");
}

pub async fn write_to_server<W, E>(
    mut writer: FramedWrite<W, E>,
    mut msg_rx: mpsc::Receiver<VariablePacket>,
    mut outgoing_rx: mpsc::Receiver<PacketAndToken>,
    state: Arc<State>,
) -> Result<(), MqttError>
where
    W: AsyncWrite + Unpin,
    E: Encoder<VariablePacket, Error = io::Error>,
{
    log::info!("start write loop");
    loop {
        tokio::select! {
            packet = msg_rx.recv() => {
                match packet {
                    Some(packet) => {
                        let resp = match packet {
                            VariablePacket::PublishPacket(packet) => {
                                match handle_publish(&packet, state.clone()).await {
                                    Ok(Some(resp)) => resp,
                                    Ok(None) => continue,
                                    Err(err) => {
                                        log::error!("handle publish message failed: {}", err);
                                        break;
                                    }
                                }
                            }
                            VariablePacket::PubackPacket(packet) => {
                                handle_puback(packet.packet_identifier(), state.clone());
                                continue;
                            }
                            VariablePacket::PubrecPacket(packet) => {
                                handle_pubrec(packet.packet_identifier())
                            }
                            VariablePacket::PubrelPacket(packet) => {
                                handle_pubrel(packet.packet_identifier(), state.clone()).await
                            }
                            VariablePacket::PubcompPacket(packet) => {
                                handle_pubcomp(packet.packet_identifier(), state.clone());
                                continue;
                            }
                            VariablePacket::SubackPacket(packet) => {
                                handle_suback(&packet, state.clone());
                                continue;
                            }
                            VariablePacket::UnsubackPacket(packet) => {
                                handle_unsuback(packet.packet_identifier(), state.clone());
                                continue;
                            }
                            VariablePacket::DisconnectPacket(_packet) => {
                                handle_disconnect();
                                break;
                            }
                            VariablePacket::PingrespPacket(_packet) => {
                                handle_pingresp();
                                continue;
                            }
                            _ => {
                                log::debug!("unsupported packet: {:?}", packet);
                                break;
                            }
                        };

                        log::debug!("write response packet {:?}", resp);

                        if let Err(err) = writer.send(resp).await {
                            log::error!("write to server {}", err);
                            break;
                        }

                        {
                            let mut last_sent_packet_at = state.last_sent_packet_at.lock();
                            *last_sent_packet_at = Instant::now();
                        }
                    }
                    None => {
                        log::warn!("incoming receive channel closed");
                        break;
                    }
                }
            }
            outgoing = outgoing_rx.recv() => {
                match outgoing {
                    Some(outgoing) => {
                        let packet = outgoing.packet;

                        log::debug!("write outgoing packet {:?}", packet);

                        if let Err(err) = writer.send(packet).await {
                            let errstr = err.to_string();
                            if let Some(mut token) = outgoing.token {
                                token.set_error(MqttError::IOError(err));
                            };
                            log::error!("write outgoing to server: {}", errstr);
                            break;
                        }

                        {
                            let mut last_sent_packet_at = state.last_sent_packet_at.lock();
                            *last_sent_packet_at = Instant::now();
                        }

                        match outgoing.token {
                            Some(Token::Publish(mut token)) => {
                                if token.qos() == QualityOfService::Level0 {
                                    token.flow_complete()
                                }
                            },
                            Some(Token::Disconnect(mut token)) => {
                                token.flow_complete()
                            }
                            _ => {},
                        };
                    }
                    None => {
                        log::warn!("outgoing receive channel closed");
                        break;
                    }
                }
            }
        }
    }
    log::info!("write loop exited.");

    Ok(())
}

pub async fn keep_alive(duration: Duration, state: Arc<State>, exit: Arc<AtomicBool>) {
    log::info!("start keep_alive loop.");
    while !exit.load(Ordering::SeqCst) {
        let elapsed;
        {
            let last_sent_packet_at = state.last_sent_packet_at.lock();
            let now = Instant::now();
            elapsed = now.duration_since(*last_sent_packet_at);
        }

        if elapsed >= duration {
            let packet = PingreqPacket::new().into();
            log::debug!("send pingreq packet.");
            if state
                .outgoing_tx
                .clone()
                .unwrap()
                .send(PacketAndToken::new(packet))
                .await
                .is_err()
            {
                log::error!("send pingreq packet error");
            }
        } else {
            let remaining = duration - elapsed;
            let next_execution = Instant::now() + remaining;
            sleep_until(next_execution).await;
            continue;
        }

        sleep(duration).await;
    }
    log::info!("keep_alive loop exited.");
}

async fn handle_publish(
    packet: &PublishPacket,
    state: Arc<State>,
) -> Result<Option<VariablePacket>, MqttError> {
    let (qos, pkid) = packet.qos().split();

    let subscriptions = &state
        .topic_manager
        .match_topic(packet.topic_name().to_string());

    match qos {
        QualityOfService::Level0 => {
            let msg: Message = packet.into();

            for subscription in subscriptions {
                let subscription = subscription.clone();
                let msg = msg.clone();

                tokio::spawn(async move {
                    (subscription.handler)(&msg);
                });
            }

            Ok(None)
        }
        QualityOfService::Level1 => {
            let msg: Message = packet.into();

            for subscription in subscriptions {
                let subscription = subscription.clone();
                let msg = msg.clone();

                tokio::spawn(async move {
                    (subscription.handler)(&msg);
                });
            }

            Ok(Some(PubackPacket::new(pkid.unwrap()).into()))
        }
        QualityOfService::Level2 => {
            let pkid = pkid.unwrap();
            // store message to pending_packets
            if state.pending_packets.contains_key(&pkid) {
                log::debug!("received duplicate message: packet_id {}", pkid);
            }
            state.pending_packets.insert(pkid, packet.clone()).await;

            Ok(Some(PubrecPacket::new(pkid).into()))
        }
    }
}

fn handle_puback(pkid: u16, state: Arc<State>) {
    let mut pkids = state.packet_ids.lock();

    if let Some(token) = pkids.get_token(pkid) {
        token.flow_complete();
        pkids.free_id(&pkid);
    }
}

fn handle_pubrec(pkid: u16) -> VariablePacket {
    PubrelPacket::new(pkid).into()
}

async fn handle_pubrel(pkid: u16, state: Arc<State>) -> VariablePacket {
    let packet = state.pending_packets.remove(&pkid).await;

    match packet {
        Some(packet) => {
            let subscriptions = &state
                .topic_manager
                .match_topic(packet.topic_name().to_string());

            let msg = Message::from(&packet);

            for subscription in subscriptions {
                let subscription = subscription.clone();
                let msg = msg.clone();

                tokio::spawn(async move {
                    (subscription.handler)(&msg);
                });
            }
        }
        None => log::error!("packet id {} not found.", pkid),
    }

    PubcompPacket::new(pkid).into()
}

fn handle_pubcomp(pkid: u16, state: Arc<State>) {
    let mut pkids = state.packet_ids.lock();

    if let Some(token) = pkids.get_token(pkid) {
        token.flow_complete();
        pkids.free_id(&pkid);
    }
}

fn handle_suback(packet: &SubackPacket, state: Arc<State>) {
    let mut pkids = state.packet_ids.lock();

    let pkid = packet.packet_identifier();

    if let Some(Token::Subscribe(token)) = pkids.get_token(pkid) {
        let mut subscriptions = state.subscriptions.lock();

        for (i, code) in packet.subscribes().iter().enumerate() {
            token.set_result(i, *code);

            match token.get_subscription(i) {
                Some(subscription) => {
                    if *code != SubscribeReturnCode::Failure {
                        state.topic_manager.add(subscription.clone());
                        subscriptions.insert(subscription.topic.to_owned(), subscription);
                    }
                }
                None => log::warn!("subscription {} not found.", i),
            }
        }

        token.flow_complete();
        pkids.free_id(&pkid);
    }
}

fn handle_unsuback(pkid: u16, state: Arc<State>) {
    let mut pkids = state.packet_ids.lock();

    if let Some(Token::Unsubscribe(token)) = pkids.get_token(pkid) {
        let topics = token.topics();

        for topic in topics {
            state.topic_manager.remove(topic);
        }

        token.flow_complete();
        pkids.free_id(&pkid);
    }
}

fn handle_disconnect() {}

fn handle_pingresp() {}
