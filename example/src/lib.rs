use thawing::guest::{Message, State};
use thawing::widget::{button, column, text};
use thawing::{Application, Center, Element};

pub struct MyApp;

impl Application for MyApp {
    fn view(state: State) -> Element {
        column![
            button("Increment").on_press(Message::Increment),
            text(state.counter).size(50),
            button("Decrement").on_press(Message::Decrement)
        ]
        .padding(20)
        .align_x(Center)
        .into()
    }
}

thawing::thaw!(MyApp);
