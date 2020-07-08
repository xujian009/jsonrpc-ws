extern crate jsonrpc_websock;

use jsonrpc_lite::Error as JsonRpcError;
use jsonrpc_websock::WsServer;
use jsonrpc_ws::route::Route;
use jsonrpc_ws::Data;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::{Arc, RwLock};

#[derive(Serialize)]
pub enum ExampleError {
    // websock 错误
    ParamIsNone,
}

impl Into<JsonRpcError> for ExampleError {
    fn into(self) -> JsonRpcError {
        let (code, message) = match self {
            ParamIsNone => (1000i64, "Param is none"),
            _ => (9999i64, "Unexpect error"),
        };

        JsonRpcError {
            code,
            message: message.to_string(),
            data: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct CurrencyDetail {
    value: u64,
    id: String,
    dcds: String,
    locked: bool,
    owner: String,
}

pub struct CurrencyStore {}
impl CurrencyStore {
    pub fn get_detail_by_ids(
        &self,
        req: GetDetailParam,
    ) -> Result<Vec<CurrencyDetail>, ExampleError> {
        if req.ids.len() == 0 {
            return Err(ExampleError::ParamIsNone);
        }
        Ok(Vec::<CurrencyDetail>::new())
    }
}

#[derive(Serialize, Deserialize)]
pub struct GetDetailParam {
    ids: Vec<String>,
}

pub async fn get_detail_by_ids(
    wallet: Data<TSystem>,
    req: GetDetailParam,
) -> Result<Vec<CurrencyDetail>, ExampleError> {
    let store = wallet.get_ref().store.try_read().unwrap();
    store.get_detail_by_ids(req)
}

pub struct TSystem {
    pub store: RwLock<CurrencyStore>,
}

impl TSystem {
    pub fn new() -> Self {
        Self {
            store: RwLock::new(CurrencyStore {}),
        }
    }
}

pub async fn start_ws_server(bind_transport: String) {
    let route: Arc<Route> = Arc::new(
        Route::new()
            .data(TSystem::new())
            .to("currency.ids.detail".to_string(), get_detail_by_ids),
    );

    let ws_server = match WsServer::bind(bind_transport).await {
        Ok(ws_server) => ws_server,
        Err(err) => panic!("{}", err),
    };

    ws_server.listen_loop(route).await;
}

static LOCAL_SERVER: &'static str = "127.0.0.1:9000";

#[tokio::main]
async fn main() {
    use env_logger::Env;
    env_logger::from_env(Env::default().default_filter_or("warn")).init();

    let bind_transport = env::args()
        .nth(1)
        .unwrap_or_else(|| LOCAL_SERVER.to_string());

    start_ws_server(bind_transport).await;
}
