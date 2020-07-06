use crate::data::DataFactory;
use crate::data::{Data, DataExtensions};
use jsonrpc_lite::Error as JsonRpcError;
use jsonrpc_lite::JsonRpc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Deserialize, Debug)]
pub struct Request {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
    pub id: i64,
}

pub struct Server {
    map: HashMap<
        String,
        Box<dyn Fn(Arc<DataExtensions>, Request) -> Pin<Box<dyn Future<Output = Value> + Send>>>,
    >,
    extensions: Arc<DataExtensions>,
}

impl Server {
    pub fn new() -> Self {
        Server {
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
