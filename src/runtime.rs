use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use iced_core::Element;
use iced_core::{text, widget};
use tempfile::TempDir;
use wasmtime::component::{Component, Linker, Resource, ResourceAny};
use wasmtime::{Engine, Store};

use crate::guest;

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

pub(crate) struct Runtime<'a, Theme, Renderer> {
    engine: Engine,
    linker: Linker<guest::State<'a, Theme, Renderer>>,
    state: State<'a, Theme, Renderer>,
    binary_path: PathBuf,
    _temp_dir: TempDir,
}

impl<'a, Theme, Renderer> Runtime<'a, Theme, Renderer> {
    pub fn reload(&mut self) {
        self.state
            .reload(&self.engine, &self.linker, &self.binary_path);
        self.state.fill_store();
    }
}

impl<'a, Theme, Renderer> Runtime<'a, Theme, Renderer>
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
        let binary_path = manifest
            .join("target")
            .join("wasm32-unknown-unknown")
            .join("debug")
            .join("component.wasm");

        let engine = Engine::default();
        let mut linker = Linker::new(&engine);
        Thawing::add_to_linker(&mut linker, |state| state).unwrap();

        let mut state = State::new(&engine, &linker, &binary_path);
        state.fill_store();

        Self {
            engine,
            linker,
            state,
            binary_path,
            _temp_dir: temp_dir,
        }
    }

    pub fn call<Message: serde::de::DeserializeOwned>(
        &self,
        closure: u32,
        data: impl Into<Option<Bytes>>,
    ) -> Message {
        self.state.call(closure, data)
    }

    pub fn view(&self, bytes: &Vec<u8>) -> Element<'a, guest::Message, Theme, Renderer> {
        self.state.view(bytes)
    }

    pub(crate) fn state(&self) -> State<'a, Theme, Renderer> {
        self.state.clone()
    }
}

pub(crate) struct State<'a, Theme, Renderer> {
    pub(crate) store: Rc<RefCell<Store<guest::State<'a, Theme, Renderer>>>>,
    pub(crate) bindings: Rc<RefCell<Thawing>>,
    pub(crate) table: Rc<RefCell<ResourceAny>>,
}

impl<'a, Theme, Renderer> State<'a, Theme, Renderer> {
    fn new(
        engine: &Engine,
        linker: &Linker<guest::State<'a, Theme, Renderer>>,
        binary_path: impl AsRef<Path>,
    ) -> Self {
        let component = Component::from_file(&engine, binary_path).unwrap();

        let mut store = Store::new(&engine, guest::State::new());
        let bindings = Thawing::instantiate(&mut store, &component, linker).unwrap();

        let table = bindings
            .thawing_core_guest()
            .table()
            .call_constructor(&mut store)
            .unwrap();

        let store = Rc::new(RefCell::new(store));
        let bindings = Rc::new(RefCell::new(bindings));
        let table = Rc::new(RefCell::new(table));

        Self {
            store,
            bindings,
            table,
        }
    }

    pub(crate) fn call<Message: serde::de::DeserializeOwned>(
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

    fn reload(
        &mut self,
        engine: &Engine,
        linker: &Linker<guest::State<'a, Theme, Renderer>>,
        binary_path: impl AsRef<Path>,
    ) {
        let component = Component::from_file(&engine, binary_path).unwrap();
        let mut store = self.store.borrow_mut();
        let mut bindings = self.bindings.borrow_mut();
        *store = Store::new(&engine, guest::State::new());
        *bindings = Thawing::instantiate(&mut *store, &component, &linker).unwrap();

        let mut table = self.table.borrow_mut();

        *table = bindings
            .thawing_core_guest()
            .table()
            .call_constructor(&mut *store)
            .unwrap();
    }

    fn fill_store(&mut self) {
        self.store.borrow_mut().data_mut().runtime = Some(self.clone());
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
    fn view(&self, bytes: &Vec<u8>) -> Element<'a, guest::Message, Theme, Renderer> {
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

impl<'a, Theme, Renderer> Clone for State<'a, Theme, Renderer> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            bindings: self.bindings.clone(),
            table: self.table.clone(),
        }
    }
}
