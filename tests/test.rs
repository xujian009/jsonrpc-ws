use jsonrpc_ws::server::{Request, Server};
use jsonrpc_ws::Data;
use std::sync::Mutex;
use tokio::time::{self, Duration};
use serde_json::json;

#[derive(Debug)]
pub struct Test {
    pub a: Mutex<u32>,
    pub b: Mutex<String>,
}

async fn route_a(local_test: Data<Test>) {
    let mut a = *local_test.get_ref().a.lock().unwrap();

    a += 1;
    assert_eq!(100u32, a);
}

async fn route_b(local_test: Data<Test>, req: Request) {
    let timeout = 1000u64;
    time::delay_for(Duration::from_millis(timeout.into()));

    let mut a = *local_test.get_ref().a.lock().unwrap();

    a += 1;
    assert_eq!(100u32, a);
}

#[test]
fn test_server() {
    let mut server = Server::new()
        .data(Test {
            a: Mutex::new(99u32),
            b: Mutex::new("abcdefg".to_string()),
        })
        .to("route_b".to_string(), route_b);

    let runtime = tokio::runtime::Runtime();
    
    runtime.block_on(server.route(json!({
        "jsonrpc": "2.0",
        "method": "route_b",
        "params": {"a":1,  "b":2},
        "id": 99,
    })));
}
