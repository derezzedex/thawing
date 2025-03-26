use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

use iced::futures;
use iced::futures::channel::mpsc::channel;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::Task;
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use wasmtime::component::{Component, Linker, Resource, ResourceAny, ResourceTable};

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

pub fn watch(path: impl AsRef<Path>) -> Task<Message> {
    Task::stream(watch_file(path.as_ref()))
}

#[derive(Debug, Clone)]
pub enum Message {
    Stateless(u32),
    Stateful(u32, Bytes),
    Thawing(Duration),
}

pub(crate) struct State {
    path: PathBuf,
    store: Rc<RefCell<wasmtime::Store<InternalState>>>,
    bindings: Rc<RefCell<Thawing>>,
    table: Rc<RefCell<ResourceAny>>,

    engine: wasmtime::Engine,
    linker: Linker<InternalState>,
}

impl State {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let engine = wasmtime::Engine::default();
        let component = Component::from_file(&engine, &path).unwrap();

        let mut linker = Linker::new(&engine);
        Thawing::add_to_linker(&mut linker, |state| state).unwrap();

        let mut store = wasmtime::Store::new(&engine, InternalState::default());
        let bindings = Thawing::instantiate(&mut store, &component, &linker).unwrap();

        let table = bindings
            .thawing_core_guest()
            .table()
            .call_constructor(&mut store)
            .unwrap();

        Self {
            path,
            store: Rc::new(RefCell::new(store)),
            bindings: Rc::new(RefCell::new(bindings)),
            table: Rc::new(RefCell::new(table)),
            engine,
            linker,
        }
    }

    pub fn thaw(&mut self) {
        let component = Component::from_file(&self.engine, &self.path).unwrap();

        let mut store = self.store.borrow_mut();
        let mut bindings = self.bindings.borrow_mut();
        *store = wasmtime::Store::new(&self.engine, InternalState::default());
        *bindings = Thawing::instantiate(&mut *store, &component, &self.linker).unwrap();

        let mut table = self.table.borrow_mut();

        *table = bindings
            .thawing_core_guest()
            .table()
            .call_constructor(&mut *store)
            .unwrap();
    }

    pub fn call(&mut self, closure: u32) -> Vec<u8> {
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

    pub fn call_with(&mut self, closure: u32, state: Bytes) -> Vec<u8> {
        self.bindings
            .borrow_mut()
            .thawing_core_guest()
            .table()
            .call_call_with(
                &mut *self.store.borrow_mut(),
                *self.table.borrow(),
                Resource::new_own(closure),
                &state,
            )
            .unwrap()
    }

    pub fn view(&self, state: Vec<u8>) -> iced::Element<'static, Message> {
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
            .call_constructor(&mut *store, &state)
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
pub type Element = iced::Element<'static, Message>;

#[derive(Default)]
pub(crate) struct InternalState {
    pub(crate) table: ResourceTable,
    pub(crate) element: Table<Element>,
}

impl InternalState {
    pub fn push<W>(&mut self, widget: W) -> Resource<Empty>
    where
        W: Into<Element>,
    {
        let res = self.table.push(()).unwrap();
        self.element.insert(res.rep(), widget.into());
        res
    }

    pub fn get<R>(&mut self, element: &Resource<R>) -> Element
    where
        R: 'static,
    {
        self.element.remove(&element.rep()).unwrap()
    }

    pub fn get_widget<W, R>(&mut self, element: &Resource<R>) -> W
    where
        R: 'static,
        W: iced::advanced::Widget<Message, iced::Theme, iced::Renderer>,
    {
        *self.get(element).downcast::<W>()
    }

    pub fn insert<E, R>(&mut self, resource: Resource<R>, widget: E) -> Resource<R>
    where
        R: 'static,
        E: Into<Element>,
    {
        self.element.insert(resource.rep(), widget.into());
        Resource::new_own(resource.rep())
    }
}

impl core::widget::Host for InternalState {}
impl core::types::Host for InternalState {}

impl core::types::HostElement for InternalState {
    fn drop(&mut self, element: Resource<core::types::Element>) -> wasmtime::Result<()> {
        self.element.remove(&element.rep());
        Ok(())
    }
}

impl core::types::HostClosure for InternalState {
    fn new(&mut self) -> wasmtime::component::Resource<core::widget::Closure> {
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

fn watch_file(path: &Path) -> impl Stream<Item = Message> {
    let path = path.canonicalize().expect("failed to canonicalize path");

    iced::stream::channel(10, |mut output| async move {
        let (mut tx, mut rx) = channel(1);

        let mut debouncer = new_debouncer(Duration::from_millis(500), move |res| {
            futures::executor::block_on(async {
                tx.send(res).await.expect("Failed to send debounce event");
            })
        })
        .expect("Failed to create file watcher");

        debouncer
            .watcher()
            .watch(&path, RecursiveMode::NonRecursive)
            .expect("Failed to watch path");

        tracing::info!("Watching {path:?}");

        loop {
            for _ in rx
                .next()
                .await
                .map(Result::ok)
                .flatten()
                .into_iter()
                .flat_map(|events| {
                    events
                        .into_iter()
                        .filter(|event| event.kind == DebouncedEventKind::Any)
                })
                .collect::<Vec<_>>()
            {
                tracing::info!("Building component...");
                let timer = Instant::now();
                Command::new("cargo")
                    .args(["component", "build", "--target", "wasm32-unknown-unknown"])
                    .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/example"))
                    .stdin(Stdio::null())
                    .output()
                    .expect("Failed to build component");
                tracing::info!("Component built in {:?}", timer.elapsed());

                output
                    .send(Message::Thawing(timer.elapsed()))
                    .await
                    .expect("Failed to send message");
            }
        }
    })
}
