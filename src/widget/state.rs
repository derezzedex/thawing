use std::path::PathBuf;
use std::sync::Arc;

use iced_core::widget::{Operation, Tree};
use iced_core::{Clipboard, Event, Layout, Rectangle, Shell};
use iced_core::{layout, mouse, renderer};

use crate::Element;
use crate::{guest, runtime};

pub struct Error<Message> {
    element: Element<'static, Message>,
    invalidated: bool,
}

impl<Message: 'static> Error<Message> {
    pub fn new(error: crate::Error) -> Self {
        let element = failed(&error);

        Self {
            element,
            invalidated: true,
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

impl<Message> std::fmt::Debug for State<Message> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Loading { .. } => f.write_str("State::Loading {..}"),
            Self::Loaded(Ok(_)) => f.write_str("State::Loaded(Ok(..))"),
            Self::Loaded(Err(_)) => f.write_str("State::Loaded(Err(..))"),
        }
    }
}

pub struct Inner<Message> {
    runtime: runtime::Runtime<'static>,
    element: Result<Element<'static, guest::Message>, Error<Message>>,
    mapper: Box<dyn Fn(guest::Message) -> Message>,
    bytes: Arc<Vec<u8>>,
    invalidated: bool,
}

impl<Message> Inner<Message> {
    pub fn engine(&self) -> runtime::Engine<'static> {
        self.runtime.engine()
    }
}

impl<Message> Inner<Message>
where
    Message: serde::de::DeserializeOwned + 'static,
{
    pub fn diff(&self, tree: &mut Tree) {
        match &self.element {
            Ok(element) => element.as_widget().diff(tree),
            Err(error) => error.element.as_widget().diff(tree),
        }
    }

    pub fn layout(
        &self,
        tree: &mut Tree,
        renderer: &iced_widget::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        match &self.element {
            Ok(element) => element.as_widget().layout(tree, renderer, limits),
            Err(error) => error.element.as_widget().layout(tree, renderer, limits),
        }
    }

    pub fn operate(
        &self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced_widget::Renderer,
        operation: &mut dyn Operation,
    ) {
        match &self.element {
            Ok(element) => element
                .as_widget()
                .operate(tree, layout, renderer, operation),
            Err(error) => error
                .element
                .as_widget()
                .operate(tree, layout, renderer, operation),
        }
    }

    pub fn update(
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
        if self.invalidated {
            shell.request_redraw();
            self.invalidated = false;
        }

        match &mut self.element {
            Ok(element) => {
                let mut messages = vec![];
                let mut guest = Shell::new(&mut messages);

                element.as_widget_mut().update(
                    tree, event, layout, cursor, renderer, clipboard, &mut guest, viewport,
                );

                let runtime = self.runtime.state();
                shell.merge(guest, move |message| {
                    runtime.call(message.closure, message.data)
                });
            }
            Err(error) => error.element.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
            ),
        }
    }

    pub fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced_widget::Renderer,
    ) -> mouse::Interaction {
        match &self.element {
            Ok(element) => element
                .as_widget()
                .mouse_interaction(tree, layout, cursor, viewport, renderer),
            Err(error) => error
                .element
                .as_widget()
                .mouse_interaction(tree, layout, cursor, viewport, renderer),
        }
    }

    pub fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced_widget::Renderer,
        theme: &iced_widget::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        match &self.element {
            Ok(element) => element
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport),
            Err(error) => error
                .element
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport),
        }
    }

    pub fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced_widget::Renderer,
        viewport: &iced_core::Rectangle,
        translation: iced_core::Vector,
    ) -> Option<iced_core::overlay::Element<'b, Message, iced_widget::Theme, iced_widget::Renderer>>
    {
        match &mut self.element {
            Ok(element) => element
                .as_widget_mut()
                .overlay(tree, layout, renderer, viewport, translation)
                .map(|overlay| overlay.map(&self.mapper)),
            Err(error) => {
                error
                    .element
                    .as_widget_mut()
                    .overlay(tree, layout, renderer, viewport, translation)
            }
        }
    }
}

impl<Message: 'static> State<Message> {
    pub fn new(
        bytes: &Result<Arc<Vec<u8>>, crate::Error>,
        caller: &Result<PathBuf, crate::Error>,
    ) -> Self {
        let bytes = match bytes {
            Ok(bytes) => Arc::clone(bytes),
            Err(error) => return Self::failed(error),
        };

        let caller = match caller {
            Ok(caller) => caller.clone(),
            Err(error) => return Self::failed(error),
        };

        Self::Loading { bytes, caller }
    }

    pub fn failed(error: &crate::Error) -> Self {
        Self::Loaded(Err(Error::new(error.clone())))
    }

    pub fn error(&mut self, error: Option<Error<Message>>) {
        let error = if let Some(error) = error {
            error
        } else {
            return;
        };

        match self {
            State::Loading { .. } => *self = State::Loaded(Err(error)),
            State::Loaded(Err(previous)) => *previous = error,
            State::Loaded(Ok(inner)) => inner.element = Err(error),
        }
    }

    pub fn reload(&mut self, state: Result<runtime::State<'static>, crate::Error>) {
        if let State::Loaded(Ok(inner)) = self {
            let timer = std::time::Instant::now();
            if let Err(error) = inner.runtime.reload(state) {
                tracing::error!("Failed to reload: {error:?}");
                inner.element = Err(Error::new(error));
                return;
            }

            inner.element = inner.runtime.view(&inner.bytes).map_err(Error::new);
            inner.invalidated = true;
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
        let element = runtime.view(&bytes).map_err(Error::new);
        let mapper = {
            let runtime = runtime.state();
            Box::new(move |message: guest::Message| runtime.call(message.closure, message.data))
        };

        let inner = Inner {
            runtime,
            element,
            mapper,
            bytes,
            invalidated: true,
        };

        Self::Loaded(Ok(inner))
    }

    pub fn diff(
        &mut self,
        other: &Result<Arc<Vec<u8>>, crate::Error>,
        initial: &Element<'_, Message>,
        tree: &mut Vec<Tree>,
    ) {
        match self {
            State::Loading { .. } => initial.as_widget().diff(&mut tree[0]),
            State::Loaded(Err(error)) => {
                initial.as_widget().diff(&mut tree[0]);

                if tree.get(1).is_none() {
                    tree.push(Tree::new(error.element.as_widget()));
                }
                error.element.as_widget().diff(&mut tree[1]);
            }
            State::Loaded(Ok(inner)) => {
                match other {
                    Err(error) => {
                        inner.element = Err(Error::new(error.clone()));
                    }
                    Ok(other) => {
                        if !Arc::ptr_eq(&inner.bytes, other) {
                            inner.bytes = Arc::clone(other);
                            inner.element = inner.runtime.view(other).map_err(Error::new);
                        }
                    }
                }

                inner.diff(&mut tree[0])
            }
        }
    }

    pub fn layout(
        &self,
        initial: &Element<'_, Message>,
        tree: &mut Vec<Tree>,
        renderer: &iced_widget::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        match self {
            State::Loaded(Err(error)) => {
                if tree.get(1).is_none() {
                    return initial.as_widget().layout(&mut tree[0], renderer, limits);
                }

                let base = initial.as_widget().layout(&mut tree[0], renderer, limits);
                let overlay = error
                    .element
                    .as_widget()
                    .layout(&mut tree[1], renderer, limits);

                layout::Node::with_children(limits.max(), vec![base, overlay])
            }
            State::Loading { .. } => initial.as_widget().layout(&mut tree[0], renderer, limits),
            State::Loaded(Ok(inner)) => inner.layout(&mut tree[0], renderer, limits),
        }
    }

    pub fn operate(
        &self,
        initial: &Element<'_, Message>,
        tree: &mut Vec<Tree>,
        layout: Layout<'_>,
        renderer: &iced_widget::Renderer,
        operation: &mut dyn Operation,
    ) {
        match self {
            State::Loaded(Err(error)) => {
                if error.invalidated {
                    return;
                }

                initial.as_widget().operate(
                    &mut tree[0],
                    layout.children().nth(0).unwrap(),
                    renderer,
                    operation,
                );

                if tree.get(1).is_none() {
                    return;
                }

                error.element.as_widget().operate(
                    &mut tree[1],
                    layout.children().nth(1).unwrap(),
                    renderer,
                    operation,
                )
            }
            State::Loading { .. } => {
                initial
                    .as_widget()
                    .operate(&mut tree[0], layout, renderer, operation)
            }
            State::Loaded(Ok(inner)) => inner.operate(&mut tree[0], layout, renderer, operation),
        }
    }

    pub fn update(
        &mut self,
        initial: &mut Element<'_, Message>,
        tree: &mut Vec<Tree>,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced_widget::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        match self {
            State::Loaded(Err(error)) => {
                initial.as_widget_mut().update(
                    &mut tree[0],
                    event,
                    layout.children().nth(0).unwrap(),
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                );

                if error.invalidated {
                    shell.invalidate_widgets();
                    error.invalidated = false;
                }

                error.element.as_widget_mut().update(
                    &mut tree[1],
                    event,
                    layout.children().nth(1).unwrap(),
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                )
            }
            State::Loading { .. } => initial.as_widget_mut().update(
                &mut tree[0],
                event,
                layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            ),
            State::Loaded(Ok(inner)) => {
                inner.update(
                    &mut tree[0],
                    event,
                    layout,
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                );
            }
        }
    }

    pub fn mouse_interaction(
        &self,
        initial: &Element<'_, Message>,
        tree: &Vec<Tree>,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced_widget::Renderer,
    ) -> mouse::Interaction {
        match self {
            State::Loaded(Err(error)) => {
                let base = initial.as_widget().mouse_interaction(
                    &tree[0],
                    layout.children().nth(0).unwrap(),
                    cursor,
                    viewport,
                    renderer,
                );

                if tree.get(1).is_none() {
                    return mouse::Interaction::None;
                }

                let overlay = error.element.as_widget().mouse_interaction(
                    &tree[1],
                    layout.children().nth(1).unwrap(),
                    cursor,
                    viewport,
                    renderer,
                );

                if cursor.is_over(layout.children().nth(1).unwrap().bounds()) {
                    overlay
                } else {
                    base
                }
            }
            State::Loading { .. } => initial
                .as_widget()
                .mouse_interaction(&tree[0], layout, cursor, viewport, renderer),
            State::Loaded(Ok(inner)) => {
                inner.mouse_interaction(&tree[0], layout, cursor, viewport, renderer)
            }
        }
    }

    pub fn draw(
        &self,
        initial: &Element<'_, Message>,
        tree: &Vec<Tree>,
        renderer: &mut iced_widget::Renderer,
        theme: &iced_widget::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        match self {
            State::Loaded(Err(error)) => {
                initial.as_widget().draw(
                    &tree[0],
                    renderer,
                    theme,
                    style,
                    layout.children().nth(0).unwrap(),
                    cursor,
                    viewport,
                );

                error.element.as_widget().draw(
                    &tree[1],
                    renderer,
                    theme,
                    style,
                    layout.children().nth(1).unwrap(),
                    cursor,
                    viewport,
                )
            }
            State::Loading { .. } => initial
                .as_widget()
                .draw(&tree[0], renderer, theme, style, layout, cursor, viewport),
            State::Loaded(Ok(inner)) => {
                inner.draw(&tree[0], renderer, theme, style, layout, cursor, viewport)
            }
        }
    }

    pub fn overlay<'b>(
        &'b mut self,
        initial: &'b mut Element<'_, Message>,
        tree: &'b mut Vec<Tree>,
        layout: Layout<'b>,
        renderer: &iced_widget::Renderer,
        viewport: &iced_core::Rectangle,
        translation: iced_core::Vector,
    ) -> Option<iced_core::overlay::Element<'b, Message, iced_widget::Theme, iced_widget::Renderer>>
    {
        match self {
            State::Loaded(Err(error)) => {
                if tree.get(1).is_none() {
                    return None;
                }

                error.element.as_widget_mut().overlay(
                    &mut tree[1],
                    layout.children().nth(1).unwrap(),
                    renderer,
                    viewport,
                    translation,
                )
            }
            State::Loading { .. } => initial.as_widget_mut().overlay(
                &mut tree[0],
                layout,
                renderer,
                viewport,
                translation,
            ),
            State::Loaded(Ok(inner)) => {
                inner.overlay(&mut tree[0], layout, renderer, viewport, translation)
            }
        }
    }
}

fn failed<'a, Message: 'a>(error: &crate::Error) -> Element<'a, Message> {
    use iced_core::alignment::{Horizontal, Vertical};
    use iced_core::border;
    use iced_core::text::Renderer;
    use iced_widget::{column, container, float, row, rule, scrollable, text, vertical_rule};

    let rule = vertical_rule(10).style(|theme: &iced_core::Theme| rule::Style {
        color: theme.extended_palette().danger.weak.color,
        width: 10,
        radius: border::left(10),
        fill_mode: rule::FillMode::Full,
    });

    let title = text("Failed")
        .align_y(Vertical::Bottom)
        .size(18)
        .style(|theme: &iced_core::Theme| text::Style {
            color: Some(theme.extended_palette().background.strong.text),
        })
        .font(iced_core::Font {
            weight: iced_core::font::Weight::Bold,
            ..iced_core::Font::DEFAULT
        });

    let description = text("The `thawing` runtime has reached an irrecoverable state").size(14);

    let source = scrollable(
        container(
            text(error.to_string())
                .font(iced_widget::Renderer::MONOSPACE_FONT)
                .size(12),
        )
        .padding(8)
        .style(|theme: &iced_core::Theme| container::Style {
            border: border::rounded(10),
            ..container::dark(theme)
        }),
    );

    let content = column![column![title, description].padding(4).spacing(2), source]
        .padding(8)
        .spacing(4);

    let error = container(row![rule, content].height(iced_core::Length::Shrink))
        .center(iced_core::Length::Shrink)
        .style(|theme: &iced_core::Theme| container::Style {
            background: Some(iced_core::Background::Color(
                theme.extended_palette().danger.strong.color,
            )),
            border: border::rounded(10),
            ..Default::default()
        });

    float(error)
        .translate(|bounds, viewport| {
            let position =
                viewport
                    .shrink(8)
                    .anchor(bounds.size(), Horizontal::Center, Vertical::Bottom);
            bounds.offset(&Rectangle::new(position, bounds.size()))
        })
        .into()
}
