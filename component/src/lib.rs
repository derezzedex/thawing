#[allow(warnings)]
mod bindings;

use bindings::component::iced_thawing;
use bindings::exports::component::iced_thawing::guest;
use iced_thawing::host::Message;
use iced_thawing::types::Element;
use iced_thawing::widget::{Button, Column, Text};

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

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

    fn call(&self, state: AnyBox) -> AnyBox {
        (self.func)(state)
    }

    fn call_stateless(&self) -> AnyBox {
        (self.func)(AnyBox::new(()))
    }
}

impl guest::GuestAny for AnyBox {}

impl guest::Guest for Component {
    type App = MyApp;
    type Any = AnyBox;
}

use std::sync::{LazyLock, Mutex};
static TABLE: LazyLock<Mutex<HashMap<u32, Closure>>> =
    LazyLock::new(|| Mutex::new(HashMap::default()));

fn on_press_with(button: Button, closure: impl Fn() -> Message + Send + 'static) -> Button {
    button.on_press_with({
        let c = guest::Closure::new();
        TABLE
            .lock()
            .unwrap()
            .insert(c.id(), Closure::stateless(closure));
        c
    })
}

struct MyApp;

impl guest::GuestApp for MyApp {
    fn new() -> Self {
        TABLE
        .lock()
        .unwrap()
        .clear();

        MyApp
    }

    fn view(&self, state: i64) -> Element {
        Column::new()
            .push(
                on_press_with(
                    Button::new(Text::new("Increment").into_element()),
                    move || Message::Increment(state),
                )
                .into_element(),
            )
            .push(Text::new(&state.to_string()).size(50.0).into_element())
            .push(
                on_press_with(
                    Button::new(Text::new("Decrement").into_element()),
                    move || Message::Decrement(state),
                )
                .into_element(),
            )
            .into_element()
    }

    fn call(&self, c: guest::Closure) -> Message {
        let table = TABLE.lock().unwrap();
        let closure = table.get(&c.id()).unwrap();
        closure.call_stateless().downcast()
    }

    fn call_with(&self, _c: guest::Closure, _state: guest::Any) -> Message {
        unimplemented!()
    }
}

bindings::export!(Component with_types_in bindings);
