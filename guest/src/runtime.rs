use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::guest::Bytes;

pub static TABLE: LazyLock<Mutex<HashMap<u32, Closure>>> =
    LazyLock::new(|| Mutex::new(HashMap::default()));

pub struct Closure {
    func: Box<dyn Fn(Bytes) -> Bytes + Send>,
}

impl Closure {
    pub fn stateful<S, T>(func: impl Fn(S) -> T + Send + 'static) -> Self
    where
        S: serde::de::DeserializeOwned + 'static,
        T: serde::Serialize + 'static,
    {
        let wrapper = move |bytes: Bytes| -> Bytes {
            let msg = func(bincode::deserialize(&bytes).unwrap());
            bincode::serialize(&msg).unwrap()
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
        let wrapper = move |bytes: Bytes| -> Bytes {
            let msg = func(&bincode::deserialize(&bytes).unwrap());
            bincode::serialize(&msg).unwrap()
        };

        Self {
            func: Box::new(wrapper),
        }
    }

    pub fn stateless<T>(func: impl Fn() -> T + Send + 'static) -> Self
    where
        T: serde::Serialize + 'static,
    {
        let wrapper = move |_state: Bytes| -> Bytes { bincode::serialize(&func()).unwrap() };

        Self {
            func: Box::new(wrapper),
        }
    }

    pub fn call_with(&self, state: Bytes) -> Bytes {
        (self.func)(state)
    }

    pub fn call(&self) -> Bytes {
        (self.func)(Vec::new())
    }
}
