mod guest;
mod runtime;
mod task;
mod widget;

use std::io;
use std::sync::Arc;
use tokio::task::JoinError;

pub use iced_core::Element;
pub use serde;
pub use task::{reload, watcher};
pub use thawing_macro::data;
pub use widget::Thawing;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("io failed with: {0}")]
    IO(Arc<io::Error>),
    #[error("task failed with: {0}")]
    Task(Arc<JoinError>),
    #[error("cargo component build failed:\n{0}")]
    CargoComponent(String),
    #[error("parsing failed with: {0}")]
    Parsing(ParserError),
    #[error("failed to recv on a channel")]
    RecvFailed,
    #[error("failed to send on a channel")]
    SendFailed,
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::IO(Arc::new(error))
    }
}

impl From<JoinError> for Error {
    fn from(error: JoinError) -> Self {
        Self::Task(Arc::new(error))
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ParserError {
    #[error("failed to parse file: {0}")]
    Syn(Arc<syn::Error>),
    #[error("failed to find macro: {0}")]
    Macro(MacroError),
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum MacroError {
    #[error("`thawing::view` macro is missing")]
    ViewMacroMissing,
    #[error("`[thawing::data(state)]` attribute macro is missing")]
    StateAttributeMissing,
    #[error("`[thawing::data(message)]` attribute macro is missing")]
    MessageAttributeMissing,
}

impl From<MacroError> for Error {
    fn from(error: MacroError) -> Self {
        Self::Parsing(ParserError::Macro(error))
    }
}

impl From<ParserError> for Error {
    fn from(error: ParserError) -> Self {
        Self::Parsing(error)
    }
}

impl From<syn::Error> for Error {
    fn from(error: syn::Error) -> Self {
        Self::Parsing(ParserError::Syn(Arc::new(error)))
    }
}

#[macro_export]
macro_rules! view {
    ($widget:expr) => {
        $crate::Thawing::from_view($crate::Element::from($widget), file!())
    };
}
