use std::path::PathBuf;
use std::sync::Arc;

use iced_core::widget::{Operation, Tree};
use iced_core::{Clipboard, Event, Layout, Rectangle, Shell};
use iced_core::{layout, mouse, renderer};

use crate::Element;
use crate::{guest, runtime};

pub struct Error<Message> {
    _raw: crate::Error,
    element: Element<'static, Message>,
}

impl<Message> Error<Message> {
    pub fn new(error: crate::Error) -> Self {
        let element = failed(&error);

        Self {
            _raw: error,
            element,
        }
    }
}

pub(crate) enum View<Message> {
    None,
    Failed(Error<Message>),
    Built {
        runtime: runtime::Runtime<'static>,
        element: Element<'static, guest::Message>,
        mapper: Box<dyn Fn(guest::Message) -> Message>,
        error: Option<Error<Message>>,
    },
}

pub(crate) struct Inner<Message> {
    pub(crate) view: View<Message>,
    pub(crate) invalidated: bool,
    pub(crate) bytes: Arc<Vec<u8>>,
    pub(crate) caller: PathBuf,
}

impl<Message> Inner<Message>
where
    Message: serde::de::DeserializeOwned + 'static,
{
    pub(crate) fn new(bytes: Arc<Vec<u8>>, caller: &PathBuf) -> Self {
        let caller = caller.clone();

        Self {
            view: View::None,
            invalidated: false,
            bytes,
            caller,
        }
    }

    pub(crate) fn diff(
        &mut self,
        other: &Arc<Vec<u8>>,
        initial: &Element<'_, Message>,
        tree: &mut Tree,
    ) {
        match &self.view {
            View::Failed(error)
            | View::Built {
                error: Some(error), ..
            } => error.element.as_widget().diff(tree),
            View::None => initial.as_widget().diff(tree),
            View::Built { element, .. } => element.as_widget().diff(tree),
        }

        if Arc::ptr_eq(&self.bytes, other) {
            return;
        }

        if let View::Built {
            runtime, element, ..
        } = &mut self.view
        {
            *element = runtime.view(other);
        }

        self.bytes = Arc::clone(other);
    }

    pub fn layout(
        &self,
        initial: &Element<'_, Message>,
        tree: &mut Tree,
        renderer: &iced_widget::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        match &self.view {
            View::Failed(error)
            | View::Built {
                error: Some(error), ..
            } => error.element.as_widget().layout(tree, renderer, limits),
            View::None => initial.as_widget().layout(tree, renderer, limits),
            View::Built { element, .. } => element.as_widget().layout(tree, renderer, limits),
        }
    }

    pub fn operate(
        &self,
        initial: &Element<'_, Message>,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced_widget::Renderer,
        operation: &mut dyn Operation,
    ) {
        match &self.view {
            View::Failed(error)
            | View::Built {
                error: Some(error), ..
            } => error
                .element
                .as_widget()
                .operate(tree, layout, renderer, operation),
            View::None => initial
                .as_widget()
                .operate(tree, layout, renderer, operation),
            View::Built { element, .. } => element
                .as_widget()
                .operate(tree, layout, renderer, operation),
        }
    }

    pub fn update(
        &mut self,
        initial: &mut Element<'_, Message>,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced_widget::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if self.invalidated {
            shell.request_redraw();
            self.invalidated = false;
        }

        match &mut self.view {
            View::Failed(error)
            | View::Built {
                error: Some(error), ..
            } => error.element.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
            ),
            View::None => initial.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
            ),
            View::Built {
                element, runtime, ..
            } => {
                let mut messages = vec![];
                let mut guest = Shell::new(&mut messages);

                element.as_widget_mut().update(
                    tree, event, layout, cursor, renderer, clipboard, &mut guest, viewport,
                );

                shell.merge(guest, move |message| {
                    runtime.call(message.closure, message.data)
                });
            }
        }
    }

    pub fn mouse_interaction(
        &self,
        initial: &Element<'_, Message>,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced_widget::Renderer,
    ) -> mouse::Interaction {
        match &self.view {
            View::Failed(error)
            | View::Built {
                error: Some(error), ..
            } => error
                .element
                .as_widget()
                .mouse_interaction(tree, layout, cursor, viewport, renderer),
            View::None => initial
                .as_widget()
                .mouse_interaction(tree, layout, cursor, viewport, renderer),
            View::Built { element, .. } => element
                .as_widget()
                .mouse_interaction(tree, layout, cursor, viewport, renderer),
        }
    }

    pub fn draw(
        &self,
        initial: &Element<'_, Message>,
        tree: &Tree,
        renderer: &mut iced_widget::Renderer,
        theme: &iced_widget::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        match &self.view {
            View::Failed(error)
            | View::Built {
                error: Some(error), ..
            } => error
                .element
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport),
            View::None => initial
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport),
            View::Built { element, .. } => element
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport),
        }
    }

    pub fn overlay<'b>(
        &'b mut self,
        initial: &'b mut Element<'_, Message>,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced_widget::Renderer,
        viewport: &iced_core::Rectangle,
        translation: iced_core::Vector,
    ) -> Option<iced_core::overlay::Element<'b, Message, iced_widget::Theme, iced_widget::Renderer>>
    {
        match &mut self.view {
            View::Failed(error)
            | View::Built {
                error: Some(error), ..
            } => {
                error
                    .element
                    .as_widget_mut()
                    .overlay(tree, layout, renderer, viewport, translation)
            }
            View::None => {
                initial
                    .as_widget_mut()
                    .overlay(tree, layout, renderer, viewport, translation)
            }
            View::Built {
                element, mapper, ..
            } => element
                .as_widget_mut()
                .overlay(tree, layout, renderer, viewport, translation)
                .map(move |overlay| overlay.map(mapper)),
        }
    }
}

fn failed<'a, Message>(text: impl ToString) -> Element<'a, Message> {
    iced_widget::text(text.to_string()).size(12).into()
}
