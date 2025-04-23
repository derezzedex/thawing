mod guest;
mod runtime;
mod task;

use std::cell::OnceCell;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use iced_core::widget::{Operation, Tree, tree};
use iced_core::{Clipboard, Event, Layout, Length, Rectangle, Shell, Size, Widget};
use iced_core::{layout, mouse, renderer, text};

pub use iced_core::Element;
pub use serde;
pub use task::{reload, watcher};
pub use thawing_macro::data;

#[macro_export]
macro_rules! view {
    ($widget:expr) => {
        $crate::Thawing::from_view($crate::Element::from($widget), file!())
    };
}

pub fn component<'a, Message, Theme, Renderer, State>(
    path: impl AsRef<Path>,
) -> Thawing<'a, Message, Theme, Renderer, State> {
    Thawing::from_component(path)
}

#[derive(Debug, Clone)]
enum Kind {
    ViewMacro(PathBuf),
    ComponentFile(PathBuf),
}

pub struct Thawing<'a, Message, Theme, Renderer, State = ()> {
    id: Option<Id>,
    width: Length,
    height: Length,

    kind: Kind,
    initial: Option<Element<'a, Message, Theme, Renderer>>,
    bytes: Arc<Vec<u8>>,
    tree: Mutex<OnceCell<Tree>>,

    state: PhantomData<&'a State>,
    message: PhantomData<Message>,
}

impl<'a, Message, Theme, Renderer, State> Thawing<'a, Message, Theme, Renderer, State> {
    pub fn from_view(
        element: impl Into<Element<'a, Message, Theme, Renderer>>,
        file: &'static str,
    ) -> Self {
        Self {
            id: None,
            kind: Kind::ViewMacro(Path::new(file).canonicalize().unwrap()),
            initial: Some(element.into()),
            bytes: Arc::new(Vec::new()),
            width: Length::Shrink,
            height: Length::Shrink,
            tree: Mutex::new(OnceCell::new()),
            state: PhantomData,
            message: PhantomData,
        }
    }

    pub fn from_component(path: impl AsRef<Path>) -> Self {
        Self {
            id: None,
            kind: Kind::ComponentFile(path.as_ref().to_path_buf()),
            initial: None,
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

impl<'a, Message, Theme, Renderer, State> Thawing<'a, Message, Theme, Renderer, State>
where
    State: serde::Serialize,
{
    pub fn state<'b>(mut self, state: &'b State) -> Self {
        self.bytes = Arc::new(bincode::serialize(state).unwrap());
        self
    }
}

impl<'a, Message, Theme, Renderer, State> From<Thawing<'a, Message, Theme, Renderer, State>>
    for Element<'a, Message, Theme, Renderer>
where
    State: serde::Serialize + 'static,
    Message: 'static + serde::Serialize + serde::de::DeserializeOwned,
    Renderer: 'static + iced_core::Renderer + iced_core::text::Renderer,
    Theme: 'static
        + serde::Serialize
        + iced_widget::checkbox::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    <Theme as iced_widget::text::Catalog>::Class<'static>:
        From<iced_widget::text::StyleFn<'static, Theme>>,
{
    fn from(widget: Thawing<'a, Message, Theme, Renderer, State>) -> Self {
        Element::new(widget)
    }
}

enum Runtime<Theme, Renderer> {
    None,
    Built {
        runtime: runtime::Runtime<'static, Theme, Renderer>,
        element: Element<'static, guest::Message, Theme, Renderer>,
    },
}

pub(crate) struct Inner<Theme, Renderer> {
    runtime: Runtime<Theme, Renderer>,
    invalidated: bool,
    bytes: Arc<Vec<u8>>,
    kind: Kind,
}

impl<Theme, Renderer> Inner<Theme, Renderer>
where
    Renderer: 'static + iced_core::Renderer + text::Renderer,
    Theme: 'static
        + serde::Serialize
        + iced_widget::checkbox::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    <Theme as iced_widget::text::Catalog>::Class<'static>:
        From<iced_widget::text::StyleFn<'static, Theme>>,
{
    fn new(kind: &Kind, bytes: Arc<Vec<u8>>) -> Self {
        let runtime = match kind {
            Kind::ComponentFile(path) => {
                let runtime = runtime::Runtime::from_component(&path);
                let element = runtime.view(&bytes);

                Runtime::Built { runtime, element }
            }
            Kind::ViewMacro(_) => Runtime::None,
        };

        Self {
            runtime,
            invalidated: false,
            kind: kind.clone(),
            bytes,
        }
    }

    fn diff(&mut self, other: &Arc<Vec<u8>>) {
        if Arc::ptr_eq(&self.bytes, other) {
            return;
        }

        if let Runtime::Built { runtime, element } = &mut self.runtime {
            *element = runtime.view(other);
        }

        self.bytes = Arc::clone(other);
    }
}

impl<'a, Message, Theme, Renderer, State> Widget<Message, Theme, Renderer>
    for Thawing<'a, Message, Theme, Renderer, State>
where
    State: serde::Serialize + 'static,
    Message: serde::Serialize + serde::de::DeserializeOwned,
    Renderer: 'static + iced_core::Renderer + text::Renderer,
    Theme: 'static
        + serde::Serialize
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
        let state: Inner<Theme, Renderer> = Inner::new(&self.kind, Arc::clone(&self.bytes));
        if let Runtime::Built { element, .. } = &state.runtime {
            let _ = self.tree.lock().unwrap().set(Tree::new(element));
        }
        tree::State::new(state)
    }

    fn children(&self) -> Vec<Tree> {
        self.initial
            .as_ref()
            .map(|el| el.as_widget().children())
            .unwrap_or_else(|| vec![self.tree.lock().unwrap().take().unwrap()])
    }

    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<Inner<Theme, Renderer>>();
        state.diff(&self.bytes);

        match &state.runtime {
            Runtime::None => self
                .initial
                .as_ref()
                .unwrap()
                .as_widget()
                .diff(&mut tree.children[0]),
            Runtime::Built { element, .. } => element.as_widget().diff(&mut tree.children[0]),
        }
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

        match &state.runtime {
            Runtime::None => self.initial.as_ref().unwrap().as_widget().layout(
                &mut tree.children[0],
                renderer,
                limits,
            ),
            Runtime::Built { element, .. } => {
                element
                    .as_widget()
                    .layout(&mut tree.children[0], renderer, limits)
            }
        }
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

        operation.custom(id, layout.bounds(), state);
        operation.container(id, layout.bounds(), &mut |operation| match &state.runtime {
            Runtime::None => self.initial.as_ref().unwrap().as_widget().operate(
                &mut tree.children[0],
                layout,
                renderer,
                operation,
            ),
            Runtime::Built { element, .. } => {
                element
                    .as_widget()
                    .operate(&mut tree.children[0], layout, renderer, operation)
            }
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
        let state = tree.state.downcast_mut::<Inner<Theme, Renderer>>();

        if state.invalidated {
            shell.request_redraw();
            state.invalidated = false;
        }

        match &mut state.runtime {
            Runtime::None => self.initial.as_mut().unwrap().as_widget_mut().update(
                &mut tree.children[0],
                event,
                layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            ),
            Runtime::Built { element, runtime } => {
                let mut messages = vec![];
                let mut guest = Shell::new(&mut messages);

                element.as_widget_mut().update(
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
                    runtime.call(message.closure, message.data)
                });
            }
        }
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

        match &state.runtime {
            Runtime::None => self
                .initial
                .as_ref()
                .unwrap()
                .as_widget()
                .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer),
            Runtime::Built { element, .. } => element.as_widget().mouse_interaction(
                &tree.children[0],
                layout,
                cursor,
                viewport,
                renderer,
            ),
        }
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

        match &state.runtime {
            Runtime::None => self.initial.as_ref().unwrap().as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout,
                cursor,
                viewport,
            ),
            Runtime::Built { element, .. } => element.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout,
                cursor,
                viewport,
            ),
        }
    }

    // TODO(derezzedex): implement Widget::overlay
}

#[derive(Debug, Clone)]
pub struct Id(iced_core::widget::Id);

impl Id {
    pub fn new(id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        Self(iced_core::widget::Id::new(id))
    }

    pub fn unique() -> Self {
        Self(iced_core::widget::Id::unique())
    }
}

impl From<iced_core::widget::Id> for Id {
    fn from(id: iced_core::widget::Id) -> Self {
        Self(id)
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
