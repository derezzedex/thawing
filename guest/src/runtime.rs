use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::guest;

pub static TABLE: LazyLock<Mutex<HashMap<u32, Closure>>> =
    LazyLock::new(|| Mutex::new(HashMap::default()));

pub struct AnyBox(Box<dyn std::any::Any>);

impl AnyBox {
    pub fn new<T: 'static>(value: T) -> Self {
        Self(Box::new(value))
    }

    pub fn downcast<T: 'static>(self) -> T {
        *self.0.downcast::<T>().unwrap()
    }
}

pub struct Closure {
    func: Box<dyn Fn(AnyBox) -> AnyBox + Send>,
}

impl Closure {
    pub fn stateful<S, T>(func: impl Fn(S) -> T + Send + 'static) -> Self
    where
        S: serde::de::DeserializeOwned + 'static,
        T: serde::Serialize + 'static,
    {
        let wrapper = move |state: AnyBox| -> AnyBox {
            let bytes = state.downcast::<guest::Bytes>();
            let msg = func(bincode::deserialize(&bytes).unwrap());
            AnyBox::new(bincode::serialize(&msg).unwrap())
        };

        Self {
            func: Box::new(wrapper),
        }
    }

    pub fn stateful_ref<S, T>(func: impl Fn(&S) -> T + Send + 'static) -> Self
    where
        S: serde::de::DeserializeOwned + 'static,
        T: serde::Serialize + 'static,
    {
        let wrapper = move |state: AnyBox| -> AnyBox {
            let bytes = state.downcast::<guest::Bytes>();
            let msg = func(&bincode::deserialize(&bytes).unwrap());
            AnyBox::new(bincode::serialize(&msg).unwrap())
        };

        Self {
            func: Box::new(wrapper),
        }
    }

    pub fn stateless<T>(func: impl Fn() -> T + Send + 'static) -> Self
    where
        T: serde::Serialize + 'static,
    {
        let wrapper =
            move |_state: AnyBox| -> AnyBox { AnyBox::new(bincode::serialize(&func()).unwrap()) };

        Self {
            func: Box::new(wrapper),
        }
    }

    pub fn call_with(&self, state: AnyBox) -> AnyBox {
        (self.func)(state)
    }

    pub fn call(&self) -> AnyBox {
        (self.func)(AnyBox::new(()))
    }
}
