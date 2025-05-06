mod types;
mod widget;

use std::collections::HashMap;

use iced_core::{Widget, element};
use wasmtime::component::{Resource, ResourceTable};

use crate::Element;
use crate::runtime::thawing::core;
use crate::runtime::{self, Bytes, Empty};

type Table<T> = HashMap<u32, T>;

#[derive(Debug, Clone)]
pub struct Message {
    pub(crate) closure: u32,
    pub(crate) data: Option<Bytes>,
}

impl Message {
    pub fn stateless<U: 'static>(resource: &Resource<U>) -> Self {
        Self {
            closure: resource.rep(),
            data: None,
        }
    }

    pub fn stateful<T: serde::Serialize, U: 'static>(resource: &Resource<U>, value: T) -> Self {
        let bytes = bincode::serialize(&value).unwrap();

        Self {
            closure: resource.rep(),
            data: Some(bytes),
        }
    }
}

#[derive(Default)]
pub(crate) struct State<'a> {
    pub(crate) table: ResourceTable,
    pub(crate) element: Table<Element<'a, Message>>,
    pub(crate) runtime: Option<runtime::State<'a>>,
}

// This should be safe, `wasmtime::Store` seems to require `Send` because of `Preview 3`,
// but that's not used and not even available yet.
unsafe impl<'a> Send for State<'a> {}

impl<'a> State<'a> {
    pub(crate) fn new() -> Self {
        Self {
            table: ResourceTable::new(),
            element: Table::new(),
            runtime: None,
        }
    }
}

impl<'a> State<'a> {
    pub fn push<W>(&mut self, widget: W) -> Resource<Empty>
    where
        W: Into<Element<'a, Message>>,
    {
        let res = self.table.push(()).unwrap();
        self.element.insert(res.rep(), widget.into());
        res
    }

    pub fn get<R>(&mut self, element: &Resource<R>) -> Element<'a, Message>
    where
        R: 'static,
    {
        self.element.remove(&element.rep()).unwrap()
    }

    pub fn get_widget<W, R>(&mut self, element: &Resource<R>) -> W
    where
        R: 'static,
        W: Widget<Message, iced_widget::Theme, iced_widget::Renderer>,
    {
        let widget = element::into_raw(self.get(element));

        unsafe { *Box::from_raw(Box::into_raw(widget) as *mut W) }
    }

    pub fn insert<E, R>(&mut self, resource: Resource<R>, widget: E) -> Resource<R>
    where
        R: 'static,
        E: Into<Element<'a, Message>>,
    {
        self.element.insert(resource.rep(), widget.into());
        Resource::new_own(resource.rep())
    }
}

impl<'a> core::widget::Host for State<'a> {}

impl<'a> core::types::Host for State<'a> {}

impl<'a> core::types::HostElement for State<'a> {
    fn drop(&mut self, element: Resource<core::types::Element>) -> wasmtime::Result<()> {
        self.element.remove(&element.rep());
        Ok(())
    }
}

impl<'a> core::types::HostClosure for State<'a> {
    fn new(&mut self) -> Resource<core::widget::Closure> {
        self.table.push(()).unwrap()
    }

    fn id(&mut self, closure: Resource<core::widget::Closure>) -> u32 {
        closure.rep()
    }

    fn drop(&mut self, closure: Resource<core::widget::Closure>) -> wasmtime::Result<()> {
        let _ = self.table.delete(closure);

        Ok(())
    }
}
