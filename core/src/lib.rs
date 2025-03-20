#[allow(warnings)]
pub mod bindings;

#[macro_export]
macro_rules! thaw {
    ($ty: ident) => {
        use $crate::{bindings, runtime};
        use bindings::exports::thawing::core::guest;
        use bindings::exports::thawing::core::runtime as wasm;

        #[doc(hidden)]
        struct _Component;

        #[doc(hidden)]
        struct _Table;

        impl wasm::Guest for _Component {
            type Table = _Table;
        }

        impl wasm::GuestTable for _Table {
            fn new() -> Self {
                runtime::TABLE.lock().unwrap().clear();

                _Table
            }

            fn call(&self, c: wasm::Closure) -> bindings::thawing::core::host::Message {
                let table = runtime::TABLE.lock().unwrap();
                let closure = table.get(&c.id()).unwrap();
                closure.call().downcast()
            }

            fn call_with(&self, c: wasm::Closure, state: wasm::Bytes) -> bindings::thawing::core::host::Message {
                let table = runtime::TABLE.lock().unwrap();
                let closure = table.get(&c.id()).unwrap();
                closure.call_with(runtime::AnyBox::new(state)).downcast()
            }
        }

        impl guest::Guest for _Component {
            type App = $ty;
        }

        bindings::export!(_Component with_types_in bindings);
    }
}

pub mod runtime;

#[path = "widget.rs"]
mod widgets;

pub mod widget {
    pub use crate::widgets::*;
    pub fn text(content: impl ToString) -> Text {
        Text::new(content)
    }
}

pub use bindings::thawing::core;
pub use core::host;
pub use core::types::{
    Color, Element,
    Horizontal::{self, *},
    Length::{self, *},
    Padding, Pixels,
};
pub use runtime::Application;

#[macro_export]
macro_rules! color {
    ($r:expr, $g:expr, $b:expr) => {
        $crate::Color {
            r: $r as f32 / 255.0,
            g: $g as f32 / 255.0,
            b: $b as f32 / 255.0,
            a: 1.0,
        }
    };
    ($r:expr, $g:expr, $b:expr, $a:expr) => {{
        $crate::Color {
            r: $r as f32 / 255.0,
            g: $g as f32 / 255.0,
            b: $b as f32 / 255.0,
            a: $a,
        }
    }};
    ($hex:expr) => {{
        $crate::color!($hex, 1.0)
    }};
    ($hex:expr, $a:expr) => {{
        let hex = $hex as u32;

        debug_assert!(hex <= 0xffffff, "color! value must not exceed 0xffffff");

        let r = (hex & 0xff0000) >> 16;
        let g = (hex & 0xff00) >> 8;
        let b = (hex & 0xff);

        $crate::color!(r as u8, g as u8, b as u8, $a)
    }};
}

impl From<f32> for Pixels {
    fn from(amount: f32) -> Self {
        Self { amount }
    }
}

impl From<u16> for Pixels {
    fn from(amount: u16) -> Self {
        let amount = f32::from(amount);
        Self { amount }
    }
}

impl From<Pixels> for f32 {
    fn from(pixels: Pixels) -> Self {
        pixels.amount
    }
}

impl From<u16> for Padding {
    fn from(p: u16) -> Self {
        Padding {
            top: f32::from(p),
            right: f32::from(p),
            bottom: f32::from(p),
            left: f32::from(p),
        }
    }
}

impl From<[u16; 2]> for Padding {
    fn from(p: [u16; 2]) -> Self {
        Padding {
            top: f32::from(p[0]),
            right: f32::from(p[1]),
            bottom: f32::from(p[0]),
            left: f32::from(p[1]),
        }
    }
}

impl From<f32> for Padding {
    fn from(p: f32) -> Self {
        Padding {
            top: p,
            right: p,
            bottom: p,
            left: p,
        }
    }
}

impl From<[f32; 2]> for Padding {
    fn from(p: [f32; 2]) -> Self {
        Padding {
            top: p[0],
            right: p[1],
            bottom: p[0],
            left: p[1],
        }
    }
}
