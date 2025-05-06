mod id;
mod state;

use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced_core::widget::{Operation, Tree, tree};
use iced_core::{Clipboard, Event, Layout, Length, Rectangle, Shell, Size, Widget};
use iced_core::{layout, mouse, renderer};

use crate::Element;
pub use id::Id;
pub(crate) use state::{Error, Inner, View};

pub struct Thawing<'a, Message, State = ()> {
    id: Option<Id>,
    width: Length,
    height: Length,

    caller: PathBuf,
    initial: Element<'a, Message>,
    bytes: Arc<Vec<u8>>,

    state: PhantomData<&'a State>,
}

impl<'a, Message, State> Thawing<'a, Message, State> {
    pub fn from_view(element: impl Into<Element<'a, Message>>, file: &'static str) -> Self {
        Self {
            id: None,
            caller: Path::new(file).canonicalize().unwrap(),
            initial: element.into(),
            bytes: Arc::new(Vec::new()),
            width: Length::Shrink,
            height: Length::Shrink,
            state: PhantomData,
        }
    }

    pub fn id(mut self, id: impl Into<Id>) -> Self {
        self.id = Some(id.into());
        self
    }
}

impl<'a, Message, State> Thawing<'a, Message, State>
where
    State: serde::Serialize,
{
    pub fn state<'b>(mut self, state: &'b State) -> Self {
        self.bytes = Arc::new(bincode::serialize(state).unwrap());
        self
    }
}

impl<'a, Message, State> From<Thawing<'a, Message, State>> for Element<'a, Message>
where
    State: serde::Serialize + 'static,
    Message: 'static + serde::Serialize + serde::de::DeserializeOwned,
{
    fn from(widget: Thawing<'a, Message, State>) -> Self {
        Element::new(widget)
    }
}

impl<'a, Message, State> Widget<Message, iced_widget::Theme, iced_widget::Renderer>
    for Thawing<'a, Message, State>
where
    State: serde::Serialize + 'static,
    Message: serde::Serialize + serde::de::DeserializeOwned + 'static,
{
    fn tag(&self) -> tree::Tag {
        struct Tag<T>(T);

        tree::Tag::of::<Tag<State>>()
    }

    fn state(&self) -> tree::State {
        let state = Inner::<Message>::new(Arc::clone(&self.bytes), &self.caller);
        tree::State::new(state)
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.initial)]
    }

    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<Inner<Message>>();

        state.diff(&self.bytes, &self.initial, &mut tree.children[0]);
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &self,
        tree: &mut Tree,
        renderer: &iced_widget::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_ref::<Inner<Message>>();

        state.layout(&self.initial, &mut tree.children[0], renderer, limits)
    }

    fn operate(
        &self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced_widget::Renderer,
        operation: &mut dyn Operation,
    ) {
        let id = self.id.as_ref().map(|id| &id.0);
        let state = tree.state.downcast_mut::<Inner<Message>>();

        operation.custom(id, layout.bounds(), state);
        operation.container(id, layout.bounds(), &mut |operation| {
            state.operate(
                &self.initial,
                &mut tree.children[0],
                layout,
                renderer,
                operation,
            )
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced_widget::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<Inner<Message>>();

        state.update(
            &mut self.initial,
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced_widget::Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<Inner<Message>>();

        state.mouse_interaction(
            &self.initial,
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
        renderer: &mut iced_widget::Renderer,
        theme: &iced_widget::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<Inner<Message>>();

        state.draw(
            &self.initial,
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced_widget::Renderer,
        viewport: &iced_core::Rectangle,
        translation: iced_core::Vector,
    ) -> Option<iced_core::overlay::Element<'b, Message, iced_widget::Theme, iced_widget::Renderer>>
    {
        let state = tree.state.downcast_mut::<Inner<Message>>();

        state.overlay(
            &mut self.initial,
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
