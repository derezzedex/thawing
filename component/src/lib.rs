#[allow(warnings)]
mod bindings;

use bindings::exports::thawing::core::guest;
use bindings::exports::thawing::core::runtime;
use bindings::thawing::core;
use core::host;
use core::host::Message;
use core::types::Element;
use core::widget;

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

static TABLE: LazyLock<Mutex<HashMap<u32, Closure>>> =
    LazyLock::new(|| Mutex::new(HashMap::default()));

struct Component;

struct ThawingTable;

impl runtime::Guest for Component {
    type Table = ThawingTable;
}

impl runtime::GuestTable for ThawingTable {
    fn new() -> Self {
        TABLE.lock().unwrap().clear();

        ThawingTable
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

struct Button {
    raw: widget::Button,
}

impl Button {
    fn new(content: impl Into<Element>) -> Self {
        Self {
            raw: widget::Button::new(content.into()),
        }
    }

    fn on_press_with(mut self, f: impl Fn() -> Message + Send + 'static) -> Self {
        let closure = runtime::Closure::new();
        TABLE
            .lock()
            .unwrap()
            .insert(closure.id(), Closure::stateless(f));
        self.raw = self.raw.on_press_with(closure);
        self
    }
}

struct Checkbox {
    raw: widget::Checkbox,
}

impl Checkbox {
    fn new(label: impl Into<String>, is_checked: bool) -> Self {
        Self {
            raw: widget::Checkbox::new(&label.into(), is_checked),
        }
    }

    fn on_toggle(mut self, f: impl Fn(bool) -> Message + Send + 'static) -> Self {
        let guest_fn = runtime::Closure::new();
        TABLE
            .lock()
            .unwrap()
            .insert(guest_fn.id(), Closure::stateful(f));
        self.raw = self.raw.on_toggle(guest_fn);
        self
    }
}

struct Column {
    raw: widget::Column,
}

impl Column {
    fn new() -> Self {
        Self {
            raw: widget::Column::new(),
        }
    }

    fn push(mut self, content: impl Into<Element>) -> Self {
        let el = content.into();
        self.raw = self.raw.push(el);
        self
    }
}

struct Text {
    raw: widget::Text,
}

impl Text {
    fn new(fragment: impl ToString) -> Self {
        Self {
            raw: widget::Text::new(&fragment.to_string()),
        }
    }

    fn size(mut self, size: f32) -> Self {
        self.raw = self.raw.size(size.into());
        self
    }
}

impl From<Text> for Element {
    fn from(text: Text) -> Self {
        text.raw.into_element()
    }
}

impl From<Button> for Element {
    fn from(button: Button) -> Self {
        button.raw.into_element()
    }
}

impl From<Checkbox> for Element {
    fn from(checkbox: Checkbox) -> Self {
        checkbox.raw.into_element()
    }
}

impl From<Column> for Element {
    fn from(column: Column) -> Self {
        column.raw.into_element()
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
    fn stateful<S, T>(func: impl Fn(S) -> T + Send + 'static) -> Self
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

    fn stateless<T>(func: impl Fn() -> T + Send + 'static) -> Self
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

struct MyApp;

impl guest::Guest for Component {
    type App = MyApp;
}

impl guest::GuestApp for MyApp {
    fn new() -> Self {
        MyApp
    }

    fn view(&self, state: host::State) -> Element {
        Column::new()
            .push(Checkbox::new("stateful closure test", state.toggled).on_toggle(Message::Toggled))
            .push(
                Button::new(Text::new("Increment"))
                    .on_press_with(move || Message::Increment(state.counter)),
            )
            .push(Text::new(state.counter).size(50.0))
            .push(
                Button::new(Text::new("Decrement"))
                    .on_press_with(move || Message::Decrement(state.counter)),
            )
            .into()
    }
}

bindings::export!(Component with_types_in bindings);
