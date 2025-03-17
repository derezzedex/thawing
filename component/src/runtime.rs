use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::bindings::exports::thawing::core::guest;
use crate::bindings::exports::thawing::core::runtime;
use crate::core::host::Message;

pub use guest::GuestApp as Application;

pub static TABLE: LazyLock<Mutex<HashMap<u32, Closure>>> =
    LazyLock::new(|| Mutex::new(HashMap::default()));

pub struct Component;

crate::bindings::export!(Component with_types_in crate::bindings);

impl guest::Guest for Component {
    type App = crate::app::MyApp;
}

impl runtime::Guest for Component {
    type Table = Table;
}

pub struct Table;

impl runtime::GuestTable for Table {
    fn new() -> Self {
        TABLE.lock().unwrap().clear();

        Table
    }

    fn call(&self, c: runtime::Closure) -> Message {
        let table = TABLE.lock().unwrap();
        let closure = table.get(&c.id()).unwrap();
        closure.call().downcast()
    }

    fn call_with(&self, c: runtime::Closure, state: runtime::Bytes) -> Message {
        let table = TABLE.lock().unwrap();
        let closure = table.get(&c.id()).unwrap();
        closure.call_with(AnyBox::new(state)).downcast()
    }
}

pub struct AnyBox(Box<dyn std::any::Any>);

impl AnyBox {
    fn new<T: 'static>(value: T) -> Self {
        Self(Box::new(value))
    }

    fn downcast<T: 'static>(self) -> T {
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
        T: 'static,
    {
        let wrapper = move |state: AnyBox| -> AnyBox {
            let bytes = state.downcast::<runtime::Bytes>();
            AnyBox::new(func(bincode::deserialize(&bytes).unwrap()))
        };

        Self {
            func: Box::new(wrapper),
        }
    }

    pub fn stateless<T>(func: impl Fn() -> T + Send + 'static) -> Self
    where
        T: 'static,
    {
        let wrapper = move |_state: AnyBox| -> AnyBox { AnyBox::new(func()) };

        Self {
            func: Box::new(wrapper),
        }
    }

    fn call_with(&self, state: AnyBox) -> AnyBox {
        (self.func)(state)
    }

    fn call(&self) -> AnyBox {
        (self.func)(AnyBox::new(()))
    }
}
