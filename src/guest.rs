mod types;
mod widget;

use std::collections::HashMap;

use iced_core::{Element, Widget, element};
use iced_widget::text;
use wasmtime::component::{Resource, ResourceTable};

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
pub(crate) struct State<'a, Theme, Renderer> {
    pub(crate) table: ResourceTable,
    pub(crate) element: Table<Element<'a, Message, Theme, Renderer>>,
    pub(crate) runtime: Option<runtime::State<'a, Theme, Renderer>>,
}

impl<'a, Theme, Renderer> State<'a, Theme, Renderer> {
    pub(crate) fn new() -> Self {
        Self {
            table: ResourceTable::new(),
            element: Table::new(),
            runtime: None,
        }
    }
}

impl<'a, Theme, Renderer> State<'a, Theme, Renderer>
where
    Renderer: iced_core::Renderer,
{
    pub fn push<W>(&mut self, widget: W) -> Resource<Empty>
    where
        W: Into<Element<'a, Message, Theme, Renderer>>,
    {
        let res = self.table.push(()).unwrap();
        self.element.insert(res.rep(), widget.into());
        res
    }

    pub fn get<R>(&mut self, element: &Resource<R>) -> Element<'a, Message, Theme, Renderer>
    where
        R: 'static,
    {
        self.element.remove(&element.rep()).unwrap()
    }

    pub fn get_widget<W, R>(&mut self, element: &Resource<R>) -> W
    where
        R: 'static,
        W: Widget<Message, Theme, Renderer>,
    {
        let widget = element::into_raw(self.get(element));

        unsafe { *Box::from_raw(Box::into_raw(widget) as *mut W) }
    }

    pub fn insert<E, R>(&mut self, resource: Resource<R>, widget: E) -> Resource<R>
    where
        R: 'static,
        E: Into<Element<'a, Message, Theme, Renderer>>,
    {
        self.element.insert(resource.rep(), widget.into());
        Resource::new_own(resource.rep())
    }
}

// TODO(derezzedex): fix this
// this forces users to have implemented `Catalog` in their `Theme` for every widget available
impl<'a, Theme, Renderer> core::widget::Host for State<'a, Theme, Renderer>
where
    Renderer: 'a + iced_core::Renderer + iced_core::text::Renderer,
    Theme: 'a
        + serde::Serialize
        + iced_widget::checkbox::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    <Theme as text::Catalog>::Class<'a>: From<text::StyleFn<'a, Theme>>,
{
}

impl<'a, Theme, Renderer> core::types::Host for State<'a, Theme, Renderer> {}

impl<'a, Theme, Renderer> core::types::HostElement for State<'a, Theme, Renderer> {
    fn drop(&mut self, element: Resource<core::types::Element>) -> wasmtime::Result<()> {
        self.element.remove(&element.rep());
        Ok(())
    }
}

impl<'a, Theme, Renderer> core::types::HostClosure for State<'a, Theme, Renderer> {
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
