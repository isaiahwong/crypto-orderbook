use fastwebsockets::{Frame, OpCode, Payload};
use http_body_util::Empty;
use hyper::body::Bytes;
use hyper_util::rt::TokioExecutor;
use std::fmt;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio_rustls::client::{TlsConnector, TlsStream};
use tokio_rustls::rustls::ClientConfig;
use url::Url;

pub struct WsHandle {
    pub rx: Receiver<Result<Vec<u8>, WsError>>,
    pub tx: Sender<Vec<u8>>,
}

pub async fn connect(url: &str) -> Result<WsHandle, WsError> {
    let url_parsed = Url::parse(url)?;
    let host = url_parsed.host_str().ok_or(WsError::MissingHost)?;

    let port = url_parsed.port_or_known_default().unwrap_or(443);
    let addr = format!("{}:{}", host, port);

    let tcp_stream = TcpStream::connect(&addr).await?;
    let tls_stream = tls_connect(host, tcp_stream).await?;

    let req = hyper::Request::builder()
        .uri(url)
        .header("Host", host)
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header(
            "Sec-WebSocket-Key",
            fastwebsockets::handshake::generate_key(),
        )
        .header("Sec-WebSocket-Version", "13")
        .body(Empty::<Bytes>::new())?;

    let executor = TokioExecutor::new();
    let (ws, _) = fastwebsockets::handshake::client(&executor, req, tls_stream)
        .await
        .map_err(|e| WsError::Handshake(format!("{:?}", e)))?;

    let mut ws = fastwebsockets::FragmentCollector::new(ws);

    let (read_tx, read_rx) = channel(100);
    let (write_tx, mut write_rx) = channel(100);

    tokio::spawn(async move {
        loop {
            tokio::select! {
                // Write ws
                m = write_rx.recv() => {
                    if let Some(msg) = m {
                        // FIXME: handle tx error if needed
                        let _ = ws.write_frame(Frame::binary(Payload::Owned(msg))).await;
                    }
                }

                // Read ws
                res = ws.read_frame() => {
                    match process_frame(res) {
                        FrameResult::Msg(val) => {
                            if read_tx.send(Ok(val)).await.is_err() {
                                 break;
                            }
                        }
                        FrameResult::Ping(val) => {
                            if ws.write_frame(fastwebsockets::Frame::new(true, OpCode::Pong, None, fastwebsockets::Payload::Owned(val))).await.is_err() {
                                break;
                            }
                        }
                        FrameResult::Error(e) => {
                            let _ = read_tx.send(Err(e)).await;
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    Ok(WsHandle {
        rx: read_rx,
        tx: write_tx,
    })
}

fn process_frame(res: Result<Frame<'_>, fastwebsockets::WebSocketError>) -> FrameResult {
    let frame = match res {
        Ok(f) => f,
        Err(e) => return FrameResult::Error(e.into()),
    };

    match frame.opcode {
        OpCode::Text | OpCode::Binary => {
            let payload = match frame.payload {
                fastwebsockets::Payload::Owned(data) => data,
                fastwebsockets::Payload::Borrowed(data) => data.to_vec(),
                fastwebsockets::Payload::BorrowedMut(data) => data.to_vec(),
                fastwebsockets::Payload::Bytes(data) => data.into(),
            };
            FrameResult::Msg(payload)
        }
        OpCode::Ping => {
            let payload = match frame.payload {
                fastwebsockets::Payload::Owned(data) => data,
                fastwebsockets::Payload::Borrowed(data) => data.to_vec(),
                fastwebsockets::Payload::BorrowedMut(data) => data.to_vec(),
                fastwebsockets::Payload::Bytes(data) => data.into(),
            };
            FrameResult::Ping(payload)
        }
        OpCode::Close => FrameResult::Error(WsError::WebSocket(
            fastwebsockets::WebSocketError::ConnectionClosed,
        )),
        _ => FrameResult::None,
    }
}

pub enum FrameResult {
    Msg(Vec<u8>),
    Ping(Vec<u8>),
    None,
    Error(WsError),
}

async fn tls_connect(host: &str, tcp_stream: TcpStream) -> Result<TlsStream<TcpStream>, WsError> {
    let root_store = tokio_rustls::rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };

    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(config));
    let domain = tokio_rustls::rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|e| WsError::InvalidDns(e.to_string()))?;

    Ok(connector.connect(domain, tcp_stream).await?)
}

#[derive(Debug)]
pub enum WsError {
    UrlParse(url::ParseError),
    Io(std::io::Error),
    Tls(tokio_rustls::rustls::Error),
    InvalidDns(String),
    Http(http::Error),
    WebSocket(fastwebsockets::WebSocketError),
    MissingHost,
    Handshake(String),
}

impl fmt::Display for WsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WsError::UrlParse(e) => write!(f, "Invalid URL: {}", e),
            WsError::Io(e) => write!(f, "IO Error: {}", e),
            WsError::Tls(e) => write!(f, "TLS Error: {}", e),
            WsError::InvalidDns(e) => write!(f, "Invalid DNS name: {}", e),
            WsError::Http(e) => write!(f, "HTTP Error: {}", e),
            WsError::WebSocket(e) => write!(f, "WebSocket Error: {}", e),
            WsError::MissingHost => write!(f, "No host in URL"),
            WsError::Handshake(e) => write!(f, "Handshake failed: {}", e),
        }
    }
}

impl std::error::Error for WsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WsError::UrlParse(e) => Some(e),
            WsError::Io(e) => Some(e),
            WsError::Tls(e) => Some(e),
            WsError::InvalidDns(_) => None,
            WsError::Http(e) => Some(e),
            WsError::WebSocket(e) => Some(e),
            WsError::MissingHost => None,
            WsError::Handshake(_) => None,
        }
    }
}

impl From<url::ParseError> for WsError {
    fn from(e: url::ParseError) -> Self {
        WsError::UrlParse(e)
    }
}

impl From<std::io::Error> for WsError {
    fn from(e: std::io::Error) -> Self {
        WsError::Io(e)
    }
}

impl From<tokio_rustls::rustls::Error> for WsError {
    fn from(e: tokio_rustls::rustls::Error) -> Self {
        WsError::Tls(e)
    }
}

impl From<http::Error> for WsError {
    fn from(e: http::Error) -> Self {
        WsError::Http(e)
    }
}

impl From<fastwebsockets::WebSocketError> for WsError {
    fn from(e: fastwebsockets::WebSocketError) -> Self {
        WsError::WebSocket(e)
    }
}
