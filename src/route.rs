use crate::data::DataFactory;
use crate::data::{Data, DataExtensions};
use crate::server_route_error;
use futures_util::future::join_all;
use jsonrpc_lite::Error as JsonRpcError;
use jsonrpc_lite::JsonRpc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

#[derive(Deserialize, Debug)]
pub struct Request {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
    pub id: i64,
}

pub struct Route {
    map: HashMap<
        String,
        Box<dyn Fn(Arc<DataExtensions>, Request) -> Pin<Box<dyn Future<Output = Value> + Send>>>,
    >,
    extensions: Arc<DataExtensions>,
}

unsafe impl Sync for Route {}

unsafe impl Send for Route {}

impl Route {
    pub fn new() -> Self {
        Route {
            map: HashMap::new(),
            extensions: Arc::new(DataExtensions::default()),
        }
    }

    pub fn to<'a, P, F, R, E, H, T>(mut self, key: String, handle: H) -> Self
    where
        P: for<'de> Deserialize<'de> + Send + 'static,
        R: Serialize + 'static,
        E: Serialize + Into<JsonRpcError> + 'static,
        F: Future<Output = Result<R, E>> + Send + 'static,
        H: Fn(Data<T>, P) -> F + 'static + Clone + Send + Sync,
        T: 'static + Sync + Send,
    {
        let inner_handle = move |extensions: Arc<DataExtensions>,
                                 req: Request|
              -> Pin<Box<dyn Future<Output = Value> + Send>> {
            async fn inner<P, R, E, F, H, T>(
                extensions: Arc<DataExtensions>,
                req: Request,
                handle: H,
            ) -> Value
            where
                P: for<'de> Deserialize<'de> + Send + 'static,
                R: Serialize + 'static,
                E: Serialize + Into<JsonRpcError> + 'static,
                F: Future<Output = Result<R, E>> + Send + 'static,
                H: Fn(Data<T>, P) -> F + 'static + Clone + Send + Sync,
                T: 'static + Sync + Send,
            {
                let params: P = match serde_json::from_value(req.params) {
                    Ok(params) => params,
                    Err(_) => {
                        return serde_json::to_value(JsonRpc::error(
                            req.id,
                            JsonRpcError::invalid_params(),
                        ))
                        .unwrap()
                    }
                };

                let data_t = extensions.get::<Data<T>>().unwrap().clone();
                match (handle).call((data_t, params)).await {
                    Ok(result) => serde_json::to_value(JsonRpc::success(
                        req.id,
                        &serde_json::to_value(result).unwrap(),
                    ))
                    .unwrap(),
                    Err(err) => serde_json::to_value(JsonRpc::error(req.id, err.into())).unwrap(),
                }
            }
            Box::pin(inner(extensions, req, handle.clone()))
        };
        self.map.insert(key, Box::new(inner_handle));
        self
    }

    pub fn data<D: 'static>(mut self, d: D) -> Self {
        Arc::get_mut(&mut self.extensions)
            .unwrap()
            .insert(Data::new(d));
        self
    }

    /// 传入一个Value格式的json-rpc单独请求
    ///   立刻返回响应执行Future或者错误结果
    pub async fn route_once(
        &self,
        req_str: Value,
    ) -> Result<Pin<Box<dyn Future<Output = Value> + Send>>, Value> {
        let req: Request = serde_json::from_value(req_str).unwrap();
        let handle = match self.map.get(&req.method) {
            Some(handle) => handle,
            None => {
                return Err(serde_json::to_value(JsonRpc::error(
                    req.id,
                    JsonRpcError::method_not_found(),
                ))
                .unwrap())
            }
        };

        Ok(handle(self.extensions.clone(), req))
    }
}

/// 传入jsonrpc请求
///   返回结果
pub async fn route_jsonrpc(server: Arc<Route>, req_str: &str) -> String {
    let req: Value = match serde_json::from_str(req_str) {
        Ok(req) => req,
        Err(_) => {
            return serde_json::to_value(JsonRpc::error((), JsonRpcError::parse_error()))
                .unwrap()
                .to_string()
        }
    };
    let resp = match req {
        Value::Object(_) => match server.route_once(req).await {
            Ok(fut) => fut.await,
            Err(err) => err,
        },
        Value::Array(array) => {
            let share_outputs = Arc::new(Mutex::new(Vec::<Value>::new()));
            let mut tasks = Vec::new();

            for each in array {
                let inner_server = Arc::downgrade(&server);
                let share_outputs = share_outputs.clone();

                tasks.push(async move {
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
            join_all(tasks).await;

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
        _ => {
            return serde_json::to_value(JsonRpc::error((), JsonRpcError::parse_error()))
                .unwrap()
                .to_string()
        }
    };

    resp.to_string()
}
