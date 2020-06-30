use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
struct Request {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
    pub id: i64,
}

#[derive(Serialize, Debug)]
struct Response<R, E> {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<R>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<E>,
    id: i64,
}

impl<R, E> Response<R, E> {
    pub fn new(id: i64, result: Result<R, E>) -> Self {
        match result {
            Ok(r) => Response {
                jsonrpc: "2.0".to_string(),
                error: None,
                result: Some(r),
                id,
            },
            Err(e) => Response {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(e),
                id,
            },
        }
    }
}

pub struct JSONRPCServer {
    map: HashMap<String, Box<dyn Fn(Request) -> serde_json::Value>>,
}

impl JSONRPCServer {
    pub fn new() -> Self {
        JSONRPCServer {
            map: HashMap::new(),
        }
    }

    pub fn to<P, F, R, E>(&mut self, key: String, handle: F)
    where
        P: for<'de> Deserialize<'de>,
        R: Serialize,
        E: Serialize,
        F: Fn(P) -> Result<R, E>,
        F: 'static,
    {
        let inner_handle = move |req: Request| -> serde_json::Value {
            let params = serde_json::from_value(req.params).unwrap();
            let result = Response::new(req.id, handle(params));
            serde_json::to_value(result).unwrap()
        };
        self.map.insert(key, Box::new(inner_handle));
    }
}
