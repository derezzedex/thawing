use std::path::PathBuf;
use std::sync::Arc;

use iced_core::Element;
use iced_core::text;

use crate::{guest, runtime};

pub(crate) enum View<Theme, Renderer> {
    None,
    Failed(crate::Error),
    Built {
        runtime: runtime::Runtime<'static, Theme, Renderer>,
        element: Element<'static, guest::Message, Theme, Renderer>,
        error: Option<crate::Error>,
    },
}

pub(crate) struct Inner<Theme, Renderer> {
    pub(crate) view: View<Theme, Renderer>,
    pub(crate) invalidated: bool,
    pub(crate) bytes: Arc<Vec<u8>>,
    pub(crate) caller: PathBuf,
}

impl<Theme, Renderer> Inner<Theme, Renderer>
where
    Renderer: 'static + iced_core::Renderer + text::Renderer,
    Theme: 'static
        + serde::Serialize
        + iced_widget::checkbox::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text_input::Catalog
        + iced_widget::text::Catalog,
    <Theme as iced_widget::text::Catalog>::Class<'static>:
        From<iced_widget::text::StyleFn<'static, Theme>>,
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

    pub(crate) fn diff(&mut self, other: &Arc<Vec<u8>>) {
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
}
