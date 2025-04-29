pub mod error;
mod guest;
mod runtime;
mod task;
mod widget;

pub use error::Error;
pub use iced_core::Element;
pub use serde;
pub use task::{reload, watcher};
pub use thawing_macro::data;
pub use widget::Thawing;

#[macro_export]
macro_rules! view {
    ($widget:expr) => {
        $crate::Thawing::from_view($crate::Element::from($widget), file!())
    };
}
