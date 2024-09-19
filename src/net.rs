use std::{io, sync::Arc};

use futures::{SinkExt, StreamExt};

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
    log::debug!("start read_from_server loop");
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
    // let mut take_over = true;
    log::debug!("start write_to_server loop");
    loop {
        tokio::select! {
            packet = msg_rx.recv() => {
                match packet {
                    Some(packet) => {
                        let resp = match packet {
                            VariablePacket::PublishPacket(packet) => {
                                match handle_publish(&packet, state.clone()) {
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
                                handle_pubrel(packet.packet_identifier())
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
                            if let Some(token) = outgoing.token {
                                match token {
                                    Token::Connect(mut t) => t.set_error(MqttError::IOError(err)),
                                    Token::Disconnect(mut t) => t.set_error(MqttError::IOError(err)),
                                    Token::Publish(mut t) => t.set_error(MqttError::IOError(err)),
                                    Token::Subscribe(mut t) => t.set_error(MqttError::IOError(err)),
                                    Token::Unsubscribe(mut t) => t.set_error(MqttError::IOError(err)),
                                };
                            };
                            log::error!("write outgoing to server: {}", errstr);
                            break;
                        }

                        match outgoing.token {
                            Some(Token::Publish(mut t)) => {
                                if t.qos() == QualityOfService::Level0 {
                                    t.flow_complete()
                                }
                            },
                            Some(Token::Disconnect(mut t)) => {
                                t.flow_complete()
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

    Ok(())
}

fn handle_publish(
    packet: &PublishPacket,
    state: Arc<State>,
) -> Result<Option<VariablePacket>, MqttError> {
    let (qos, pkid) = packet.qos().split();

    let handlers = &state
        .subscribers
        .match_topic(packet.topic_name().to_string());
    for handler in handlers {
        handler(packet.topic_name(), packet.payload(), qos);
    }

    match qos {
        QualityOfService::Level0 => Ok(None),
        QualityOfService::Level1 => {
            let puback = PubackPacket::new(pkid.unwrap());
            Ok(Some(VariablePacket::new(puback)))
        }
        QualityOfService::Level2 => {
            let pubrec = PubrecPacket::new(pkid.unwrap());
            Ok(Some(VariablePacket::new(pubrec)))
        }
    }
}

fn handle_puback(pkid: u16, state: Arc<State>) {
    let mut pkids = state.pkids.lock().unwrap();
    if let Some(Token::Publish(t)) = pkids.get_token(pkid) {
        t.flow_complete();
        pkids.free_id(&pkid);
    }
}

fn handle_pubrec(pkid: u16) -> VariablePacket {
    let pubrel = PubrelPacket::new(pkid);
    VariablePacket::new(pubrel)
}

fn handle_pubrel(pkid: u16) -> VariablePacket {
    let pubcomp = PubcompPacket::new(pkid);
    VariablePacket::new(pubcomp)
}

fn handle_pubcomp(pkid: u16, state: Arc<State>) {
    let mut pkids = state.pkids.lock().unwrap();
    if let Some(Token::Publish(t)) = pkids.get_token(pkid) {
        t.flow_complete();
        pkids.free_id(&pkid);
    }
}

fn handle_suback(packet: &SubackPacket, state: Arc<State>) {
    let mut pkids = state.pkids.lock().unwrap();

    let pkid = packet.packet_identifier();

    if let Some(Token::Subscribe(t)) = pkids.get_token(pkid) {
        for (i, ret) in packet.subscribes().iter().enumerate() {
            let topic = t.set_result(i, *ret);

            let callback = t.get_handler(topic.to_owned()).unwrap();

            if *ret != SubscribeReturnCode::Failure {
                state.subscribers.add((topic, callback));
            }
        }

        t.flow_complete();
        pkids.free_id(&pkid);
    }
}

fn handle_unsuback(pkid: u16, state: Arc<State>) {
    let mut pkids = state.pkids.lock().unwrap();
    if let Some(Token::Unsubscribe(t)) = pkids.get_token(pkid) {
        let topics = t.topics();

        for topic in topics {
            state.subscribers.remove(topic);
        }

        t.flow_complete();
        pkids.free_id(&pkid);
    }
}

fn handle_disconnect() {}

fn handle_pingresp() {}
