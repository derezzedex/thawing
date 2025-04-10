use iced::widget::{button, checkbox, column, text};
use iced::{Center, Element};

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    iced::application("A cool counter [thawing]", Counter::update, Counter::view)
        .run_with(Counter::new)
}

const ID: &'static str = "thawing";

#[derive(Debug, Clone)]
#[thawing::data]
enum Change {
    Increment,
    Decrement,
}

#[derive(Debug, Clone)]
#[thawing::data(message)]
enum Message {
    Reloaded,
    Toggled(bool),
    Change(Change),
}

#[derive(Default)]
#[thawing::data(state)]
struct Counter {
    value: i64,
    is_checked: bool,
}

impl Counter {
    fn new() -> (Self, iced::Task<Message>) {
        (
            Self::default(),
            thawing::watcher::<iced::Theme, iced::Renderer>(ID).map(|_| Message::Reloaded),
        )
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Reloaded => {
                tracing::info!("Reloaded!");
            }
            Message::Toggled(is_checked) => self.is_checked = is_checked,
            Message::Change(Change::Increment) => self.value += 1,
            Message::Change(Change::Decrement) => self.value -= 1,
        }
    }

    fn view(&self) -> Element<Message> {
        thawing::view![
            column![
                checkbox("click me!", self.is_checked).on_toggle(Message::Toggled),
                button("Increment").on_press(Message::Change(Change::Increment)),
                text(self.value).size(50),
                button("Decrement").on_press(Message::Change(Change::Decrement))
            ]
            .padding(20)
            .align_x(Center)
        ]
        .state(self)
        .id(ID)
        .into()
    }
}
