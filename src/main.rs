use iced::Task;

mod runtime;

fn main() -> iced::Result {
    iced::application("A cool counter [thawing]", Thawing::update, Thawing::view)
        .run_with(Thawing::new)
}

pub const PATH: &'static str = "./component/target/wasm32-unknown-unknown/debug/component.wasm";

#[derive(Debug, Clone)]
pub enum Message {
    Increment(i64),
    Decrement(i64),
}

impl From<Message> for runtime::host::Message {
    fn from(msg: Message) -> Self {
        match msg {
            Message::Increment(n) => runtime::host::Message::Increment(n),
            Message::Decrement(n) => runtime::host::Message::Decrement(n),
        }
    }
}

impl From<runtime::host::Message> for Message {
    fn from(msg: runtime::host::Message) -> Self {
        match msg {
            runtime::host::Message::Increment(n) => Message::Increment(n),
            runtime::host::Message::Decrement(n) => Message::Decrement(n),
        }
    }
}

#[derive(Default)]
struct Counter {
    value: i64,
}

impl Counter {
    fn update(&mut self, message: Message) {
        match message {
            Message::Increment(_n) => self.value += 1,
            Message::Decrement(_n) => self.value -= 1,
        }
    }
}

struct Thawing {
    inner: Counter,
    runtime: runtime::State,
}

impl Thawing {
    fn new() -> (Self, Task<runtime::Message>) {
        (
            Self {
                inner: Counter::default(),
                runtime: runtime::State::new(PATH),
            },
            Task::stream(runtime::watch(PATH)),
        )
    }

    fn update(&mut self, message: runtime::Message) {
        match message {
            runtime::Message::Direct(message) => {
                self.inner.update(message.into());
            }
            runtime::Message::Stateless(id) => {
                let message = self.runtime.call(id);
                self.inner.update(message.into());
            }
            runtime::Message::Thaw => {
                self.runtime = runtime::State::new(PATH);
            }
        }
    }

    fn view(&self) -> iced::Element<runtime::Message> {
        self.runtime.view(self.inner.value)
    }
}
