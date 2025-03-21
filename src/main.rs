mod runtime;
mod types;
mod widget;

fn main() -> iced::Result {
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

impl From<runtime::host::Message> for Message {
    fn from(msg: runtime::host::Message) -> Self {
        match msg {
            runtime::host::Message::Toggled(is_checked) => Message::Toggled(is_checked),
            runtime::host::Message::Increment => Message::Increment,
            runtime::host::Message::Decrement => Message::Decrement,
        }
    }
}

impl From<&Counter> for runtime::host::State {
    fn from(state: &Counter) -> Self {
        Self {
            counter: state.value,
            toggled: state.is_checked,
        }
    }
}

#[derive(Default)]
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
            runtime::Message::Direct(message) => {
                self.state.update(message.into());
            }
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
                println!("Application thawed in {:?}", timer.elapsed() + elapsed);
            }
        }
    }

    fn view(&self) -> iced::Element<runtime::Message> {
        self.runtime.view(&self.state)
    }
}
