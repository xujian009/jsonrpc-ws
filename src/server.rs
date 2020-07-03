use crate::data::{Data, DataExtensions};
use jsonrpc_lite::Error as JsonRpcError;
use jsonrpc_lite::JsonRpc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use crate::factory::Factory;
use std::sync::Arc;
use crate::data::DataFactory;

#[derive(Deserialize, Debug)]
pub struct Request {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
    pub id: i64,
}


pub struct Server {
    map: HashMap<
        String,
        Box<dyn Fn(Arc<DataExtensions>, Request) -> Pin<Box<dyn Future<Output = serde_json::Value> + Send>>>,
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
        let inner_handle =
            move |extensions: Arc<DataExtensions>, req: Request| -> Pin<Box<dyn Future<Output = serde_json::Value> + Send>> {
                async fn inner<P, R, E, F, H, T>(extensions: Arc<DataExtensions>, req: Request, handle: H) -> serde_json::Value
                where
                    P: for<'de> Deserialize<'de> + Send + 'static,
                    R: Serialize + 'static,
                    E: Serialize + Into<JsonRpcError> + 'static,
                    F: Future<Output = Result<R, E>> + Send + 'static,
                    H: Fn(Data<T>, P) -> F + 'static + Clone + Send + Sync,
                    T: 'static + Sync + Send,
                {
                    let params: P = match serde_json::from_value(req.params){
                        Ok(params) => params,
                        Err(err) => return serde_json::to_value(JsonRpc::error(req.id, JsonRpcError::invalid_params())).unwrap(),
                    };
                    
                    let data_t = extensions.get::<Data<T>>().unwrap().clone();
                    match (handle).call((data_t, params)).await {
                        Ok(result) => serde_json::to_value(JsonRpc::success(
                            req.id,
                            &serde_json::to_value(result).unwrap(),
                        ))
                        .unwrap(),
                        Err(err) => {
                            serde_json::to_value(JsonRpc::error(req.id, err.into())).unwrap()
                        }
                    }
                }
                Box::pin(inner(extensions, req, handle.clone()))
            };
        self.map.insert(key, Box::new(inner_handle));
        self
    }

    pub fn data<D: 'static>(mut self, d: D) -> Self {
        Arc::get_mut(&mut self.extensions).unwrap().insert(Data::new(d));
        self
    }

    /// 传入一个Value格式的json-rpc单独请求
    ///   返回响应
    pub async fn route(&self, req_str: serde_json::Value) -> serde_json::Value {
        let req: Request = serde_json::from_value(req_str).unwrap();
        let handle = match self.map.get(&req.method) {
            Some(handle) => handle,
            None => return serde_json::to_value(JsonRpc::error(req.id, JsonRpcError::method_not_found())).unwrap(),
        };

        handle(self.extensions.clone(), req).await
    }
}
