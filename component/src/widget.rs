use crate::bindings::exports::thawing::core::runtime;
use crate::core::host::Message;
use crate::core::types::Element;
use crate::core::widget;
use crate::runtime::{Closure, TABLE};

pub struct Button {
    raw: widget::Button,
}

impl Button {
    pub fn new(content: impl Into<Element>) -> Self {
        Self {
            raw: widget::Button::new(content.into()),
        }
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

pub struct Column {
    raw: widget::Column,
}

impl Column {
    pub fn new() -> Self {
        Self {
            raw: widget::Column::new(),
        }
    }

    pub fn push(mut self, content: impl Into<Element>) -> Self {
        let el = content.into();
        self.raw = self.raw.push(el);
        self
    }
}

pub struct Text {
    raw: widget::Text,
}

impl Text {
    pub fn new(fragment: impl ToString) -> Self {
        Self {
            raw: widget::Text::new(&fragment.to_string()),
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.raw = self.raw.size(size.into());
        self
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
