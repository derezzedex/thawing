use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

use iced::advanced::widget;
use iced::futures;
use iced::futures::channel::mpsc::channel;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::{Element, Task};
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
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
pub struct Id(pub(crate) widget::Id);

impl Id {
    pub fn new(id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        Self(widget::Id::new(id))
    }

    pub fn unique() -> Self {
        Self(widget::Id::unique())
    }
}

impl From<Id> for widget::Id {
    fn from(id: Id) -> Self {
        id.0
    }
}

impl From<&'static str> for Id {
    fn from(value: &'static str) -> Self {
        Id::new(value)
    }
}

pub fn watch<Message, Theme, Renderer>(
    id: impl Into<Id> + Clone + Send + 'static,
    path: impl AsRef<Path>,
) -> Task<()>
where
    Message: Send + 'static,
    Theme: Send + 'static,
    Renderer: Send + 'static,
{
    Task::stream(watch_file(path.as_ref())).then(move |_| reload::<Theme, Renderer>(id.clone()))
}

// TODO(derezzedex): if `watch` is used instead, a `Widget::update` call
// is needed (e.g. moving the mouse, or focusing the window) to display changes;
// this sends a message to the application, forcing a `Widget::update`
pub fn watch_and_notify<Message, Theme, Renderer>(
    id: impl Into<Id> + Clone + Send + 'static,
    path: impl AsRef<Path>,
    on_reload: Message,
) -> Task<Message>
where
    Message: Clone + Send + 'static,
    Theme: Send + 'static,
    Renderer: Send + 'static,
{
    watch::<Message, Theme, Renderer>(id, path).map(move |_| on_reload.clone())
}

pub fn reload<Theme: Send + 'static, Renderer: Send + 'static>(
    id: impl Into<Id> + Clone + Send + 'static,
) -> Task<()> {
    let id = id.into();

    struct Reload<Theme, Renderer> {
        id: widget::Id,
        theme: PhantomData<Theme>,
        renderer: PhantomData<Renderer>,
    }

    impl<Theme: Send + 'static, Renderer: Send + 'static> widget::Operation
        for Reload<Theme, Renderer>
    {
        fn custom(
            &mut self,
            id: Option<&widget::Id>,
            _bounds: iced::Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            match id {
                Some(id) if id == &self.id => {
                    if let Some(state) = state.downcast_mut::<crate::Inner<Theme, Renderer>>() {
                        state.runtime.reload();
                        state.invalidated = true;
                    }
                }
                _ => {}
            }
        }

        fn container(
            &mut self,
            _id: Option<&widget::Id>,
            _bounds: iced::Rectangle,
            operate_on_children: &mut dyn FnMut(&mut dyn widget::Operation<()>),
        ) {
            operate_on_children(self)
        }

        fn finish(&self) -> widget::operation::Outcome<()> {
            widget::operation::Outcome::Some(())
        }
    }

    widget::operate(Reload {
        id: id.into(),
        theme: PhantomData::<Theme>,
        renderer: PhantomData::<Renderer>,
    })
}

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

pub(crate) struct State<'a, Theme, Renderer> {
    path: PathBuf,
    store: Rc<RefCell<Store<Guest<'a, Theme, Renderer>>>>,
    bindings: Rc<RefCell<Thawing>>,
    table: Rc<RefCell<ResourceAny>>,

    engine: Engine,
    linker: Linker<Guest<'a, Theme, Renderer>>,
}

impl<'a, Theme, Renderer> State<'a, Theme, Renderer> {
    pub fn reload(&mut self) {
        let component = Component::from_file(&self.engine, &self.path).unwrap();

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
    }
}

impl<'a, Theme, Renderer> State<'a, Theme, Renderer>
where
    Renderer: 'a + iced::advanced::Renderer + iced::advanced::text::Renderer,
    Theme: 'a
        + iced::widget::checkbox::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog,
    <Theme as iced::widget::text::Catalog>::Class<'a>: From<iced::widget::text::StyleFn<'a, Theme>>,
{
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let engine = Engine::default();
        let component = Component::from_file(&engine, &path).unwrap();

        let mut linker = Linker::new(&engine);
        Thawing::add_to_linker(&mut linker, |state| state).unwrap();

        let mut store = Store::new(&engine, Guest::new());
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

    pub fn view(&self, bytes: &Vec<u8>) -> iced::Element<'a, Message, Theme, Renderer> {
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

#[derive(Default)]
pub(crate) struct Guest<'a, Theme, Renderer> {
    pub(crate) table: ResourceTable,
    pub(crate) element: Table<Element<'a, Message, Theme, Renderer>>,
}

impl<'a, Theme, Renderer> Guest<'a, Theme, Renderer> {
    fn new() -> Self {
        Self {
            table: ResourceTable::new(),
            element: Table::new(),
        }
    }
}

impl<'a, Theme, Renderer> Guest<'a, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
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
        W: iced::advanced::Widget<Message, Theme, Renderer>,
    {
        *self.get(element).downcast::<W>()
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
    Renderer: 'a + iced::advanced::Renderer + iced::advanced::text::Renderer,
    Theme: 'a
        + iced::widget::checkbox::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog,
    <Theme as iced::widget::text::Catalog>::Class<'a>: From<iced::widget::text::StyleFn<'a, Theme>>,
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

fn watch_file(path: &Path) -> impl Stream<Item = ()> {
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

                output.send(()).await.expect("Failed to send message");
            }
        }
    })
}
