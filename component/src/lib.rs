#[allow(warnings)]
mod bindings;

use bindings::component::iced_thawing;
use bindings::exports::component::iced_thawing::guest;
use iced_thawing::types::Element;
use iced_thawing::widget::{Button, Column, Text};
use iced_thawing::host::Message;

// enum Message {
//     Increment,
//     Decrement,
// }

struct Component;

impl guest::Guest for Component {
    type App = MyApp;
}

struct MyApp {
    value: i64,
}

impl guest::GuestApp for MyApp {
    fn new(value: i64) -> Self {
        Self { value }
    }

    fn view(&self) -> Element {
        Column::new()
            .push(
                Button::new(Text::new("Increment").into_element())
                    .on_press(Message::Decrement)
                    .into_element(),
            )
            .push(Text::new(&self.value.to_string()).size(50.0).into_element())
            .push(
                Button::new(Text::new("Decrement").into_element())
                    .on_press(Message::Increment)
                    .into_element(),
            )
            .into_element()
    }
}

bindings::export!(Component with_types_in bindings);
