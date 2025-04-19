use crate::Element;
use crate::core::types::{Color, Horizontal, Length, Padding, Pixels};
use crate::core::widget;
use crate::guest;
use crate::runtime::{Closure, TABLE};

use std::marker::PhantomData;

pub fn button<Message: serde::Serialize + Clone + Send + 'static, Theme>(
    content: impl Into<Element<Theme>>,
) -> Button<Message, Theme> {
    Button::new(content)
}

pub struct Button<Message, Theme = crate::Theme> {
    raw: widget::Button,
    _message: PhantomData<Message>,
    _theme: PhantomData<Theme>,
}

impl<Message: serde::Serialize + Clone + Send + 'static, Theme> Button<Message, Theme> {
    pub fn new(content: impl Into<Element<Theme>>) -> Self {
        Self {
            raw: widget::Button::new(content.into().into_raw()),
            _message: PhantomData,
            _theme: PhantomData,
        }
    }

    pub fn on_press(self, message: Message) -> Self {
        self.on_press_with(move || message.clone())
    }

    pub fn on_press_with(mut self, f: impl Fn() -> Message + Send + 'static) -> Self {
        let closure = guest::Closure::new();
        TABLE
            .lock()
            .unwrap()
            .insert(closure.id(), Closure::stateless(f));
        self.raw = self.raw.on_press_with(closure);
        self
    }
}

pub fn checkbox<Message: serde::Serialize + 'static, Theme>(
    label: impl Into<String>,
    is_checked: bool,
) -> Checkbox<Message, Theme> {
    Checkbox::new(label, is_checked)
}

pub struct Checkbox<Message, Theme = crate::Theme> {
    raw: widget::Checkbox,
    _message: PhantomData<Message>,
    _theme: PhantomData<Theme>,
}

impl<Message: serde::Serialize + 'static, Theme> Checkbox<Message, Theme> {
    pub fn new(label: impl Into<String>, is_checked: bool) -> Self {
        Self {
            raw: widget::Checkbox::new(&label.into(), is_checked),
            _message: PhantomData,
            _theme: PhantomData,
        }
    }

    pub fn on_toggle(mut self, f: impl Fn(bool) -> Message + Send + 'static) -> Self {
        let guest_fn = guest::Closure::new();
        TABLE
            .lock()
            .unwrap()
            .insert(guest_fn.id(), Closure::stateful(f));
        self.raw = self.raw.on_toggle(guest_fn);
        self
    }
}

#[macro_export]
macro_rules! column {
    () => (
        $crate::widget::Column::new()
    );
    ($($x:expr),+ $(,)?) => (
        $crate::widget::Column::with_children([$($crate::Element::from($x)),+])
    );
}

pub use column;

pub struct Column<Theme = crate::Theme> {
    raw: widget::Column,
    _theme: PhantomData<Theme>,
}

impl<Theme> Column<Theme> {
    pub fn new() -> Self {
        Self {
            raw: widget::Column::new(),
            _theme: PhantomData,
        }
    }

    pub fn from_vec(children: Vec<Element<Theme>>) -> Self {
        Self {
            raw: widget::Column::from_vec(children.into_iter().map(Element::into_raw).collect()),
            _theme: PhantomData,
        }
    }

    pub fn with_children(children: impl IntoIterator<Item = Element<Theme>>) -> Self {
        let iterator = children.into_iter();

        Self::with_capacity(iterator.size_hint().0).extend(iterator)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_vec(Vec::with_capacity(capacity))
    }

    pub fn spacing(mut self, amount: impl Into<Pixels>) -> Self {
        self.raw = self.raw.spacing(amount.into());
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.raw = self.raw.padding(padding.into());
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.raw = self.raw.width(width.into());
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.raw = self.raw.height(height.into());
        self
    }

    pub fn max_width(mut self, width: impl Into<Pixels>) -> Self {
        self.raw = self.raw.max_width(width.into());
        self
    }

    pub fn align_x(mut self, align: impl Into<Horizontal>) -> Self {
        self.raw = self.raw.align_x(align.into());
        self
    }

    pub fn clip(mut self, clip: bool) -> Self {
        self.raw = self.raw.clip(clip);
        self
    }

    pub fn push(mut self, content: impl Into<Element<Theme>>) -> Self {
        self.raw = self.raw.push(content.into().into_raw());
        self
    }

    pub fn extend(self, children: impl IntoIterator<Item = Element<Theme>>) -> Self {
        children.into_iter().fold(self, Self::push)
    }
}

#[macro_export]
macro_rules! text {
    ($($arg:tt)*) => {
        $crate::widget::Text::new(format!($($arg)*))
    };
}

pub use text;

pub struct Text<Theme = crate::Theme> {
    raw: widget::Text,
    _theme: PhantomData<Theme>,
}

impl<Theme> Text<Theme> {
    pub fn new(fragment: impl ToString) -> Self {
        Self {
            raw: widget::Text::new(&fragment.to_string()),
            _theme: PhantomData,
        }
    }

    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.raw = self.raw.size(size.into());
        self
    }

    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.raw = self.raw.color(color.into());
        self
    }
}

impl<Theme> Text<Theme>
where
    Theme: serde::de::DeserializeOwned + 'static,
{
    pub fn style(mut self, f: impl Fn(&Theme) -> Style + Send + 'static) -> Self {
        let closure = guest::Closure::new();
        TABLE
            .lock()
            .unwrap()
            .insert(closure.id(), Closure::stateful_ref(f));
        self.raw = self.raw.style(closure);
        self
    }
}

#[derive(serde::Serialize)]
pub struct Style {
    pub color: Option<Color>,
}

impl<Theme> From<&str> for Element<Theme> {
    fn from(content: &str) -> Element<Theme> {
        Text::new(content).into()
    }
}

impl<T: ToString, Theme> From<T> for Text<Theme> {
    fn from(content: T) -> Text<Theme> {
        Text::new(content)
    }
}

impl<Theme> From<Text<Theme>> for Element<Theme> {
    fn from(text: Text<Theme>) -> Self {
        Element::from(text.raw.into_element())
    }
}

impl<Message, Theme> From<Button<Message, Theme>> for Element<Theme> {
    fn from(button: Button<Message, Theme>) -> Self {
        Element::from(button.raw.into_element())
    }
}

impl<Message, Theme> From<Checkbox<Message, Theme>> for Element<Theme> {
    fn from(checkbox: Checkbox<Message, Theme>) -> Self {
        Element::from(checkbox.raw.into_element())
    }
}

impl<Theme> From<Column<Theme>> for Element<Theme> {
    fn from(column: Column<Theme>) -> Self {
        Element::from(column.raw.into_element())
    }
}
