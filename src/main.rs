mod runtime;
mod types;
mod widget;

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    iced::application("A cool counter [thawing]", Thawing::update, Thawing::view)
        .run_with(Thawing::new)
}

pub const SRC_PATH: &'static str = "./example/src/lib.rs";
pub const WASM_PATH: &'static str =
    "./example/target/wasm32-unknown-unknown/debug/thawing_example.wasm";

#[derive(Debug, Clone)]
pub enum Message {
    Toggled(bool),
    Increment,
    Decrement,
}

impl From<runtime::guest::Message> for Message {
    fn from(msg: runtime::guest::Message) -> Self {
        match msg {
            runtime::guest::Message::Toggled(is_checked) => Message::Toggled(is_checked),
            runtime::guest::Message::Increment => Message::Increment,
            runtime::guest::Message::Decrement => Message::Decrement,
        }
    }
}

#[derive(Default, serde::Serialize)]
struct Counter {
    value: i64,
    is_checked: bool,
}

impl Counter {
    fn update(&mut self, message: Message) {
        match message {
            Message::Toggled(is_checked) => self.is_checked = is_checked,
            Message::Increment => self.value += 1,
            Message::Decrement => self.value -= 1,
        }
    }
}

struct Thawing {
    state: Counter,
    runtime: runtime::State,
}

impl Thawing {
    fn new() -> (Self, iced::Task<runtime::Message>) {
        (
            Self {
                state: Counter::default(),
                runtime: runtime::State::new(WASM_PATH),
            },
            runtime::watch(SRC_PATH),
        )
    }

    fn update(&mut self, message: runtime::Message) {
        match message {
            runtime::Message::Stateless(id) => {
                let message = self.runtime.call(id);
                self.state.update(message.into());
            }
            runtime::Message::Stateful(id, state) => {
                let message = self.runtime.call_with(id, state);
                self.state.update(message.into());
            }
            runtime::Message::Thawing(elapsed) => {
                let timer = std::time::Instant::now();
                self.runtime.thaw();
                tracing::info!("Application thawed in {:?}", timer.elapsed() + elapsed);
            }
        }
    }

    fn view(&self) -> iced::Element<runtime::Message> {
        self.runtime.view(bincode::serialize(&self.state).unwrap())
    }
}
