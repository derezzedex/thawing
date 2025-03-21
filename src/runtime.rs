use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use iced::futures;
use iced::futures::channel::mpsc::{channel, Receiver};
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::Task;
use wasmtime::component::{Component, Linker, Resource, ResourceAny, ResourceTable};

pub use core::host;
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
    Direct(host::Message),
    Stateless(u32),
    Stateful(u32, Bytes),
    Thawing(Duration),
}

pub(crate) struct State {
    path: PathBuf,
    store: Rc<RefCell<wasmtime::Store<InternalState>>>,
    bindings: Rc<RefCell<Thawing>>,
    app: Rc<RefCell<ResourceAny>>,
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
            .thawing_core_runtime()
            .table()
            .call_constructor(&mut store)
            .unwrap();

        let app = bindings
            .thawing_core_guest()
            .app()
            .call_constructor(&mut store)
            .unwrap();

        Self {
            path,
            store: Rc::new(RefCell::new(store)),
            bindings: Rc::new(RefCell::new(bindings)),
            app: Rc::new(RefCell::new(app)),
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
        let mut app = self.app.borrow_mut();

        *table = bindings
            .thawing_core_runtime()
            .table()
            .call_constructor(&mut *store)
            .unwrap();

        *app = bindings
            .thawing_core_guest()
            .app()
            .call_constructor(&mut *store)
            .unwrap();
    }

    pub fn call(&mut self, closure: u32) -> host::Message {
        self.bindings
            .borrow()
            .thawing_core_runtime()
            .table()
            .call_call(
                &mut *self.store.borrow_mut(),
                *self.table.borrow(),
                Resource::new_own(closure),
            )
            .unwrap()
    }

    pub fn call_with(&mut self, closure: u32, state: Bytes) -> host::Message {
        self.bindings
            .borrow_mut()
            .thawing_core_runtime()
            .table()
            .call_call_with(
                &mut *self.store.borrow_mut(),
                *self.table.borrow(),
                Resource::new_own(closure),
                &state,
            )
            .unwrap()
    }

    pub fn view(&self, state: impl Into<host::State>) -> iced::Element<'static, Message> {
        let mut store = self.store.borrow_mut();
        let mut table = self.table.borrow_mut();
        table.resource_drop(&mut *store).unwrap();

        store.data_mut().element.clear();

        *table = self
            .bindings
            .borrow()
            .thawing_core_runtime()
            .table()
            .call_constructor(&mut *store)
            .unwrap();

        let view = self
            .bindings
            .borrow()
            .thawing_core_guest()
            .app()
            .call_view(&mut *store, *self.app.borrow(), state.into())
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

impl core::host::Host for InternalState {}
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

    use notify_debouncer_mini::notify::{self, RecommendedWatcher, RecursiveMode};
    use notify_debouncer_mini::{new_debouncer, DebouncedEvent, DebouncedEventKind, Debouncer};

    pub fn async_debouncer() -> notify::Result<(
        Debouncer<RecommendedWatcher>,
        Receiver<notify::Result<Vec<DebouncedEvent>>>,
    )> {
        let (mut tx, rx) = channel(1);

        let watcher = new_debouncer(std::time::Duration::from_millis(500), move |res| {
            futures::executor::block_on(async {
                tx.send(res).await.unwrap();
            })
        })?;

        Ok((watcher, rx))
    }

    iced::stream::channel(10, |mut output| async move {
        let (mut debouncer, mut rx) = async_debouncer().expect("Failed to create watcher");
        debouncer
            .watcher()
            .watch(path.as_ref(), RecursiveMode::NonRecursive)
            .unwrap_or_else(|_| panic!("Failed to watch path {path:?}"));
        println!("Watching {path:?}");

        loop {
            while let Some(res) = rx.next().await {
                match res {
                    Ok(events) => {
                        for event in events {
                            if event.kind == DebouncedEventKind::Any {
                                println!("Building component...");
                                let timer = std::time::Instant::now();
                                let _build = std::process::Command::new("cargo")
                                    .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/example"))
                                    .args([
                                        "component",
                                        "build",
                                        "--target",
                                        "wasm32-unknown-unknown",
                                    ])
                                    .stdin(std::process::Stdio::null())
                                    .output()
                                    .expect("Failed to build component");
                                println!("Component built in {:?}", timer.elapsed());

                                output.send(Message::Thawing(timer.elapsed())).await.expect(
                                "Couldn't send a WatchedFileChanged Message for some odd reason",
                            );
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    })
}
