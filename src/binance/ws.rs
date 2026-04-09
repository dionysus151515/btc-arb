use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{client_async_tls_with_config, connect_async};
use tracing::{error, info, warn};

use super::models::CombinedStreamMsg;

/// Event sent from WebSocket to the main loop
#[derive(Debug)]
pub struct DepthUpdate {
    pub symbol: String,
    pub data: super::models::DepthStreamMsg,
    pub recv_time_ms: i64,
}

/// Connect to Binance combined stream and forward depth updates
pub async fn run_ws(
    symbols: &[String],
    depth_level: u32,
    update_speed_ms: u32,
    proxy: Option<String>,
    tx: mpsc::UnboundedSender<DepthUpdate>,
) {
    loop {
        if let Err(e) =
            connect_and_stream(symbols, depth_level, update_speed_ms, proxy.as_deref(), &tx).await
        {
            error!("WebSocket error: {e}");
        }
        warn!("WebSocket disconnected, reconnecting in 3s...");
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

async fn connect_and_stream(
    symbols: &[String],
    depth_level: u32,
    update_speed_ms: u32,
    proxy: Option<&str>,
    tx: &mpsc::UnboundedSender<DepthUpdate>,
) -> Result<(), Box<dyn std::error::Error>> {
    let streams: Vec<String> = symbols
        .iter()
        .map(|s| {
            format!(
                "{}@depth{}@{}ms",
                s.to_lowercase(),
                depth_level,
                update_speed_ms
            )
        })
        .collect();
    let url = format!(
        "wss://stream.binance.com:9443/stream?streams={}",
        streams.join("/")
    );

    info!("Connecting to {url}");

    let (mut write, mut read) = if let Some(proxy_url) = proxy {
        info!("Using proxy: {proxy_url}");
        connect_via_proxy(&url, proxy_url).await?
    } else {
        let (ws, _) = connect_async(&url).await?;
        ws.split()
    };

    info!("WebSocket connected");

    while let Some(msg) = read.next().await {
        let msg = msg?;
        match msg {
            Message::Text(text) => {
                let recv_time = chrono::Utc::now().timestamp_millis();
                match serde_json::from_str::<CombinedStreamMsg>(&text) {
                    Ok(combined) => {
                        let symbol = combined
                            .stream
                            .split('@')
                            .next()
                            .unwrap_or("")
                            .to_uppercase();
                        let _ = tx.send(DepthUpdate {
                            symbol,
                            data: combined.data,
                            recv_time_ms: recv_time,
                        });
                    }
                    Err(e) => {
                        warn!("Failed to parse message: {e}");
                    }
                }
            }
            Message::Ping(data) => {
                let _ = write.send(Message::Pong(data)).await;
            }
            Message::Close(_) => {
                warn!("Server sent close frame");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

type WsSplit = (
    futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<TcpStream>,
        >,
        Message,
    >,
    futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<TcpStream>,
        >,
    >,
);

/// Connect to WebSocket via HTTP CONNECT proxy
async fn connect_via_proxy(
    ws_url: &str,
    proxy_url: &str,
) -> Result<WsSplit, Box<dyn std::error::Error>> {
    // Parse proxy URL: http://host:port
    let proxy_url = proxy_url
        .strip_prefix("http://")
        .unwrap_or(proxy_url);
    let proxy_addr = proxy_url.trim_end_matches('/');

    // Target is stream.binance.com:9443
    let target_host = "stream.binance.com";
    let target_port = 9443;

    // Connect to proxy
    let mut stream = TcpStream::connect(proxy_addr).await?;

    // Send HTTP CONNECT
    let connect_req = format!(
        "CONNECT {target_host}:{target_port} HTTP/1.1\r\nHost: {target_host}:{target_port}\r\n\r\n"
    );
    stream.write_all(connect_req.as_bytes()).await?;

    // Read proxy response
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).await?;
    let response = String::from_utf8_lossy(&buf[..n]);

    if !response.contains("200") {
        return Err(format!("Proxy CONNECT failed: {response}").into());
    }

    info!("Proxy tunnel established");

    // TLS handshake + WebSocket upgrade over the tunnel
    let request = ws_url.into_client_request()?;
    let connector = tokio_tungstenite::Connector::NativeTls(
        native_tls::TlsConnector::new()?
    );
    let (ws_stream, _) = client_async_tls_with_config(
        request,
        stream,
        None,
        Some(connector),
    )
    .await?;

    Ok(ws_stream.split())
}
