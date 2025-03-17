use crate::host::{Message, State};
use crate::widget::{Button, Checkbox, Column, Text};
use crate::{Application, Element};

pub struct MyApp;

impl Application for MyApp {
    fn new() -> Self {
        MyApp
    }

    fn view(&self, state: State) -> Element {
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
