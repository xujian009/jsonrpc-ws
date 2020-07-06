use jsonrpc_lite::Error as JsonRpcError;
use jsonrpc_lite::JsonRpc;
use jsonrpc_ws::server::Server;
use jsonrpc_ws::Data;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::time::{self, Duration};

#[derive(Debug)]
pub struct ShareStateTest {
    pub a: Mutex<u64>,
    pub b: Mutex<String>,
}

#[derive(Deserialize)]
pub struct ReqTest {
    pub a: u64,
    pub b: String,
    pub c: Vec<String>,
}

#[derive(Deserialize, Serialize)]
pub struct RespTest {
    pub a: u64,
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

impl Into<JsonRpcError> for TestError {
    fn into(self) -> JsonRpcError {
        JsonRpcError {
            code: 1000i64,
            message: "test".to_string(),
            data: None,
        }
    }
}

async fn route_b(local_test: Data<ShareStateTest>, req: ReqTest) -> Result<RespTest, TestError> {
    time_sleep(1000).await;

    let mut a = local_test.get_ref().a.lock().unwrap();
    *a += 1;

    let mut new_resp_c = Vec::<String>::new();
    new_resp_c.extend_from_slice(&req.c[..]);
    new_resp_c.push(format!(" add {}", *a));

    Ok(RespTest {
        a: *a + req.a,
        b: req.b,
        c: new_resp_c,
    })
}

async fn time_sleep(timeout_ms: u64) {
    time::delay_for(Duration::from_millis(timeout_ms.into())).await;
}

fn server_route_error() -> JsonRpcError {
    JsonRpcError {
        code: -32500,
        message: "Server Internal Route error".to_string(),
        data: None,
    }
}

/// 传入jsonrpc请求
///   返回结果
pub async fn route_jsonrpc(server: Arc<Server>, req_str: Value) -> Value {
    match req_str {
        Value::Object(_) => match server.route_once(req_str).await {
            Ok(fut) => fut.await,
            Err(err) => err,
        },
        Value::Array(array) => {
            let localtask = tokio::task::LocalSet::new();
            let share_outputs = Arc::new(Mutex::new(Vec::<Value>::new()));

            for each in array {
                let inner_server = Arc::downgrade(&server);
                let share_outputs = share_outputs.clone();

                localtask.spawn_local(async move {
                    // task开始执行是尝试获取server对象
                    let output = match inner_server.upgrade() {
                        Some(server) => match server.route_once(each).await {
                            Ok(fut) => fut.await,
                            Err(err) => err,
                        },
                        None => serde_json::to_value(server_route_error()).unwrap(),
                    };

                    let mut outputs = share_outputs.lock().unwrap();
                    outputs.push(output);
                });
            }
            localtask.await;

            // TODO 内部panic可能要处理
            // outputs Arc持有者只剩下一个，此处取出不会失败，也不考虑失败处理
            let output = if let Ok(outputs) = Arc::try_unwrap(share_outputs) {
                // 锁持有者同理
                outputs.into_inner().unwrap()
            } else {
                panic!("Arc<Mutex<>> into_inner failed");
            };
            Value::Array(output)
        }
        _ => return serde_json::to_value(JsonRpc::error((), JsonRpcError::parse_error())).unwrap(),
    }
}

#[test]
fn test_server_simple() {
    let server = Server::new()
        .data(ShareStateTest {
            a: Mutex::new(100u64),
            b: Mutex::new("abcdefg".to_string()),
        })
        .to("route_b".to_string(), route_b);

    let mut runtime = tokio::runtime::Runtime::new().unwrap();

    runtime.block_on(async move {
        let resp = server
            .route_once(json!({
                "jsonrpc": "2.0",
                "method": "route_b",
                "params": {"err_param": 1},
                "id": 99,
            }))
            .await;

        assert_eq!(
            json!({
                "error":{
                    "code":-32602,
                    "message":"Invalid params"
                },
                "id":99,
                "jsonrpc":"2.0"
            }),
            resp.unwrap().await
        );
    });
}

#[tokio::test]
async fn test_server_route_and_array() {
    let server = Arc::new(
        Server::new()
            .data(ShareStateTest {
                a: Mutex::new(100u64),
                b: Mutex::new("abcdefg".to_string()),
            })
            .to("route_b".to_string(), route_b),
    );

    let tasks = async move {
        let resp = route_jsonrpc(
            server.clone(),
            json!({
                "jsonrpc": "2.0",
                "method": "route_b",
                "params": {"err_param": 1},
                "id": 99,
            }),
        )
        .await;

        assert_eq!(
            json!({"error":{"code":-32602,"message":"Invalid params"},"id":99,"jsonrpc":"2.0"})
                .to_string(),
            resp.to_string()
        );

        let resp = route_jsonrpc(
            server.clone(),
            json!([{
                "jsonrpc": "2.0",
                "method": "route_b",
                "params": {"err_param": 1},
                "id": 91,
            },{
                "jsonrpc": "2.0",
                "method": "route_b",
                "params": {"a": 8888u64, "b":"_8888_", "c":["c","_string_","_8888_"]},
                "id": 92,
            },{
                "jsonrpc": "2.0",
                "method": "route_b",
                "params": {"a": 8888u64, "b":"_8888_", "c":["c","_string_","_8888_"]},
                "id": 93,
            }]),
        )
        .await;

        let resp_vec = match resp {
            Value::Array(array) => array,
            _ => panic!("unexpect error"),
        };

        let ans_91: Vec<&Value> = resp_vec
            .iter()
            .filter(|&resp| resp["id"].as_u64().unwrap() == 91)
            .collect();

        assert_eq!(1, ans_91.len());
        assert_eq!(
            "Invalid params",
            ans_91[0]["error"]["message"].as_str().unwrap()
        );

        let ans_9293_a_sum = resp_vec
            .iter()
            .filter(|&resp| {
                resp["id"].as_u64().unwrap() == 92 || resp["id"].as_u64().unwrap() == 93
            })
            .fold(0, |sum, resp| {
                sum + serde_json::from_value::<RespTest>(resp["result"].clone())
                    .unwrap()
                    .a
            });

        assert_eq!(8888u64 * 2 + 101 + 102, ans_9293_a_sum);
    };

    let local = tokio::task::LocalSet::new();

    local.run_until(tasks).await;
}
