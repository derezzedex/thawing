use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use iced_core::widget;
use iced_core::{Element, Widget, element, text};
use wasmtime::component::{Component, Linker, Resource, ResourceAny, ResourceTable};
use wasmtime::{Engine, Store};

use thawing::core;

pub type Empty = ();
pub type Bytes = Vec<u8>;

wasmtime::component::bindgen!({
    world: "thawing",
    with: {
        "thawing:core/widget/column": Empty,
        "thawing:core/widget/text": Empty,
        "thawing:core/widget/button": Empty,
        "thawing:core/widget/checkbox": Empty,
        "thawing:core/types/closure": Empty,
        "thawing:core/types/element": Empty,
    },
});

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

enum BinaryPath {
    Temporary {
        _temp_dir: tempfile::TempDir,
        path: PathBuf,
    },
    UserProvided(PathBuf),
}

impl BinaryPath {
    fn temporary(_temp_dir: tempfile::TempDir, path: PathBuf) -> Self {
        Self::Temporary { _temp_dir, path }
    }
}

impl AsRef<Path> for BinaryPath {
    fn as_ref(&self) -> &Path {
        match self {
            BinaryPath::Temporary { path, .. } => path,
            BinaryPath::UserProvided(path) => path,
        }
    }
}

pub(crate) struct State<'a, Theme, Renderer> {
    wasm: BinaryPath,
    store: Rc<RefCell<Store<Guest<'a, Theme, Renderer>>>>,
    bindings: Rc<RefCell<Thawing>>,
    table: Rc<RefCell<ResourceAny>>,

    engine: Engine,
    linker: Linker<Guest<'a, Theme, Renderer>>,
}

impl<'a, Theme, Renderer> State<'a, Theme, Renderer> {
    pub fn reload(&mut self) {
        let component = Component::from_file(&self.engine, &self.wasm).unwrap();
        let mut store = self.store.borrow_mut();
        let mut bindings = self.bindings.borrow_mut();
        *store = Store::new(&self.engine, Guest::new());
        *bindings = Thawing::instantiate(&mut *store, &component, &self.linker).unwrap();

        let mut table = self.table.borrow_mut();

        *table = bindings
            .thawing_core_guest()
            .table()
            .call_constructor(&mut *store)
            .unwrap();

        store.data_mut().fill(
            self.store.clone(),
            self.bindings.clone(),
            self.table.clone(),
        );
    }
}

impl<'a, Theme, Renderer> State<'a, Theme, Renderer>
where
    Renderer: 'a + iced_core::Renderer + text::Renderer,
    Theme: 'a
        + serde::Serialize
        + iced_widget::checkbox::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    <Theme as widget::text::Catalog>::Class<'a>: From<widget::text::StyleFn<'a, Theme>>,
{
    pub fn from_view(temp_dir: tempfile::TempDir) -> Self {
        let manifest = temp_dir.path().join("component");
        let wasm = manifest
            .join("target")
            .join("wasm32-unknown-unknown")
            .join("debug")
            .join("component.wasm");

        let engine = Engine::default();
        let component = Component::from_file(&engine, &wasm).unwrap();

        let mut linker = Linker::new(&engine);
        Thawing::add_to_linker(&mut linker, |state| state).unwrap();

        let mut store = Store::new(&engine, Guest::new());
        let bindings = Thawing::instantiate(&mut store, &component, &linker).unwrap();

        let table = bindings
            .thawing_core_guest()
            .table()
            .call_constructor(&mut store)
            .unwrap();

        let store = Rc::new(RefCell::new(store));
        let bindings = Rc::new(RefCell::new(bindings));
        let table = Rc::new(RefCell::new(table));

        store
            .borrow_mut()
            .data_mut()
            .fill(store.clone(), bindings.clone(), table.clone());

        Self {
            wasm: BinaryPath::temporary(temp_dir, wasm),
            store,
            bindings,
            table,
            engine,
            linker,
        }
    }

    pub fn from_component(path: impl AsRef<Path>) -> Self {
        let manifest = path.as_ref().to_path_buf();
        let wasm = std::fs::read_dir(
            manifest
                .join("target")
                .join("wasm32-unknown-unknown")
                .join("debug"),
        )
        .unwrap()
        .filter_map(Result::ok)
        .filter(|dir| {
            dir.file_type()
                .ok()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
        })
        .find(|dir| {
            dir.path()
                .extension()
                .map(|ext| ext == "wasm")
                .unwrap_or(false)
        })
        .unwrap()
        .path();

        let engine = Engine::default();
        let component = Component::from_file(&engine, &wasm).unwrap();

        let mut linker = Linker::new(&engine);
        Thawing::add_to_linker(&mut linker, |state| state).unwrap();

        let mut store = Store::new(&engine, Guest::new());
        let bindings = Thawing::instantiate(&mut store, &component, &linker).unwrap();

        let table = bindings
            .thawing_core_guest()
            .table()
            .call_constructor(&mut store)
            .unwrap();

        let store = Rc::new(RefCell::new(store));
        let bindings = Rc::new(RefCell::new(bindings));
        let table = Rc::new(RefCell::new(table));

        store
            .borrow_mut()
            .data_mut()
            .fill(store.clone(), bindings.clone(), table.clone());

        Self {
            wasm: BinaryPath::UserProvided(wasm),
            store,
            bindings,
            table,
            engine,
            linker,
        }
    }

    pub fn call<Message: serde::de::DeserializeOwned>(
        &self,
        closure: u32,
        data: impl Into<Option<Bytes>>,
    ) -> Message {
        let bytes = match data.into() {
            Some(bytes) => self.call_stateful(closure, bytes),
            None => self.call_stateless(closure),
        };

        bincode::deserialize(&bytes).unwrap()
    }

    fn call_stateless(&self, closure: u32) -> Vec<u8> {
        self.bindings
            .borrow()
            .thawing_core_guest()
            .table()
            .call_call(
                &mut *self.store.borrow_mut(),
                *self.table.borrow(),
                Resource::new_own(closure),
            )
            .unwrap()
    }

    fn call_stateful(&self, closure: u32, bytes: Bytes) -> Vec<u8> {
        self.bindings
            .borrow_mut()
            .thawing_core_guest()
            .table()
            .call_call_with(
                &mut *self.store.borrow_mut(),
                *self.table.borrow(),
                Resource::new_own(closure),
                &bytes,
            )
            .unwrap()
    }

    pub fn view(&self, bytes: &Vec<u8>) -> Element<'a, Message, Theme, Renderer> {
        let mut store = self.store.borrow_mut();
        let mut table = self.table.borrow_mut();
        table.resource_drop(&mut *store).unwrap();

        store.data_mut().element.clear();

        *table = self
            .bindings
            .borrow()
            .thawing_core_guest()
            .table()
            .call_constructor(&mut *store)
            .unwrap();

        let app = self
            .bindings
            .borrow()
            .thawing_core_guest()
            .app()
            .call_constructor(&mut *store, bytes)
            .unwrap();

        let view = self
            .bindings
            .borrow()
            .thawing_core_guest()
            .app()
            .call_view(&mut *store, app)
            .unwrap();

        store.data_mut().element.remove(&view.rep()).unwrap()
    }
}

type Table<T> = HashMap<u32, T>;
type StoreCell<'a, Theme, Renderer> = Rc<RefCell<Store<Guest<'a, Theme, Renderer>>>>;
type BindingsCell = Rc<RefCell<Thawing>>;
type ResourceCell = Rc<RefCell<ResourceAny>>;

#[derive(Default)]
pub(crate) struct Guest<'a, Theme, Renderer> {
    pub(crate) table: ResourceTable,
    pub(crate) element: Table<Element<'a, Message, Theme, Renderer>>,
    pub(crate) store: Option<StoreCell<'a, Theme, Renderer>>,
    pub(crate) bindings: Option<BindingsCell>,
    pub(crate) resource: Option<ResourceCell>,
}

impl<'a, Theme, Renderer> Guest<'a, Theme, Renderer> {
    fn new() -> Self {
        Self {
            table: ResourceTable::new(),
            element: Table::new(),
            store: None,
            bindings: None,
            resource: None,
        }
    }

    fn fill(
        &mut self,
        store: StoreCell<'a, Theme, Renderer>,
        bindings: BindingsCell,
        table: ResourceCell,
    ) {
        self.store = Some(store);
        self.bindings = Some(bindings);
        self.resource = Some(table);
    }
}

impl<'a, Theme, Renderer> Guest<'a, Theme, Renderer>
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
impl<'a, Theme, Renderer> core::widget::Host for Guest<'a, Theme, Renderer>
where
    Renderer: 'a + iced_core::Renderer + text::Renderer,
    Theme: 'a
        + serde::Serialize
        + iced_widget::checkbox::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    <Theme as widget::text::Catalog>::Class<'a>: From<widget::text::StyleFn<'a, Theme>>,
{
}

impl<'a, Theme, Renderer> core::types::Host for Guest<'a, Theme, Renderer> {}

impl<'a, Theme, Renderer> core::types::HostElement for Guest<'a, Theme, Renderer> {
    fn drop(&mut self, element: Resource<core::types::Element>) -> wasmtime::Result<()> {
        self.element.remove(&element.rep());
        Ok(())
    }
}

impl<'a, Theme, Renderer> core::types::HostClosure for Guest<'a, Theme, Renderer> {
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
