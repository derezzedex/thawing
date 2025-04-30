use std::path::PathBuf;
use std::sync::Arc;

use crate::Element;
use crate::{guest, runtime};

pub(crate) enum View {
    None,
    Failed(crate::Error),
    Built {
        runtime: runtime::Runtime<'static>,
        element: Element<'static, guest::Message>,
        error: Option<crate::Error>,
    },
}

pub(crate) struct Inner {
    pub(crate) view: View,
    pub(crate) invalidated: bool,
    pub(crate) bytes: Arc<Vec<u8>>,
    pub(crate) caller: PathBuf,
}

impl Inner {
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
