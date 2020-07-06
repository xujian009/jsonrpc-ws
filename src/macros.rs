use jsonrpc_lite::JsonRpc::{Success, Error};
use jsonrpc_lite::JsonRpc;
use serde_json::{Value, json};
use alloc::collections::btree_map::BTreeMap;
use serde::{Serialize, Deserialize};
use jsonrpc_lite::Error as JsonRpcError;
use jsonrpc_lite::Params as JsonrpcParams;
use std::future::Future;
use crate::data::{DataExtensions, Data};
use std::sync::Arc;
use std::pin::Pin;
use std::any::{Any, TypeId};
use crate::data::DataFactory;
use crate::error::WallerError;


#[derive(Deserialize, Debug)]
pub struct Request {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
    pub id: i64,
}

trait Route{
    type OutputT: Future<Output = Value> + Clone + Send;
    fn route(&self, app_data: Arc<DataExtensions>, req_param: Value) -> Self::OutputT;
}

pub struct RouteFn<D,
    P: for<'de> Deserialize<'de>,
    R: Serialize,
    E: Into<JsonRpcError> >{
    fn_call: Box<dyn Fn(Data<D>, P) -> Pin<Box<dyn Future<Output = Result<R, E>> + Send + 'static>>>,

    app_data: Arc<DataExtensions>,
}

impl<D, P: for<'de> Deserialize<'de>,
R: Serialize,
E: Into<JsonRpcError> > RouteFn<D, P, R, E>{
    pub fn new(
        fn_call: Box<dyn Fn(Data<D>, P) -> Pin<Box<dyn Future<Output = Result<R, E>> + Send + 'static>>>,
         app_data: Arc<DataExtensions>) -> Self {
        Self{
            fn_call,
            app_data
        }
    }
}

impl<D, P, R, E> Route for RouteFn<D, P, R, E>
where
    P: for<'de> Deserialize<'de>,
    R: Serialize,
    E: Into<JsonRpcError> 
{
    type OutputT = impl Future<Output = Value> + Send;

    fn route(&self, app_data: Arc<DataExtensions>, req: Request) -> Self::OutputT{
        let fn_call = self.fn_call.clone();
        async move{
            let datad = app_data.get::<Data<D>>().unwrap().clone();
            let p: P = serde_json::from_value(req.params).unwrap();
            match fn_call(datad, p).await {
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
    }
}

type WalletRouteFn<D, P, R> = RouteFn<D, P, R, WallerError>;


impl<P, R, E, Output> Route<D, P, R, E> for RouteFn
where 
    D,
    P: for<'de> Deserialize<'de>,
    R: Serialize,
    E: Into<JsonRpcError>,
{
    type Output = impl Future<Output = Value>;

    fn route(&self, app_data: Arc<DataExtensions>, req_param: Value) -> Output{
        async move{
            let datad = params.0.get::<Data<D>>().unwrap().clone();
            let p: P = serde_json::from_value(req_param);
            match self.fn_call(datad, p).await {
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
    }
}

struct Executer{
    routes: BTreeMap<String, RouteOnce>,
}


impl Executer{
    fn find_route(&self, method: &str) -> Result<&Route, ()> {
        if let Some(route) = self.routes.get(method) {
            Ok(&route)
        }
        else{
            return Err(());
        }
    }

    fn execute_jsonstr(json_str: &str) -> String {
        let json_data = serde_json::from_str(json_str) {
            Ok(json_data) => json_data,
            Err(_) => return serde_json::to_string();
        };
    
        match json_data{
            Value::Object(_) => serde_json::to_string(
                self.execute_once(
                    JsonRpc::parse(json_data).unwrap_or(JsonRpc::error((), JsonrpcError::invalid_request())))),
            Value::Array(array) => serde_json::to_string(
                array.map(|obj| self.execute_once(JsonRpc::parse_vec(obj).
                unwrap_or(JsonRpc::error((), JsonrpcError::invalid_request())))).collect()),
            _ => return serde_json::to_string(JsonRpc::error((), JsonrpcError::parse_error())),
        }
    }

    fn execute_once(&self, req_json: JsonRpc) -> JsonRpc {
        let route = match self.find_route(req_json.get_method()){
            Ok(route) => route,
            Err(_) => return JsonRpc::error((), JsonrpcError::method_not_found()),
        };

        match route.execute(req_json.get_params()) {
            Ok(ans) => ans,
            Err(err) => err,
        }
    }
}


macro_rules! json_rpc_dispatch {
    ( $( $fn_name:expr => $fn_ident:ident => $param_type:ty ),*) => {{
        let mut json_rpc_dispatcher = BTreeMap::<String, NewType>::new();

        $(
            if let Some(_) = json_rpc_dispatcher.get(&$fn_name) {
                panic!("json_rpc method {} has registed", $fn_name);
            }
            json_rpc_dispatcher.insert($fn_name, NewType{a: 1u64, b: 99u64});
        )*

        json_rpc_dispatcher
    }};
}

// macro_rules! get_fn_name {
//     ($fn_ident:block) => {
//         format!("{:?}", $fn_ident)
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Serialize, Deserialize};
    use jsonrpc_lite::JsonRpc::{Request, Success, Error};
    use jsonrpc_lite::JsonRpc;
    use alloc::string::String;

    #[derive(Serialize, Deserialize)]
    struct RequestA {
        param_1: String,
        param_2: String,
        param_3: u32,
    }
    
    fn fn_testa(req_data: RequestA) -> JsonRpc {
        JsonRpc::success(99i64, &json!({"ans": req_data.param_2}))
    }

    #[test]
    fn test_dispatch(){
        trace_macros!(true);
        let dispatcher = json_rpc_dispatch!(
            "fn_testa".to_string() => fn_testa => RequestA
        );
        trace_macros!(false);

        println!("{:?}", dispatcher);
    }
}