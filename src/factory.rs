use crate::data::{Data};
use serde::{Deserialize, Serialize};
use std::future::Future;


pub(crate) trait Factory<T, R, O>: Clone + 'static
where
    O: Serialize,
    R: Future<Output = O> + Send,
{
    fn call(&self, params: T) -> R;
}

impl<F, R, O, T, P> Factory<(Data<T>, P), R, O> for F
where
    F: Fn(Data<T>, P) -> R + Clone + 'static,
    O: Serialize,
    R: Future<Output = O> + Send,
    P: for<'de> Deserialize<'de>,
    T: 'static,
{
    fn call(&self, params: (Data<T>, P)) -> R {
        (self)(params.0, params.1)
    }
}

