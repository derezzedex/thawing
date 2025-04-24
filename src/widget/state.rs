use std::sync::Arc;

use iced_core::Element;
use iced_core::text;

use crate::widget::Kind;
use crate::{guest, runtime};

pub(crate) enum View<Theme, Renderer> {
    None,
    Built {
        runtime: runtime::Runtime<'static, Theme, Renderer>,
        element: Element<'static, guest::Message, Theme, Renderer>,
    },
}

pub(crate) struct Inner<Theme, Renderer> {
    pub(crate) view: View<Theme, Renderer>,
    pub(crate) invalidated: bool,
    pub(crate) bytes: Arc<Vec<u8>>,
    pub(crate) kind: Kind,
}

impl<Theme, Renderer> Inner<Theme, Renderer>
where
    Renderer: 'static + iced_core::Renderer + text::Renderer,
    Theme: 'static
        + serde::Serialize
        + iced_widget::checkbox::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    <Theme as iced_widget::text::Catalog>::Class<'static>:
        From<iced_widget::text::StyleFn<'static, Theme>>,
{
    pub(crate) fn new(kind: &Kind, bytes: Arc<Vec<u8>>) -> Self {
        let runtime = match kind {
            Kind::ComponentFile(path) => {
                let runtime = runtime::Runtime::from_component(&path);
                let element = runtime.view(&bytes);

                View::Built { runtime, element }
            }
            Kind::ViewMacro(_) => View::None,
        };

        Self {
            view: runtime,
            invalidated: false,
            kind: kind.clone(),
            bytes,
        }
    }

    pub(crate) fn diff(&mut self, other: &Arc<Vec<u8>>) {
        if Arc::ptr_eq(&self.bytes, other) {
            return;
        }

        if let View::Built { runtime, element } = &mut self.view {
            *element = runtime.view(other);
        }

        self.bytes = Arc::clone(other);
    }
}
