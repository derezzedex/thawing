use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use wasmtime::component::{Component, Linker, Resource, ResourceAny, ResourceTable};
use wasmtime::{Engine, Store};

use crate::Element;
use crate::guest;

pub type Empty = ();
pub type Bytes = Vec<u8>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("root element not found")]
    RootElementNotFound,
}

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

pub(crate) struct Runtime<'a> {
    engine: Engine,
    linker: Arc<Linker<guest::State<'a>>>,
    state: State<'a>,
    binary_path: PathBuf,
}

impl<'a> Runtime<'a> {
    pub fn reload(&mut self) -> Result<(), crate::Error> {
        self.state
            .reload(&self.engine, &self.linker, &self.binary_path)?;
        self.state.fill_store();

        Ok(())
    }
}

impl<'a> Runtime<'a> {
    pub fn new(manifest: &PathBuf) -> Result<Self, crate::Error> {
        let binary_path = manifest
            .join("target")
            .join("wasm32-unknown-unknown")
            .join("debug")
            .join("component.wasm");

        let engine = Engine::default();
        let mut linker = Linker::new(&engine);
        Thawing::add_to_linker(&mut linker, |state| state)?;

        let mut state = State::new(&engine, &linker, &binary_path)?;
        state.fill_store();

        let linker = Arc::new(linker);

        Ok(Self {
            engine,
            linker,
            state,
            binary_path,
        })
    }

    pub fn view(&self, bytes: &Vec<u8>) -> Result<Element<'a, guest::Message>, crate::Error> {
        self.state.view(bytes)
    }

    pub(crate) fn state(&self) -> State<'a> {
        self.state.clone()
    }
}

pub(crate) struct State<'a> {
    pub(crate) store: Arc<Mutex<Store<guest::State<'a>>>>,
    pub(crate) bindings: Arc<Thawing>,
    pub(crate) table: Arc<ResourceAny>,
}

impl<'a> State<'a> {
    fn new(
        engine: &Engine,
        linker: &Linker<guest::State<'a>>,
        binary_path: impl AsRef<Path>,
    ) -> Result<Self, crate::Error> {
        let component = Component::from_file(&engine, binary_path)?;

        let mut store = Store::new(&engine, guest::State::new());
        let bindings = Thawing::instantiate(&mut store, &component, linker)?;

        let table = bindings
            .thawing_core_guest()
            .table()
            .call_constructor(&mut store)?;

        let store = Arc::new(Mutex::new(store));
        let bindings = Arc::new(bindings);
        let table = Arc::new(table);

        Ok(Self {
            store,
            bindings,
            table,
        })
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
            .thawing_core_guest()
            .table()
            .call_call(
                &mut *self.store.lock().unwrap(),
                *self.table,
                Resource::new_own(closure),
            )
            .unwrap()
    }

    fn call_stateful(&self, closure: u32, bytes: Bytes) -> Vec<u8> {
        self.bindings
            .thawing_core_guest()
            .table()
            .call_call_with(
                &mut *self.store.lock().unwrap(),
                *self.table,
                Resource::new_own(closure),
                &bytes,
            )
            .unwrap()
    }

    fn reload(
        &mut self,
        engine: &Engine,
        linker: &Linker<guest::State<'a>>,
        binary_path: impl AsRef<Path>,
    ) -> Result<(), crate::Error> {
        let component = Component::from_file(&engine, binary_path)?;
        let mut store = self.store.lock().unwrap();
        *store = Store::new(&engine, guest::State::new());
        self.bindings = Arc::new(Thawing::instantiate(&mut *store, &component, &linker)?);

        let table = self
            .bindings
            .thawing_core_guest()
            .table()
            .call_constructor(&mut *store)?;
        self.table = Arc::new(table);

        Ok(())
    }

    fn fill_store(&mut self) {
        self.store.lock().unwrap().data_mut().runtime = Some(self.clone());
    }
}

impl<'a> State<'a> {
    fn view(&self, bytes: &Vec<u8>) -> Result<Element<'a, guest::Message>, crate::Error> {
        let mut store = self.store.lock().unwrap();

        store.data_mut().element.clear();
        store.data_mut().table = ResourceTable::new();

        let app = self
            .bindings
            .thawing_core_guest()
            .app()
            .call_constructor(&mut *store, bytes)?;

        let view = self
            .bindings
            .thawing_core_guest()
            .app()
            .call_view(&mut *store, app)?;

        let element = store
            .data_mut()
            .element
            .remove(&view.rep())
            .ok_or(crate::Error::Runtime(Error::RootElementNotFound))?;

        Ok(element)
    }
}

impl<'a> Clone for State<'a> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            bindings: self.bindings.clone(),
            table: self.table.clone(),
        }
    }
}
