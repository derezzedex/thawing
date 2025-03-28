mod runtime;
mod types;
mod widget;

use std::path::Path;
use std::sync::{Arc, Mutex};

use iced::advanced::widget::{tree, Tree};
use iced::advanced::{self, layout, mouse, renderer, Layout, Shell, Widget};
use iced::{Element, Length, Size};

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    iced::application("A cool counter [thawing]", Counter::update, Counter::view).run()
}

pub const SRC_PATH: &'static str = "./example/src/lib.rs";
pub const WASM_PATH: &'static str =
    "./example/target/wasm32-unknown-unknown/debug/thawing_example.wasm";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Message {
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
    fn update(&mut self, message: Message) {
        match message {
            Message::Toggled(is_checked) => self.is_checked = is_checked,
            Message::Increment => self.value += 1,
            Message::Decrement => self.value -= 1,
            Message::Reload => {
                panic!("should reload!");
            }
        }
    }

    fn view(&self) -> iced::Element<Message> {
        Thawing::from_file(WASM_PATH, self, || Message::Reload).into()
    }
}

pub struct Thawing<'a, State, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    width: Length,
    height: Length,
    state: std::marker::PhantomData<State>,

    runtime: Arc<Mutex<runtime::State<'a, Theme, Renderer>>>,
    element: Element<'a, runtime::Message, Theme, Renderer>,
    on_reload: Box<dyn Fn() -> Message + 'a>,
}

impl<'a, State, Message, Theme, Renderer> Thawing<'a, State, Message, Theme, Renderer>
where
    State: serde::Serialize,
    Renderer: 'a + iced::advanced::Renderer + iced::advanced::text::Renderer,
    Theme: 'a
        + iced::widget::checkbox::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog,
    <Theme as iced::widget::text::Catalog>::Class<'a>: From<iced::widget::text::StyleFn<'a, Theme>>,
{
    pub fn from_file<'b>(
        path: impl AsRef<Path>,
        state: &'b State,
        on_reload: impl Fn() -> Message + 'a,
    ) -> Self {
        let runtime = runtime::State::new(path.as_ref());
        let element = runtime.view(state);

        Self {
            width: Length::Shrink,
            height: Length::Shrink,
            state: std::marker::PhantomData,
            runtime: Arc::new(Mutex::new(runtime)),
            element,
            on_reload: Box::new(on_reload),
        }
    }
}

impl<'a, State, Message, Theme, Renderer> From<Thawing<'a, State, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    'a: 'static,
    State: serde::Serialize + 'static,
    Message: 'a + serde::Serialize + serde::de::DeserializeOwned,
    Renderer: 'a + iced::advanced::Renderer + iced::advanced::text::Renderer,
    Theme: 'a
        + iced::widget::checkbox::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog,
    <Theme as iced::widget::text::Catalog>::Class<'a>: From<iced::widget::text::StyleFn<'a, Theme>>,
{
    fn from(widget: Thawing<'a, State, Message, Theme, Renderer>) -> Self {
        Element::new(widget)
    }
}

struct Inner<Theme, Renderer> {
    runtime: Arc<Mutex<runtime::State<'static, Theme, Renderer>>>,
}

impl<Theme, Renderer> Inner<Theme, Renderer>
where
    Renderer: 'static + iced::advanced::Renderer + iced::advanced::text::Renderer,
    Theme: 'static
        + iced::widget::checkbox::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog,
    <Theme as iced::widget::text::Catalog>::Class<'static>:
        From<iced::widget::text::StyleFn<'static, Theme>>,
{
    fn new<State>(runtime: Arc<Mutex<runtime::State<'static, Theme, Renderer>>>) -> Self
    where
        State: serde::Serialize,
    {
        Self { runtime }
    }
}

impl<'a, State, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Thawing<'a, State, Message, Theme, Renderer>
where
    'a: 'static,
    State: serde::Serialize + 'static,
    Message: serde::Serialize + serde::de::DeserializeOwned,
    Renderer: 'a + iced::advanced::Renderer + iced::advanced::text::Renderer,
    Theme: 'a
        + iced::widget::checkbox::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog,
    <Theme as iced::widget::text::Catalog>::Class<'a>: From<iced::widget::text::StyleFn<'a, Theme>>,
{
    fn tag(&self) -> tree::Tag {
        struct Tag<T>(T);

        tree::Tag::of::<Tag<State>>()
    }

    fn state(&self) -> tree::State {
        let state = Inner::new::<State>(Arc::clone(&self.runtime));
        tree::State::new(state)
    }

    fn children(&self) -> Vec<tree::Tree> {
        vec![Tree::new(&self.element)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.children[0].diff(&self.element);
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &self,
        tree: &mut advanced::widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.element
            .as_widget()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn advanced::Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        let mut messages = vec![];
        let mut guest = Shell::new(&mut messages);

        self.element.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            &mut guest,
            viewport,
        );

        let state = tree.state.downcast_mut::<Inner<Theme, Renderer>>();
        let runtime = Arc::clone(&state.runtime);

        shell.merge(guest, move |message| match message {
            runtime::Message::Thawing(_) => (self.on_reload)(),
            runtime::Message::Guest(closure, data) => runtime.lock().unwrap().call(closure, data),
        });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.element.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &advanced::widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        self.element.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    // fn overlay<'b>(
    //     &'b mut self,
    //     state: &'b mut Tree,
    //     layout: Layout<'_>,
    //     renderer: &Renderer,
    //     translation: iced::Vector,
    // ) -> Option<advanced::overlay::Element<'b, Message, Theme, Renderer>> {
    //     self.element
    //         .as_widget_mut()
    //         .overlay(state, layout, renderer, translation)
    // }
}
