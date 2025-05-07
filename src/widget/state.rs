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

pub enum State<Message> {
    Loading {
        bytes: Arc<Vec<u8>>,
        caller: PathBuf,
    },
    Loaded(Result<Inner<Message>, Error<Message>>),
}

pub struct Inner<Message> {
    runtime: runtime::Runtime<'static>,
    element: Element<'static, guest::Message>,
    error: Option<Error<Message>>,
    mapper: Box<dyn Fn(guest::Message) -> Message>,
    bytes: Arc<Vec<u8>>,
    invalidated: bool,
}

impl<Message> State<Message> {
    pub fn new(bytes: &Arc<Vec<u8>>, caller: &PathBuf) -> Self {
        let bytes = Arc::clone(bytes);
        let caller = caller.clone();

        Self::Loading { bytes, caller }
    }

    pub fn error(&mut self, error: Option<Error<Message>>) {
        match self {
            State::Loading { .. } => {
                if let Some(error) = error {
                    *self = State::Loaded(Err(error))
                }
            }
            State::Loaded(Err(previous)) => {
                if let Some(error) = error {
                    *previous = error
                }
            }
            State::Loaded(Ok(inner)) => inner.error = error,
        }
    }

    pub fn reload(&mut self) {
        if let State::Loaded(Ok(inner)) = self {
            let timer = std::time::Instant::now();
            inner.runtime.reload();
            tracing::info!("Reloaded in {:?}", timer.elapsed());
        }
    }
}

impl<Message> State<Message>
where
    Message: serde::de::DeserializeOwned + 'static,
{
    pub fn loaded(runtime: runtime::Runtime<'static>, bytes: &Arc<Vec<u8>>) -> Self {
        let bytes = Arc::clone(bytes);
        let element = runtime.view(&bytes);
        let mapper = {
            let runtime = runtime.state();
            Box::new(move |message: guest::Message| runtime.call(message.closure, message.data))
        };

        let inner = Inner {
            runtime,
            element,
            error: None,
            mapper,
            bytes,
            invalidated: true,
        };

        Self::Loaded(Ok(inner))
    }

    pub fn diff(&mut self, other: &Arc<Vec<u8>>, initial: &Element<'_, Message>, tree: &mut Tree) {
        match self {
            State::Loading { .. } => initial.as_widget().diff(tree),
            State::Loaded(Err(error)) => error.element.as_widget().diff(tree),
            State::Loaded(Ok(inner)) => {
                if !Arc::ptr_eq(&inner.bytes, other) {
                    inner.bytes = Arc::clone(other);
                    inner.element = inner.runtime.view(other);
                }

                inner.element.as_widget().diff(tree)
            }
        }
    }

    pub fn layout(
        &self,
        initial: &Element<'_, Message>,
        tree: &mut Tree,
        renderer: &iced_widget::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        match self {
            State::Loaded(Err(error)) => error.element.as_widget().layout(tree, renderer, limits),
            State::Loading { .. } => initial.as_widget().layout(tree, renderer, limits),
            State::Loaded(Ok(inner)) => inner.element.as_widget().layout(tree, renderer, limits),
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
        match self {
            State::Loaded(Err(error)) => error
                .element
                .as_widget()
                .operate(tree, layout, renderer, operation),
            State::Loading { .. } => initial
                .as_widget()
                .operate(tree, layout, renderer, operation),
            State::Loaded(Ok(inner)) => inner
                .element
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
        match self {
            State::Loaded(Err(error)) => error.element.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
            ),
            State::Loading { .. } => initial.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
            ),
            State::Loaded(Ok(inner)) => {
                if inner.invalidated {
                    shell.request_redraw();
                    inner.invalidated = false;
                }

                let mut messages = vec![];
                let mut guest = Shell::new(&mut messages);

                inner.element.as_widget_mut().update(
                    tree, event, layout, cursor, renderer, clipboard, &mut guest, viewport,
                );

                let runtime = inner.runtime.state();
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
        match self {
            State::Loaded(Err(error)) => error
                .element
                .as_widget()
                .mouse_interaction(tree, layout, cursor, viewport, renderer),
            State::Loading { .. } => initial
                .as_widget()
                .mouse_interaction(tree, layout, cursor, viewport, renderer),
            State::Loaded(Ok(inner)) => inner
                .element
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
        match self {
            State::Loaded(Err(error)) => error
                .element
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport),
            State::Loading { .. } => initial
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport),
            State::Loaded(Ok(inner)) => inner
                .element
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
        match self {
            State::Loaded(Err(error)) => {
                error
                    .element
                    .as_widget_mut()
                    .overlay(tree, layout, renderer, viewport, translation)
            }
            State::Loading { .. } => {
                initial
                    .as_widget_mut()
                    .overlay(tree, layout, renderer, viewport, translation)
            }
            State::Loaded(Ok(inner)) => inner
                .element
                .as_widget_mut()
                .overlay(tree, layout, renderer, viewport, translation)
                .map(|overlay| overlay.map(&inner.mapper)),
        }
    }
}

fn failed<'a, Message>(text: impl ToString) -> Element<'a, Message> {
    iced_widget::text(text.to_string()).size(12).into()
}
