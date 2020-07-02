use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Deserialize, Debug)]
pub(crate) struct Request {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
    pub id: i64,
}

#[derive(Serialize, Debug)]
pub(crate) struct Response<R, E> {
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

pub(crate) trait DataFactory {}

pub struct Data<T>(T);

impl<T> Data<T> {
    pub fn new(t: T) -> Self {
        Data(t)
    }
}

impl<T> DataFactory for Data<T> {}

pub(crate) trait Factory<P, D, R, O>: Clone + 'static
where
    P: for<'de> Deserialize<'de>,
    O: Serialize,
    R: Future<Output = O>,
{
    fn call_tuple(&self, data: Data<D>, param: P) -> R;
}

pub struct Server {
    map: HashMap<
        String,
        Box<
            dyn Fn(
                Arc<dyn Any>,
                Request,
            ) -> Pin<Box<dyn Future<Output = serde_json::Value> + Send>>,
        >,
    >,
    state: Option<Arc<dyn Any>>,
}

impl Server {
    pub fn new() -> Self {
        Server {
            map: HashMap::new(),
            state: None,
        }
    }

    pub fn to<P, F, R, E, H, T>(mut self, key: String, handle: H) -> Self
    where
        P: for<'de> Deserialize<'de> + Send + 'static,
        R: Serialize + 'static,
        E: Serialize + 'static,
        F: Future<Output = Result<R, E>> + Send + 'static,
        H: Fn(Data<T>, P) -> F + 'static + Clone + Send,
    {
        let inner_handle =
            move |data: Arc<dyn Any>,
                  req: Request|
                  -> Pin<Box<dyn Future<Output = serde_json::Value> + Send>> {
                async fn inner<P, R, E, F, H, T>(
                    data: Data<T>,
                    req: Request,
                    handle: H,
                ) -> serde_json::Value
                where
                    P: for<'de> Deserialize<'de> + Send + 'static,
                    R: Serialize + 'static,
                    E: Serialize + 'static,
                    F: Future<Output = Result<R, E>> + Send + 'static,
                    H: Fn(Data<T>, P) -> F + 'static + Clone,
                {
                    let params: P = serde_json::from_value(req.params).unwrap();
                    let _r = (handle)(data, params);
                    let result = Response::new(req.id, _r.await);
                    serde_json::to_value(result).unwrap()
                }
                Box::pin(inner(data, req, handle.clone()))
            };
        self.map.insert(key, Box::new(inner_handle));
        self
    }

    pub fn data<D: 'static>(mut self, d: D) -> Self {
        if self.state.is_none() {
            self.state = Some(Arc::new(Data::new(d)))
        }
        self
    }

    pub async fn recv(&self, input: serde_json::Value) -> serde_json::Value {
        let r: Request = serde_json::from_value(input).unwrap();
        let h = self.map.get(&r.method).unwrap();
        let d = self.state.as_ref().unwrap();
        h(d.clone(), r).await
    }
}
