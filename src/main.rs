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

#[derive(Debug, Clone, serde::Deserialize)]
pub enum Message {
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
            runtime::Message::Guest(id, data) => {
                let bytes = match data {
                    Some(bytes) => self.runtime.call_with(id, bytes),
                    None => self.runtime.call(id),
                };

                let message = bincode::deserialize(&bytes).unwrap();
                self.state.update(message);
            }
            runtime::Message::Thawing(elapsed) => {
                let timer = std::time::Instant::now();
                self.runtime.reload();
                tracing::info!("Application thawed in {:?}", timer.elapsed() + elapsed);
            }
        }
    }

    fn view(&self) -> iced::Element<runtime::Message> {
        self.runtime.view(bincode::serialize(&self.state).unwrap())
    }
}
