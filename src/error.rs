#[derive(Debug, thiserror::Error)]
pub(crate) enum MqttError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("Network Unreachable")]
    NetworkUnreachable,
    #[error("connect failed, return code: ({0})")]
    MqttConnectFailed(u8),
    #[error("Connection Lost")]
    ConnectionLost,
    #[error("Protocol Error")]
    ProtocolError,
    #[error("Invalid Topic")]
    InvalidTopic,
    #[error("No Packet ID Available")]
    PacketIdError,
    #[error("Internal Channel Error")]
    InternalChannelError,
    #[error(transparent)]
    VariablePacketError(#[from] mqtt_codec_kit::v4::packet::VariablePacketError),
}

#[derive(Debug)]
pub enum TokenError {
    IOError(String),
    NotConnect,
    MqttConnectFailed(u8),
    ConnectionLost,
    InvalidTopic,
    PacketIdError,
    InternalChannelError,
    PacketError(String),
}

impl From<&MqttError> for TokenError {
    fn from(value: &MqttError) -> Self {
        match value {
            MqttError::IOError(e) => TokenError::IOError(e.to_string()),
            MqttError::NetworkUnreachable => TokenError::NotConnect,
            MqttError::MqttConnectFailed(n) => TokenError::MqttConnectFailed(*n),
            MqttError::ConnectionLost => TokenError::ConnectionLost,
            MqttError::ProtocolError => TokenError::PacketError("Protocol Error".to_string()),
            MqttError::InvalidTopic => TokenError::InvalidTopic,
            MqttError::PacketIdError => TokenError::PacketIdError,
            MqttError::InternalChannelError => TokenError::InternalChannelError,
            MqttError::VariablePacketError(e) => TokenError::PacketError(e.to_string()),
        }
    }
}
