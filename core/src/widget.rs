use crate::bindings::exports::thawing::core::runtime;
use crate::core::host::Message;
use crate::core::types::{Color, Element, Horizontal, Length, Padding, Pixels};
use crate::core::widget;
use crate::runtime::{Closure, TABLE};

pub fn button(content: impl Into<Element>) -> Button {
    Button::new(content)
}

pub struct Button {
    raw: widget::Button,
}

impl Button {
    pub fn new(content: impl Into<Element>) -> Self {
        Self {
            raw: widget::Button::new(content.into()),
        }
    }

    pub fn on_press(mut self, message: Message) -> Self {
        self.raw = self.raw.on_press(message);
        self
    }

    pub fn on_press_with(mut self, f: impl Fn() -> Message + Send + 'static) -> Self {
        let closure = runtime::Closure::new();
        TABLE
            .lock()
            .unwrap()
            .insert(closure.id(), Closure::stateless(f));
        self.raw = self.raw.on_press_with(closure);
        self
    }
}

pub fn checkbox(label: impl Into<String>, is_checked: bool) -> Checkbox {
    Checkbox::new(label, is_checked)
}

pub struct Checkbox {
    raw: widget::Checkbox,
}

impl Checkbox {
    pub fn new(label: impl Into<String>, is_checked: bool) -> Self {
        Self {
            raw: widget::Checkbox::new(&label.into(), is_checked),
        }
    }

    pub fn on_toggle(mut self, f: impl Fn(bool) -> Message + Send + 'static) -> Self {
        let guest_fn = runtime::Closure::new();
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
        $crate::widget::Column::with_children([$($crate::core::types::Element::from($x)),+])
    );
}

pub use column;

pub struct Column {
    raw: widget::Column,
}

impl Column {
    pub fn new() -> Self {
        Self {
            raw: widget::Column::new(),
        }
    }

    pub fn from_vec(children: Vec<Element>) -> Self {
        Self {
            raw: widget::Column::from_vec(children),
        }
    }

    pub fn with_children(children: impl IntoIterator<Item = Element>) -> Self {
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

    pub fn push(mut self, content: impl Into<Element>) -> Self {
        self.raw = self.raw.push(content.into());
        self
    }

    pub fn extend(self, children: impl IntoIterator<Item = Element>) -> Self {
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

pub struct Text {
    raw: widget::Text,
}

impl Text {
    pub fn new(fragment: impl ToString) -> Self {
        Self {
            raw: widget::Text::new(&fragment.to_string()),
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

impl From<&str> for Element {
    fn from(content: &str) -> Element {
        Text::new(content).into()
    }
}

impl<T: ToString> From<T> for Text {
    fn from(content: T) -> Text {
        Text::new(content)
    }
}

impl From<Text> for Element {
    fn from(text: Text) -> Self {
        text.raw.into_element()
    }
}

impl From<Button> for Element {
    fn from(button: Button) -> Self {
        button.raw.into_element()
    }
}

impl From<Checkbox> for Element {
    fn from(checkbox: Checkbox) -> Self {
        checkbox.raw.into_element()
    }
}

impl From<Column> for Element {
    fn from(column: Column) -> Self {
        column.raw.into_element()
    }
}
