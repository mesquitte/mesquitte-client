use std::{future::Future, net::SocketAddr, path::Path, time::Duration};

use async_tungstenite::{
    tokio::{connect_async, TokioAdapter},
    tungstenite::{client::IntoClientRequest, http::HeaderValue},
    WebSocketStream,
};
use mqtt_codec_kit::v4::packet::{MqttDecoder, MqttEncoder};
use s2n_quic::{client::Connect, stream::BidirectionalStream, Client};
use tokio::{
    io::{split, AsyncRead, AsyncWrite, ReadHalf, WriteHalf},
    net::{
        lookup_host,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    time::timeout,
};
use tokio_util::codec::{FramedRead, FramedWrite};
use url::Url;

use crate::{token::TokenError, ws_stream::WsByteStream};

pub trait Transport {
    type FrameReader: AsyncRead + Unpin + Send + 'static;
    type FrameWriter: AsyncWrite + Unpin + Send + 'static;

    fn connect(
        &self,
        addr: String,
        connect_timeout: Duration,
    ) -> impl Future<
        Output = Result<
            (
                FramedRead<Self::FrameReader, MqttDecoder>,
                FramedWrite<Self::FrameWriter, MqttEncoder>,
            ),
            TokenError,
        >,
    > + Send;
}

pub struct Tcp {}

impl Transport for Tcp {
    type FrameReader = OwnedReadHalf;
    type FrameWriter = OwnedWriteHalf;

    async fn connect(
        &self,
        addr: String,
        connect_timeout: Duration,
    ) -> Result<
        (
            FramedRead<Self::FrameReader, MqttDecoder>,
            FramedWrite<Self::FrameWriter, MqttEncoder>,
        ),
        TokenError,
    > {
        type Network = (
            FramedRead<OwnedReadHalf, MqttDecoder>,
            FramedWrite<OwnedWriteHalf, MqttEncoder>,
        );

        let addrs = lookup_host(addr).await.unwrap();

        let mut network: Option<Network> = None;
        let mut error = TokenError::NetworkUnreachable;

        for addr in addrs {
            match timeout(connect_timeout, TcpStream::connect(addr)).await {
                Ok(res) => match res {
                    Ok(s) => {
                        let (rd, wr) = s.into_split();
                        let frame_reader = FramedRead::new(rd, MqttDecoder::new());
                        let frame_writer = FramedWrite::new(wr, MqttEncoder::new());
                        network = Some((frame_reader, frame_writer));
                        break;
                    }
                    Err(err) => error = TokenError::IOError(err.to_string()),
                },
                Err(_) => error = TokenError::ConnectTimeout,
            }
        }

        match network {
            Some(network) => Ok(network),
            None => Err(error),
        }
    }
}

pub struct Ws {}

impl Transport for Ws {
    type FrameReader = ReadHalf<WsByteStream<WebSocketStream<TokioAdapter<TcpStream>>>>;
    type FrameWriter = WriteHalf<WsByteStream<WebSocketStream<TokioAdapter<TcpStream>>>>;

    async fn connect(
        &self,
        addr: String,
        connect_timeout: Duration,
    ) -> Result<
        (
            FramedRead<Self::FrameReader, MqttDecoder>,
            FramedWrite<Self::FrameWriter, MqttEncoder>,
        ),
        TokenError,
    > {
        let url = Url::parse(&addr).unwrap();

        let mut request = url.into_client_request().unwrap();

        request
            .headers_mut()
            .insert("Sec-WebSocket-Protocol", HeaderValue::from_static("mqtt"));

        match timeout(connect_timeout, connect_async(request)).await {
            Ok(stream) => match stream {
                Ok((stream, _)) => {
                    let ws_stream = WsByteStream::new(stream);

                    let (rd, wr) = split(ws_stream);

                    let frame_reader = FramedRead::new(rd, MqttDecoder::new());
                    let frame_writer = FramedWrite::new(wr, MqttEncoder::new());

                    Ok((frame_reader, frame_writer))
                }
                Err(_) => Err(TokenError::NetworkUnreachable),
            },
            Err(_) => Err(TokenError::ConnectTimeout),
        }
    }
}

pub struct Quic {
    cert: String,
}

impl Quic {
    pub fn new<S: Into<String>>(cert: S) -> Self {
        Self { cert: cert.into() }
    }
}

impl Transport for Quic {
    type FrameReader = ReadHalf<BidirectionalStream>;
    type FrameWriter = WriteHalf<BidirectionalStream>;

    async fn connect(
        &self,
        addr: String,
        connect_timeout: Duration,
    ) -> Result<
        (
            FramedRead<Self::FrameReader, MqttDecoder>,
            FramedWrite<Self::FrameWriter, MqttEncoder>,
        ),
        TokenError,
    > {
        let client = Client::builder();
        let client = client.with_tls(Path::new(self.cert.as_str())).unwrap();
        let client = client.with_io("0.0.0.0:0")?.start()?;

        match addr.parse::<SocketAddr>() {
            Ok(addr) => {
                let connect = Connect::new(addr).with_server_name("localhost");
                let mut connection = client.connect(connect).await?;

                // ensure the connection doesn't time out with inactivity
                connection.keep_alive(true)?;

                match timeout(connect_timeout, connection.open_bidirectional_stream()).await {
                    Ok(stream) => match stream {
                        Ok(stream) => {
                            let (rd, wr) = split(stream);

                            let frame_reader = FramedRead::new(rd, MqttDecoder::new());
                            let frame_writer = FramedWrite::new(wr, MqttEncoder::new());

                            Ok((frame_reader, frame_writer))
                        }
                        Err(err) => Err(err.into()),
                    },
                    Err(_) => Err(TokenError::ConnectTimeout),
                }
            }
            Err(err) => Err(TokenError::ConnectError(err.to_string())),
        }
    }
}
