use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use jsonrpc_core::route::{route_jsonrpc, Route};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

type WebSockWriteHalf = SplitSink<WebSocketStream<TcpStream>, Message>;
type WebSockReadHalf = SplitStream<WebSocketStream<TcpStream>>;

const REQ_QUEUE_LEN: usize = 10;

pub struct WsServer {
    listener: TcpListener,
}

impl WsServer {
    pub async fn bind(bind_transport: String) -> Result<Self, String> {
        let listener = TcpListener::bind(&bind_transport)
            .await
            .map_err(|err| err.to_string())?;

        log::info!("Listening on: {}", &bind_transport);

        let instance = Self { listener };

        Ok(instance)
    }

    pub async fn listen_loop(mut self, route: Arc<Route>) {
        while let Ok((stream, _)) = self.listener.accept().await {
            let route_ = route.clone();
            tokio::spawn(async move {
                if let Err(err) = Self::client_loop(stream, route_).await {
                    log::warn!("{}", err);
                }
            });
        }
    }

    async fn client_loop(stream: TcpStream, route: Arc<Route>) -> Result<(), String> {
        let peer = stream
            .peer_addr()
            .map_err(|err| format!("get client peer_addr error, with info: {}", err))?;

        let ws_stream = accept_async(stream)
            .await
            .map_err(|err| format!("ws_stream accept error, with info: {}", err))?;

        log::info!("client {} connect", peer);
        let (write_half, read_half) = ws_stream.split();

        let (req_pipe_in, req_pipe_out) = mpsc::channel(REQ_QUEUE_LEN);
        let (resp_pipe_in, resp_pipe_out) = mpsc::channel(REQ_QUEUE_LEN);

        tokio::select! {
            _ = Self::dispatch_loop(route, req_pipe_out, resp_pipe_in) => {
                log::info!("client {} close because dispatch_loop", peer);
            },
            _ = Self::read_half_loop(read_half, req_pipe_in) => {
                log::info!("client {} close because read_half", peer);
            },
            _ = Self::write_half_loop(write_half, resp_pipe_out) => {
                log::info!("client {} close because write_half", peer);
            },
        };

        Ok(())
    }

    async fn dispatch_loop(
        route: Arc<Route>,
        mut req_pipe: mpsc::Receiver<String>,
        mut resp_pipe: mpsc::Sender<String>,
    ) {
        while let Some(req_str) = req_pipe.recv().await {
            let route_ = route.clone();
            let resp_str = route_jsonrpc(route_, &req_str).await;
            if let Err(_) = resp_pipe.send(resp_str).await {
                // 处理完客户端已断开，忽略
                return;
            }
        }
    }

    async fn read_half_loop(mut read_half: WebSockReadHalf, mut req_pipe_in: mpsc::Sender<String>) {
        while let Some(ans) = read_half.next().await {
            match ans {
                Err(_) => {
                    return;
                }
                Ok(Message::Text(msg_str)) => {
                    if let Err(_) = req_pipe_in.send(msg_str).await {
                        return;
                    }
                }
                Ok(Message::Ping(_)) => log::debug!("recv message ping/pong"),
                Ok(Message::Pong(_)) => log::debug!("recv message ping/pong"),
                Ok(_) => log::debug!("data format not String, ignore this item"),
            }
        }
    }

    async fn write_half_loop(
        mut write_half: WebSockWriteHalf,
        mut resp_pipe_out: mpsc::Receiver<String>,
    ) {
        while let Some(msg_str) = resp_pipe_out.recv().await {
            if let Err(_) = write_half.send(Message::Text(msg_str)).await {
                return;
            }
        }
    }
}
