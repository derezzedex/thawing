pub mod error;
mod guest;
mod runtime;
mod task;
mod widget;

pub use error::Error;
pub use serde;
pub use task::thaw;
pub use thawing_macro::data;
pub use widget::Thawing;

pub type Element<'a, Message> =
    iced_core::Element<'a, Message, iced_widget::Theme, iced_widget::Renderer>;

#[macro_export]
macro_rules! view {
    ($widget:expr) => {
        $crate::Thawing::from_view($crate::Element::from($widget), file!())
    };
}
