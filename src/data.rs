use std::any::{Any, TypeId};
use std::sync::Arc;

use fxhash::FxHashMap;

pub struct Data<T>(Arc<T>);

impl<T> Data<T> {
    pub fn new(d: T) -> Self {
        Self(Arc::new(d))
    }

    pub fn get_ref(&self) -> &T {
        self.0.as_ref()
    }
}

impl<T> Clone for Data<T> {
    fn clone(&self) -> Data<T> {
        Data(self.0.clone())
    }
}

pub(crate) trait DataFactory {
    fn get<D: Clone + 'static>(&self) -> Option<&D>;
}

pub struct DataExtensions(FxHashMap<TypeId, Box<dyn Any>>);

impl DataExtensions {
    pub fn insert<T: Clone + 'static>(&mut self, t: T) {
        self.0.insert(TypeId::of::<T>(), Box::new(t));
    }
}

unsafe impl Sync for DataExtensions {}

unsafe impl Send for DataExtensions {}

impl Default for DataExtensions {
    fn default() -> Self {
        Self(FxHashMap::<TypeId, Box<dyn Any>>::default())
    }
}

impl DataFactory for DataExtensions {
    fn get<D: Clone + 'static>(&self) -> Option<&D> {
        self.0
            .get(&TypeId::of::<D>())
            .and_then(|boxed| boxed.downcast_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Debug)]
    pub struct Test {
        pub a: Mutex<u32>,
        pub b: Mutex<String>,
    }

    #[test]
    fn test_data() {
        let data = Data::new(Test {
            a: Mutex::new(99u32),
            b: Mutex::new("abcdefg".to_string()),
        });

        let mut extension = DataExtensions::default();
        extension.insert(data);

        let obj = extension.get::<Data<Test>>().unwrap().clone();

        assert_eq!(99, *obj.get_ref().a.lock().unwrap());
        assert_eq!("abcdefg".to_string(), *obj.get_ref().b.lock().unwrap());
    }
}
