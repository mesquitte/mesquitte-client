use futures::{SinkExt, StreamExt};
use std::{io, sync::Arc};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc,
};
use tokio_util::codec::{Decoder, Encoder, FramedRead, FramedWrite};

use mqtt_codec_kit::{
    common::QualityOfService,
    v4::packet::{
        suback::SubscribeReturnCode, PubackPacket, PubcompPacket, PublishPacket, PubrecPacket,
        PubrelPacket, SubackPacket, VariablePacket, VariablePacketError,
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
    log::info!("start read loop");
    loop {
        match reader.next().await {
            None => {
                log::info!("connection closed");
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
    log::info!("read loop exited");
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
    log::info!("write loop exited");

    Ok(())
}

async fn handle_publish(
    packet: &PublishPacket,
    state: Arc<State>,
) -> Result<Option<VariablePacket>, MqttError> {
    let (qos, pkid) = packet.qos().split();

    let handlers = &state
        .topic_manager
        .match_topic(packet.topic_name().to_string());

    match qos {
        QualityOfService::Level0 => {
            // process message via callback
            let packet: Message = packet.into();

            for handler in handlers {
                let handler = *handler;
                let packet = packet.clone();
                tokio::spawn(async move {
                    handler(&packet);
                });
            }

            Ok(None)
        }
        QualityOfService::Level1 => {
            // process message via callback
            let packet: Message = packet.into();

            for handler in handlers {
                let handler = *handler;
                let packet = packet.clone();
                tokio::spawn(async move {
                    handler(&packet);
                });
            }

            let puback = PubackPacket::new(pkid.unwrap());
            Ok(Some(VariablePacket::new(puback)))
        }
        QualityOfService::Level2 => {
            let pkid = pkid.unwrap();
            // store message to pending_packets
            if state.pending_packets.contains_key(&pkid) {
                log::debug!("received duplicate message: packet_id {}", pkid);
            }
            state.pending_packets.insert(pkid, packet.clone()).await;

            let pubrec = PubrecPacket::new(pkid);
            Ok(Some(VariablePacket::new(pubrec)))
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
    let pubrel = PubrelPacket::new(pkid);
    VariablePacket::new(pubrel)
}

async fn handle_pubrel(pkid: u16, state: Arc<State>) -> VariablePacket {
    let packet = state.pending_packets.remove(&pkid).await;

    match packet {
        Some(packet) => {
            let handlers = &state
                .topic_manager
                .match_topic(packet.topic_name().to_string());

            let msg = Message::from(&packet);

            // process message via callback
            for handler in handlers {
                let handler = *handler;
                let msg = msg.clone();
                tokio::spawn(async move {
                    handler(&msg);
                });
            }
        }
        None => log::error!("packet id {} not found.", pkid),
    }

    let pubcomp = PubcompPacket::new(pkid);
    VariablePacket::new(pubcomp)
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
        for (i, code) in packet.subscribes().iter().enumerate() {
            let topic = token.set_result(i, *code);

            let callback = token.get_handler(topic.to_owned()).unwrap();

            if *code != SubscribeReturnCode::Failure {
                state.topic_manager.add((topic, callback));
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
