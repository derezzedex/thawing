use std::path::Path;

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    iced::application("A cool counter [thawing]", Counter::update, Counter::view)
        .run_with(Counter::new)
}

const ID: &'static str = "thawing";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Message {
    #[serde(skip)]
    Reload,
    Toggled(bool),
    Increment,
    Decrement,
}

#[derive(Default, serde::Serialize)]
struct Counter {
    value: i64,
    is_checked: bool,
}

impl Counter {
    fn new() -> (Self, iced::Task<Message>) {
        (
            Self::default(),
            thawing::watch_and_reload::<Message, iced::Theme, iced::Renderer>(ID, Message::Reload),
        )
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Reload => {
                tracing::info!("Reloaded!");
            }
            Message::Toggled(is_checked) => self.is_checked = is_checked,
            Message::Increment => self.value += 1,
            Message::Decrement => self.value -= 1,
        }
    }

    fn view(&self) -> iced::Element<Message> {
        thawing::component(Path::new(env!("CARGO_MANIFEST_DIR")).join("component"))
            .state(self)
            .id(ID)
            .into()
    }
}
