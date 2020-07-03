use jsonrpc_ws::server::{Request, Server};
use jsonrpc_ws::Data;
use std::sync::Mutex;
use tokio::time::{self, Duration};
use serde_json::json;
use serde::{Serialize, Deserialize};
use jsonrpc_lite::Error as JsonRpcError;

#[derive(Debug)]
pub struct ShareStateTest {
    pub a: Mutex<u32>,
    pub b: Mutex<String>,
}

#[derive(Deserialize)]
pub struct ReqTest {
    pub a: u32,
    pub b: String,
    pub c: Vec<String>,
}

#[derive(Serialize)]
pub struct RespTest {
    pub a: u32,
    pub b: String,
    pub c: Vec<String>,
}

#[derive(Debug, Serialize)]
pub enum TestError {
    // websock 错误
    WebSockServerBindError,
    WebSockServerAcceptConnError,
    WebSockServerGetPeerError,
}

impl Into<JsonRpcError> for TestError{
    fn into(self) -> JsonRpcError {
        JsonRpcError{code: 1000i64, message: "test".to_string(), data: None}
    }
}

async fn route_a(local_test: Data<ShareStateTest>) {
    let mut a = *local_test.get_ref().a.lock().unwrap();

    a += 1;
    assert_eq!(100u32, a);
}

async fn route_b(local_test: Data<ShareStateTest>, req: ReqTest) -> Result<RespTest, TestError> {
    let timeout = 1000u64;
    time::delay_for(Duration::from_millis(timeout.into())).await;

    let mut a = *local_test.get_ref().a.lock().unwrap();
    a += 1;

    let mut new_resp_c = Vec::<String>::new();
    new_resp_c.extend_from_slice(&req.c[..]);
    new_resp_c.push("add".to_string());

    Ok(RespTest{a: a, b: req.b, c: req.c})
}

#[test]
fn test_server() {
    let server = Server::new()
        .data(ShareStateTest {
            a: Mutex::new(99u32),
            b: Mutex::new("abcdefg".to_string()),
        })
        .to("route_b".to_string(), route_b);

    let  mut runtime = tokio::runtime::Runtime::new().unwrap();
    
    runtime.block_on(async move{
        let resp = server.route(json!({
            "jsonrpc": "2.0",
            "method": "route_b",
            "params": {"err_param": 1},
            "id": 99,
        })).await;

        assert_eq!(
            json!({
                "error":{
                    "code":-32602,
                    "message":"Invalid params"
                },
                "id":99,
                "jsonrpc":"2.0"
            }),
            resp
        );

        let resp = server.route(json!({
            "jsonrpc": "2.0",
            "method": "route_b",
            "params": {"a": 99u32, "b": "Test中文", "c": ["tset", "中文"]},
            "id": 2,
        })).await;

        println!("{}", resp);

        let resp = server.route(json!({
            "jsonrpc": "2.0",
            "method": "route_b",
            "params": {"a": 99u32, "b": "Test中文", "c": ["tset", "中文"]},
            "id": 3,
        })).await;

        println!("{}", resp);
    });
}
