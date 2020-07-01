use crate::data::{Data, DataExtensions, DataFactory};
use serde::{Deserialize, Serialize};
use std::future::Future;

pub(crate) trait Factory<T, InnerT, R, O>: Clone + 'static
where
    O: Serialize,
    R: Future<Output = O>,
{
    fn call(&self, params: T) -> R;
}

impl<F, R, O> Factory<(), (), R, O> for F
where
    F: Fn() -> R + Clone + 'static,
    O: Serialize,
    R: Future<Output = O>,
{
    fn call(&self, _: ()) -> R {
        (self)()
    }
}

impl<F, R, O, T> Factory<(&DataExtensions,), (Data<T>,), R, O> for F
where
    F: Fn(Data<T>) -> R + Clone + 'static,
    O: Serialize,
    R: Future<Output = O>,
    T: 'static,
{
    fn call(&self, params: (&DataExtensions,)) -> R {
        (self)(params.0.get::<Data<T>>().unwrap().clone())
    }
}

impl<F, R, O, P> Factory<(P,), (P,), R, O> for F
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

impl<F, R, O, T, P> Factory<(&DataExtensions, P), (Data<T>, P), R, O> for F
where
    F: Fn(Data<T>, P) -> R + Clone + 'static,
    O: Serialize,
    R: Future<Output = O>,
    P: for<'de> Deserialize<'de>,
    T: 'static,
{
    fn call(&self, params: (&DataExtensions, P)) -> R {
        (self)(params.0.get::<Data<T>>().unwrap().clone(), params.1)
    }
}

impl<F, R, O, T, P> Factory<(&DataExtensions, P), (P, Data<T>), R, O> for F
where
    F: Fn(P, Data<T>) -> R + Clone + 'static,
    O: Serialize,
    R: Future<Output = O>,
    P: for<'de> Deserialize<'de>,
    T: 'static,
{
    fn call(&self, params: (&DataExtensions, P)) -> R {
        (self)(params.1, params.0.get::<Data<T>>().unwrap().clone())
    }
}
