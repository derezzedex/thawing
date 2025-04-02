use thawing_guest::widget::{button, checkbox, column, text};
use thawing_guest::{Application, Center, Element};

#[derive(Debug, Clone, serde::Serialize)]
enum Message {
    Toggled(bool),
    Increment,
    Decrement,
}

#[derive(serde::Deserialize)]
pub struct MyApp {
    counter: i64,
    is_toggled: bool,
}

impl Application for MyApp {
    fn view(&self) -> Element {
        column![
            checkbox("click me!", self.is_toggled).on_toggle(Message::Toggled),
            button("Increment").on_press(Message::Increment),
            text(self.counter).size(50),
            button("Decrement").on_press(Message::Decrement)
        ]
        .padding(20)
        .align_x(Center)
        .into()
    }
}

thawing_guest::thaw!(MyApp);
