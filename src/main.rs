mod runtime;
mod types;
mod widget;

use std::cell::OnceCell;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use iced::advanced::widget::{tree, Tree};
use iced::advanced::{self, layout, mouse, renderer, Layout, Shell, Widget};
use iced::{Element, Length, Size};

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    iced::application("A cool counter [thawing]", Counter::update, Counter::view)
        .run_with(Counter::new)
}

const ID: &'static str = "thawing";
const SRC_PATH: &'static str = "./example/src/lib.rs";
const WASM_PATH: &'static str =
    "./example/target/wasm32-unknown-unknown/debug/thawing_example.wasm";

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
            runtime::watch_and_notify::<Message, iced::Theme, iced::Renderer>(
                ID,
                SRC_PATH,
                Message::Reload,
            ),
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
        Thawing::from_file(WASM_PATH).state(self).id(ID).into()
    }
}

pub struct Thawing<'a, Message, State = ()> {
    id: Option<runtime::Id>,
    width: Length,
    height: Length,

    path: PathBuf,
    bytes: Arc<Vec<u8>>,
    tree: Mutex<OnceCell<Tree>>,

    state: PhantomData<&'a State>,
    message: PhantomData<Message>,
}

impl<'a, State, Message> Thawing<'a, Message, State> {
    pub fn from_file(path: impl AsRef<Path>) -> Self {
        Self {
            id: None,
            path: path.as_ref().to_path_buf(),
            bytes: Arc::new(Vec::new()),
            width: Length::Shrink,
            height: Length::Shrink,
            tree: Mutex::new(OnceCell::new()),
            state: PhantomData,
            message: PhantomData,
        }
    }

    pub fn id(mut self, id: impl Into<runtime::Id>) -> Self {
        self.id = Some(id.into());
        self
    }
}

impl<'a, State, Message> Thawing<'a, Message, State>
where
    State: serde::Serialize,
{
    pub fn state<'b>(mut self, state: &'b State) -> Self {
        self.bytes = Arc::new(bincode::serialize(state).unwrap());
        self
    }
}

impl<'a, State, Message, Theme, Renderer> From<Thawing<'a, Message, State>>
    for Element<'a, Message, Theme, Renderer>
where
    State: serde::Serialize + 'static,
    Message: 'static + serde::Serialize + serde::de::DeserializeOwned,
    Renderer: 'static + iced::advanced::Renderer + iced::advanced::text::Renderer,
    Theme: 'static
        + iced::widget::checkbox::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog,
    <Theme as iced::widget::text::Catalog>::Class<'static>:
        From<iced::widget::text::StyleFn<'static, Theme>>,
{
    fn from(widget: Thawing<'a, Message, State>) -> Self {
        Element::new(widget)
    }
}

pub(crate) struct Inner<Theme, Renderer> {
    invalidated: bool,
    bytes: Arc<Vec<u8>>,
    runtime: runtime::State<'static, Theme, Renderer>,
    element: Element<'static, runtime::Message, Theme, Renderer>,
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
    fn new(path: PathBuf, bytes: Arc<Vec<u8>>) -> Self {
        let runtime = runtime::State::new(&path);
        let element = runtime.view(&bytes);

        Self {
            invalidated: false,
            bytes,
            runtime,
            element,
        }
    }

    fn diff(&mut self, other: &Arc<Vec<u8>>) {
        if Arc::ptr_eq(&self.bytes, other) {
            return;
        }

        self.element = self.runtime.view(other);
        self.bytes = Arc::clone(other);
    }
}

impl<'a, State, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Thawing<'a, Message, State>
where
    State: serde::Serialize + 'static,
    Message: serde::Serialize + serde::de::DeserializeOwned,
    Renderer: 'static + iced::advanced::Renderer + iced::advanced::text::Renderer,
    Theme: 'static
        + iced::widget::checkbox::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog,
    <Theme as iced::widget::text::Catalog>::Class<'static>:
        From<iced::widget::text::StyleFn<'static, Theme>>,
{
    fn tag(&self) -> tree::Tag {
        struct Tag<T>(T);

        tree::Tag::of::<Tag<State>>()
    }

    fn state(&self) -> tree::State {
        let state: Inner<Theme, Renderer> = Inner::new(self.path.clone(), Arc::clone(&self.bytes));
        let _ = self.tree.lock().unwrap().set(Tree::new(&state.element));
        tree::State::new(state)
    }

    fn children(&self) -> Vec<Tree> {
        vec![self.tree.lock().unwrap().take().unwrap()]
    }

    fn diff(&self, tree: &mut Tree) {
        tracing::warn!("diffing!");
        let state = tree.state.downcast_mut::<Inner<Theme, Renderer>>();
        state.diff(&self.bytes);

        state.element.as_widget().diff(&mut tree.children[0]);
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
        let state = tree.state.downcast_ref::<Inner<Theme, Renderer>>();

        state
            .element
            .as_widget()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn advanced::widget::Operation,
    ) {
        let id = self.id.as_ref().map(|id| &id.0);
        let state = tree.state.downcast_mut::<Inner<Theme, Renderer>>();

        operation.custom(id, layout.bounds(), &mut *state);
        operation.container(id, layout.bounds(), &mut |operation| {
            state
                .element
                .as_widget()
                .operate(&mut tree.children[0], layout, renderer, operation);
        });
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

        let state = tree.state.downcast_mut::<Inner<Theme, Renderer>>();

        if state.invalidated {
            shell.invalidate_widgets();
            shell.request_redraw();
            state.invalidated = false;
        }

        state.element.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            &mut guest,
            viewport,
        );

        shell.merge(guest, move |message| {
            state.runtime.call(message.closure, message.data)
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
        let state = tree.state.downcast_ref::<Inner<Theme, Renderer>>();

        state.element.as_widget().mouse_interaction(
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
        let state = tree.state.downcast_ref::<Inner<Theme, Renderer>>();

        state.element.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    // TODO(derezzedex): implement Widget::overlay
}
