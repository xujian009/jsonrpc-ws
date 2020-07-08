use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use std::env;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use tokio::time;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use url::Url;

static LOCAL_SERVER: &'static str = "ws://127.0.0.1:9000";

const RECONN_INTERVAL: u64 = 3000;

struct WebSockWriteHalf(pub Option<SplitSink<WebSocketStream<TcpStream>, Message>>);
struct WebSockReadHalf(pub Option<SplitStream<WebSocketStream<TcpStream>>>);

async fn set_conn_none(
    lock_ws_receiver: Arc<Mutex<WebSockReadHalf>>,
    lock_ws_sender: Arc<Mutex<WebSockWriteHalf>>,
) -> bool {
    let mut ws_receiver = lock_ws_receiver.lock().unwrap();
    let mut ws_sender = lock_ws_sender.lock().unwrap();

    ws_receiver.0 = None;
    ws_sender.0 = None;
    return true;
}

async fn client_check_conn(
    case_url: Url,
    lock_ws_receiver: Arc<Mutex<WebSockReadHalf>>,
    lock_ws_sender: Arc<Mutex<WebSockWriteHalf>>,
) -> bool {
    let ws_receiver = lock_ws_receiver.lock().unwrap();

    if let None = ws_receiver.0 {
        drop(ws_receiver);

        if let Ok((ws_stream, _)) = connect_async(case_url).await {
            let (sender, receiver) = ws_stream.split();
            let mut ws_receiver = lock_ws_receiver.lock().unwrap();
            let mut ws_sender = lock_ws_sender.lock().unwrap();

            ws_sender.0 = Some(sender);
            ws_receiver.0 = Some(receiver);
            log::info!("connect success");
            return true;
        } else {
            log::info!("connect fail, reconning ...");
            return false;
        }
    }
    return true;
}

async fn receiver_loop(
    case_url: Url,
    lock_ws_receiver: Arc<Mutex<WebSockReadHalf>>,
    lock_ws_sender: Arc<Mutex<WebSockWriteHalf>>,
) {
    loop {
        let mut ws_receiver = lock_ws_receiver.lock().unwrap();

        let result: Result<String, bool> = match &mut ws_receiver.0 {
            Some(ws_receiver) => match ws_receiver.next().await {
                Some(Ok(msg)) => {
                    if msg.is_text() {
                        Ok(msg.into_text().unwrap())
                    } else {
                        log::warn!("Peer receive data format error, not text");
                        Err(false)
                    }
                }
                Some(Err(_)) => {
                    log::warn!("server close connect");
                    Err(true)
                }
                None => Err(true),
            },
            None => Err(true),
        };
        drop(ws_receiver);

        match result {
            Ok(msg) => {
                println!("resp: {}", msg);
            }
            Err(is_reconn) => {
                if is_reconn {
                    set_conn_none(lock_ws_receiver.clone(), lock_ws_sender.clone()).await;
                    if client_check_conn(
                        case_url.clone(),
                        lock_ws_receiver.clone(),
                        lock_ws_sender.clone(),
                    )
                    .await
                    {
                        log::info!("re_conn: {}", case_url);
                        continue;
                    } else {
                        time::delay_for(Duration::from_millis(RECONN_INTERVAL)).await;
                    }
                }
            }
        }
    }
}

async fn ws_send(str_cmd: String, lock_ws_sender: Arc<Mutex<WebSockWriteHalf>>) {
    let mut ws_sender = match lock_ws_sender.try_lock() {
        Ok(ws_sender) => ws_sender,
        Err(_) => {
            time::delay_for(Duration::from_millis(100)).await;

            log::warn!("ws_stream close, skip send");
            return;
        }
    };

    if let Some(ws_sender) = &mut ws_sender.0 {
        if let Err(err) = ws_sender.send(Message::Text(str_cmd)).await {
            log::warn!("ws_stream send failed with err: {}", err);
        }
    } else {
        log::warn!("ws_stream close, skip send");
    }
}

#[tokio::main]
async fn main() {
    use env_logger::Env;
    env_logger::from_env(Env::default().default_filter_or("warn")).init();

    let connect_transport = env::args()
        .nth(1)
        .unwrap_or_else(|| LOCAL_SERVER.to_string());

    let case_url = Url::parse(&connect_transport).expect("Bad testcase URL");

    let lock_ws_receiver = Arc::new(Mutex::new(WebSockReadHalf(None)));
    let lock_ws_sender = Arc::new(Mutex::new(WebSockWriteHalf(None)));

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            tokio::task::spawn_local(receiver_loop(
                case_url,
                lock_ws_receiver.clone(),
                lock_ws_sender.clone(),
            ));

            let mut reader = BufReader::new(tokio::io::stdin());
            loop {
                let mut str_cmd = String::new();
                reader.read_line(&mut str_cmd).await.unwrap();
                str_cmd.pop();

                ws_send(str_cmd, lock_ws_sender.clone()).await;
            }
        })
        .await;
}
