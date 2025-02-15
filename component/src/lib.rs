#[allow(warnings)]
mod bindings;

use bindings::component::iced_thawing;
use bindings::exports::component::iced_thawing::guest;
use iced_thawing::host::Message;
use iced_thawing::types::Element;
use iced_thawing::widget;

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
static TABLE: LazyLock<Mutex<HashMap<u32, Closure>>> =
    LazyLock::new(|| Mutex::new(HashMap::default()));

struct Button {
    raw: widget::Button,
}

impl Button {
    fn new(content: impl Into<Element>) -> Self {
        Self {
            raw: widget::Button::new(content.into()),
        }
    }

    fn on_press_with(mut self, closure: impl Fn() -> Message + Send + 'static) -> Self {
        let closure = closure.into();
        self.raw = self.raw.on_press_with(closure);
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

impl From<Column> for Element {
    fn from(column: Column) -> Self {
        column.raw.into_element()
    }
}

struct Component;

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
    #[allow(dead_code)]
    fn stateful<S, T>(func: impl Fn(S) -> T + Send + 'static) -> Self
    where
        S: 'static,
        T: 'static,
    {
        let wrapper = move |state: AnyBox| -> AnyBox { AnyBox::new(func(state.downcast::<S>())) };

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

    #[allow(dead_code)]
    fn call_with(&self, state: AnyBox) -> AnyBox {
        (self.func)(state)
    }

    fn call(&self) -> AnyBox {
        (self.func)(AnyBox::new(()))
    }
}

impl guest::GuestAny for AnyBox {}

impl guest::Guest for Component {
    type App = MyApp;
    type Any = AnyBox;
}

impl<F: Fn() -> Message + Send + 'static> From<F> for guest::Closure {
    fn from(f: F) -> guest::Closure {
        let closure = guest::Closure::new();
        TABLE
            .lock()
            .unwrap()
            .insert(closure.id(), Closure::stateless(f));
        closure
    }
}

struct MyApp;

impl guest::GuestApp for MyApp {
    fn new() -> Self {
        TABLE.lock().unwrap().clear();

        MyApp
    }

    fn view(&self, state: i64) -> Element {
        Column::new()
            .push(
                Button::new(Text::new("Increment"))
                    .on_press_with(move || Message::Increment(state)),
            )
            .push(Text::new(state).size(50.0))
            .push(
                Button::new(Text::new("Decrement"))
                    .on_press_with(move || Message::Decrement(state)),
            )
            .into()
    }

    fn call(&self, c: guest::Closure) -> Message {
        let table = TABLE.lock().unwrap();
        let closure = table.get(&c.id()).unwrap();
        closure.call().downcast()
    }

    fn call_with(&self, _c: guest::Closure, _state: guest::Any) -> Message {
        unimplemented!()
    }
}

bindings::export!(Component with_types_in bindings);
