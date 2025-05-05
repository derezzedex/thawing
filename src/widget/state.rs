use std::path::PathBuf;
use std::sync::Arc;

use crate::Element;
use crate::{guest, runtime};

pub struct Error<Message> {
    _raw: crate::Error,
    element: Element<'static, Message>,
}

impl<Message> Error<Message> {
    pub fn new(error: crate::Error) -> Self {
        let element = failed(&error);

        Self {
            _raw: error,
            element,
        }
    }
}

impl<Message> AsRef<Element<'static, Message>> for Error<Message> {
    fn as_ref(&self) -> &Element<'static, Message> {
        &self.element
    }
}

impl<Message> AsMut<Element<'static, Message>> for Error<Message> {
    fn as_mut(&mut self) -> &mut Element<'static, Message> {
        &mut self.element
    }
}

pub(crate) enum View<Message> {
    None,
    Failed(Error<Message>),
    Built {
        runtime: runtime::Runtime<'static>,
        element: Element<'static, guest::Message>,
        mapper: Box<dyn Fn(guest::Message) -> Message>,
        error: Option<Error<Message>>,
    },
}

pub(crate) struct Inner<Message> {
    pub(crate) view: View<Message>,
    pub(crate) invalidated: bool,
    pub(crate) bytes: Arc<Vec<u8>>,
    pub(crate) caller: PathBuf,
}

impl<Message> Inner<Message> {
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

fn failed<'a, Message>(text: impl ToString) -> Element<'a, Message> {
    iced_widget::text(text.to_string()).size(12).into()
}
