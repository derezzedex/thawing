mod runtime;
mod types;
mod widget;

use std::cell::OnceCell;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use iced_core::widget::{Operation, Tree, operation, tree};
use iced_core::{Clipboard, Element, Event, Layout, Length, Rectangle, Shell, Size, Widget};
use iced_core::{layout, mouse, renderer, text};
use iced_futures::futures::channel::mpsc::channel;
use iced_futures::futures::{SinkExt, Stream, StreamExt};
use iced_futures::{futures, stream};
use iced_runtime::{Task, task};
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{DebouncedEventKind, new_debouncer};

pub fn component<'a, Message, State>(path: impl AsRef<Path>) -> Thawing<'a, Message, State> {
    Thawing::from_component(path)
}

pub struct Thawing<'a, Message, State = ()> {
    id: Option<Id>,
    width: Length,
    height: Length,

    path: PathBuf,
    bytes: Arc<Vec<u8>>,
    tree: Mutex<OnceCell<Tree>>,

    state: PhantomData<&'a State>,
    message: PhantomData<Message>,
}

impl<'a, State, Message> Thawing<'a, Message, State> {
    pub fn from_component(path: impl AsRef<Path>) -> Self {
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

    pub fn id(mut self, id: impl Into<Id>) -> Self {
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
    Renderer: 'static + iced_core::Renderer + iced_core::text::Renderer,
    Theme: 'static
        + iced_widget::checkbox::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    <Theme as iced_widget::text::Catalog>::Class<'static>:
        From<iced_widget::text::StyleFn<'static, Theme>>,
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
    Renderer: 'static + iced_core::Renderer + text::Renderer,
    Theme: 'static
        + iced_widget::checkbox::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    <Theme as iced_widget::text::Catalog>::Class<'static>:
        From<iced_widget::text::StyleFn<'static, Theme>>,
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
    Renderer: 'static + iced_core::Renderer + text::Renderer,
    Theme: 'static
        + iced_widget::checkbox::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    <Theme as iced_widget::text::Catalog>::Class<'static>:
        From<iced_widget::text::StyleFn<'static, Theme>>,
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
        let state = tree.state.downcast_mut::<Inner<Theme, Renderer>>();
        state.diff(&self.bytes);

        state.element.as_widget().diff(&mut tree.children[0]);
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &self,
        tree: &mut Tree,
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
        operation: &mut dyn Operation,
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
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
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
        viewport: &Rectangle,
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
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
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

#[derive(Debug, Clone)]
pub struct Id(pub(crate) iced_core::widget::Id);

impl Id {
    pub fn new(id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        Self(iced_core::widget::Id::new(id))
    }

    pub fn unique() -> Self {
        Self(iced_core::widget::Id::unique())
    }
}

impl From<Id> for iced_core::widget::Id {
    fn from(id: Id) -> Self {
        id.0
    }
}

impl From<&'static str> for Id {
    fn from(value: &'static str) -> Self {
        Id::new(value)
    }
}

pub fn watch<Message, Theme, Renderer>(
    id: impl Into<Id> + Clone + Send + 'static,
    path: impl AsRef<Path> + 'static,
) -> Task<()>
where
    Message: Send + 'static,
    Theme: Send + 'static,
    Renderer: Send + 'static,
{
    Task::stream(watch_file(path.as_ref())).then(move |_| reload::<Theme, Renderer>(id.clone()))
}

// TODO(derezzedex): if `watch` is used instead, a `Widget::update` call
// is needed (e.g. moving the mouse, or focusing the window) to display changes;
// this sends a message to the application, forcing a `Widget::update`
pub fn watch_and_notify<Message, Theme, Renderer>(
    id: impl Into<Id> + Clone + Send + 'static,
    path: impl AsRef<Path> + 'static,
    on_reload: Message,
) -> Task<Message>
where
    Message: Clone + Send + 'static,
    Theme: Send + 'static,
    Renderer: Send + 'static,
{
    watch::<Message, Theme, Renderer>(id, path).map(move |_| on_reload.clone())
}

pub fn reload<Theme: Send + 'static, Renderer: Send + 'static>(
    id: impl Into<Id> + Clone + Send + 'static,
) -> Task<()> {
    let id = id.into();

    struct Reload<Theme, Renderer> {
        id: iced_core::widget::Id,
        theme: PhantomData<Theme>,
        renderer: PhantomData<Renderer>,
    }

    impl<Theme: Send + 'static, Renderer: Send + 'static> Operation for Reload<Theme, Renderer> {
        fn custom(
            &mut self,
            id: Option<&iced_core::widget::Id>,
            _bounds: Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            match id {
                Some(id) if id == &self.id => {
                    if let Some(state) = state.downcast_mut::<crate::Inner<Theme, Renderer>>() {
                        state.runtime.reload();
                        state.invalidated = true;
                    }
                }
                _ => {}
            }
        }

        fn container(
            &mut self,
            _id: Option<&iced_core::widget::Id>,
            _bounds: Rectangle,
            operate_on_children: &mut dyn FnMut(&mut dyn Operation<()>),
        ) {
            operate_on_children(self)
        }

        fn finish(&self) -> operation::Outcome<()> {
            operation::Outcome::Some(())
        }
    }

    task::widget(Reload {
        id: id.into(),
        theme: PhantomData::<Theme>,
        renderer: PhantomData::<Renderer>,
    })
}

fn watch_file(path: &Path) -> impl Stream<Item = ()> + use<> {
    let path = path
        .canonicalize()
        .expect(&format!("failed to canonicalize path: {path:?}"));

    stream::channel(
        10,
        |mut output: futures::channel::mpsc::Sender<()>| async move {
            let (mut tx, mut rx) = channel(1);

            let mut debouncer = new_debouncer(Duration::from_millis(500), move |res| {
                futures::executor::block_on(async {
                    tx.send(res).await.expect("Failed to send debounce event");
                })
            })
            .expect("Failed to create file watcher");

            debouncer
                .watcher()
                .watch(&path, RecursiveMode::NonRecursive)
                .expect("Failed to watch path");

            tracing::info!("Watching {path:?}");

            loop {
                for _ in rx
                    .next()
                    .await
                    .map(Result::ok)
                    .flatten()
                    .into_iter()
                    .flat_map(|events| {
                        events
                            .into_iter()
                            .filter(|event| event.kind == DebouncedEventKind::Any)
                    })
                    .collect::<Vec<_>>()
                {
                    tracing::info!("Building component...");
                    let timer = Instant::now();
                    Command::new("cargo")
                        .args(["component", "build", "--target", "wasm32-unknown-unknown"])
                        // TODO(derezzedex): handle this properly
                        .current_dir(path.parent().unwrap().parent().unwrap())
                        .stdin(Stdio::null())
                        .output()
                        .expect("Failed to build component");
                    tracing::info!("Component built in {:?}", timer.elapsed());

                    output.send(()).await.expect("Failed to send message");
                }
            }
        },
    )
}
