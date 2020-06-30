use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

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

pub(crate) trait Factory<T, R, O>: Clone + 'static
where
    O: Serialize,
    R: Future<Output = O>,
{
    fn call(&self, params: T) -> R;
}

impl<F, R, O> Factory<(), R, O> for F
where
    F: Fn() -> R + Clone + 'static,
    O: Serialize,
    R: Future<Output = O>,
{
    fn call(&self, _: ()) -> R {
        (self)()
    }
}

impl<F, R, O, D> Factory<(Data<D>,), R, O> for F
where
    F: Fn(Data<D>) -> R + Clone + 'static,
    O: Serialize,
    R: Future<Output = O>,
{
    fn call(&self, params: (Data<D>,)) -> R {
        (self)(params.0)
    }
}

impl<F, R, O, P> Factory<(P,), R, O> for F
where
    F: Fn(P) -> R + Clone + 'static,
    O: Serialize,
    R: Future<Output = O>,
    P: for<'de> Deserialize<'de>,
{
    fn call(&self, params: (P,)) -> R {
        (self)(params.0)
    }
}

impl<F, R, O, D, P> Factory<(Data<D>, P), R, O> for F
where
    F: Fn(Data<D>, P) -> R + Clone + 'static,
    O: Serialize,
    R: Future<Output = O>,
    P: for<'de> Deserialize<'de>,
{
    fn call(&self, params: (Data<D>, P)) -> R {
        (self)(params.0, params.1)
    }
}

impl<F, R, O, D, P> Factory<(P, Data<D>), R, O> for F
where
    F: Fn(P, Data<D>) -> R + Clone + 'static,
    O: Serialize,
    R: Future<Output = O>,
    P: for<'de> Deserialize<'de>,
{
    fn call(&self, params: (P, Data<D>)) -> R {
        (self)(params.0, params.1)
    }
}

pub struct Server {
    map: HashMap<
        String,
        Box<dyn FnOnce(Request) -> Pin<Box<dyn Future<Output = serde_json::Value> + Send>>>,
    >,
    state: Option<Box<dyn DataFactory>>,
}

impl Server {
    pub fn new() -> Self {
        Server {
            map: HashMap::new(),
            state: None,
        }
    }

    pub fn to<P, F, R, E, H>(mut self, key: String, handle: H) -> Self
    where
        P: for<'de> Deserialize<'de> + Send + 'static,
        R: Serialize + 'static,
        E: Serialize + 'static,
        F: Future<Output = Result<R, E>> + Send + 'static,
        H: Fn(P) -> F + 'static + Send,
    {
        let inner_handle =
            move |req: Request| -> Pin<Box<dyn Future<Output = serde_json::Value> + Send>> {
                async fn inner<P, R, E, F, H>(req: Request, handle: H) -> serde_json::Value
                where
                    P: for<'de> Deserialize<'de> + Send + 'static,
                    R: Serialize + 'static,
                    E: Serialize + 'static,
                    F: Future<Output = Result<R, E>> + Send + 'static,
                    H: Fn(P) -> F + 'static + Send,
                {
                    let params: P = serde_json::from_value(req.params).unwrap();
                    let _r = (handle)(params);
                    let result = Response::new(req.id, _r.await);
                    serde_json::to_value(result).unwrap()
                }
                Box::pin(inner(req, handle))
            };
        self.map.insert(key, Box::new(inner_handle));
        self
    }

    pub fn data<D: 'static>(mut self, d: D) -> Self {
        if self.state.is_none() {
            self.state = Some(Box::new(Data::new(d)))
        }
        self
    }
}
