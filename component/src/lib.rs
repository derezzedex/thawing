#[allow(warnings)]
mod bindings;

use bindings::exports::component::iced_thawing::guest;
// use bindings::component::iced_thawing::host;

enum Message {
    ButtonPressed,
}

struct Component;

impl guest::Guest for Component {
    type App = MyApp;
}

struct MyApp;

impl guest::GuestApp for MyApp {
    fn new() -> Self {
        todo!()
    }

    fn view(&self) -> guest::Element {
        todo!()
    }
}

bindings::export!(Component with_types_in bindings);
