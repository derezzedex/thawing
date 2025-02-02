use iced::Task;

mod runtime;

fn main() -> iced::Result {
    iced::application("Component model counter", Counter::update, Counter::view)
        .run_with(Counter::new)
}

pub const PATH: &'static str = "./component/target/wasm32-unknown-unknown/debug/component.wasm";

#[derive(Debug)]
pub enum Message {
    Runtime(runtime::Message),
    FileChanged,
}

struct Counter {
    value: i64,
    state: runtime::State,
}

impl Counter {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                value: 0,
                state: runtime::State::new(PATH),
            },
            Task::stream(runtime::watch(PATH)),
        )
    }
    fn update(&mut self, message: Message) {
        match message {
            Message::Runtime(message) => match message {
                runtime::Message::Increment => {
                    self.value += 1;
                }
                runtime::Message::Decrement => {
                    self.value -= 1;
                }
            },
            Message::FileChanged => {
                println!("FileChanged!");
                self.state = runtime::State::new(PATH);
            }
        }
    }

    fn view(&self) -> iced::Element<Message> {
        self.state.view(self.value).map(Message::Runtime)
    }
}
