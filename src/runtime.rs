use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use iced::futures;
use iced::futures::channel::mpsc::{channel, Receiver};
use iced::futures::{SinkExt, Stream, StreamExt};
use wasmtime::component::{Component, Linker, Resource, ResourceAny, ResourceTable};

pub use core::host;
use core::types::{Color, Pixels};
use thawing::core;

pub type IcedColumn = iced::widget::Column<'static, Message>;
pub type IcedButton = iced::widget::Button<'static, Message>;
pub type IcedText = iced::widget::Text<'static>;
pub type IcedCheckbox = iced::widget::Checkbox<'static, Message>;
pub type IcedElement = iced::Element<'static, Message>;

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

pub fn watch(path: impl AsRef<Path>) -> impl Stream<Item = Message> {
    let path = path
        .as_ref()
        .canonicalize()
        .expect("failed to canonicalize path");

    use notify_debouncer_mini::notify::{self, RecommendedWatcher, RecursiveMode};
    use notify_debouncer_mini::{new_debouncer, DebouncedEvent, DebouncedEventKind, Debouncer};

    pub fn async_debouncer() -> notify::Result<(
        Debouncer<RecommendedWatcher>,
        Receiver<notify::Result<Vec<DebouncedEvent>>>,
    )> {
        let (mut tx, rx) = channel(1);

        let watcher = new_debouncer(std::time::Duration::from_secs(1), move |res| {
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
                            println!("{event:?}");
                            if event.kind == DebouncedEventKind::Any {
                                let _build = std::process::Command::new("cargo")
                                    .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/component"))
                                    .args([
                                        "component",
                                        "build",
                                        "--target",
                                        "wasm32-unknown-unknown",
                                    ])
                                    .stdin(std::process::Stdio::null())
                                    .output()
                                    .expect("Failed to build component");
                                println!("component built!");

                                output.send(Message::Thaw).await.expect(
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

#[derive(Debug, Clone)]
pub enum Message {
    Direct(host::Message),
    Stateless(u32),
    Stateful(u32, Bytes),
    Thaw,
}

pub(crate) struct State {
    store: Rc<RefCell<wasmtime::Store<InternalState>>>,
    bindings: Rc<RefCell<Thawing>>,
    app: Rc<RefCell<ResourceAny>>,
    table: Rc<RefCell<ResourceAny>>,

    engine: wasmtime::Engine,
    component: Component,
    linker: Linker<InternalState>,
}

impl State {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let engine = wasmtime::Engine::default();
        let component = Component::from_file(&engine, path).unwrap();

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
            store: Rc::new(RefCell::new(store)),
            bindings: Rc::new(RefCell::new(bindings)),
            app: Rc::new(RefCell::new(app)),
            table: Rc::new(RefCell::new(table)),
            engine,
            component,
            linker,
        }
    }

    pub fn call(&mut self, closure: u32) -> host::Message {
        self.bindings
            .borrow_mut()
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

    pub fn view(&self, state: host::State) -> iced::Element<'static, Message> {
        let mut store = self.store.borrow_mut();
        self.table.borrow_mut().resource_drop(&mut *store).unwrap();
        self.app.borrow_mut().resource_drop(&mut *store).unwrap();
        *store = wasmtime::Store::new(&self.engine, InternalState::default());
        let mut bindings = self.bindings.borrow_mut();
        *bindings = Thawing::instantiate(&mut *store, &self.component, &self.linker).unwrap();

        let mut table = self.table.borrow_mut();
        *table = bindings
            .thawing_core_runtime()
            .table()
            .call_constructor(&mut *store)
            .unwrap();

        let mut app = self.app.borrow_mut();
        *app = bindings
            .thawing_core_guest()
            .app()
            .call_constructor(&mut *store)
            .unwrap();

        let view = bindings
            .thawing_core_guest()
            .app()
            .call_view(&mut *store, *app, state)
            .unwrap();

        let el = store.data_mut().element.remove(&view.rep()).unwrap();

        el
    }
}

type Table<T> = HashMap<u32, T>;

#[derive(Default)]
struct InternalState {
    table: ResourceTable,
    element: Table<IcedElement>,
}

impl core::host::Host for InternalState {}

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

impl core::widget::Host for InternalState {}

impl core::widget::HostCheckbox for InternalState {
    fn new(&mut self, label: String, is_checked: bool) -> Resource<core::widget::Checkbox> {
        let checkbox = IcedCheckbox::new(label, is_checked);

        let i = self.table.push(()).unwrap();
        self.element.insert(i.rep(), checkbox.into());
        i
    }

    fn on_toggle(
        &mut self,
        checkbox: Resource<core::widget::Checkbox>,
        closure: Resource<core::types::Closure>,
    ) -> Resource<core::widget::Checkbox> {
        let mut widget = self
            .element
            .remove(&checkbox.rep())
            .unwrap()
            .downcast::<IcedCheckbox>();
        *widget = widget.on_toggle(move |is_checked| {
            Message::Stateful(closure.rep(), bincode::serialize(&is_checked).unwrap())
        });
        self.element.insert(checkbox.rep(), (*widget).into());

        Resource::new_own(checkbox.rep())
    }

    fn into_element(
        &mut self,
        button: Resource<core::widget::Checkbox>,
    ) -> Resource<core::widget::Element> {
        Resource::new_own(button.rep())
    }

    fn drop(&mut self, _button: Resource<core::widget::Checkbox>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl core::widget::HostButton for InternalState {
    fn new(&mut self, content: Resource<core::widget::Element>) -> Resource<core::widget::Button> {
        let content = self
            .element
            .remove(&content.rep())
            .expect("button content not found");
        let button = IcedButton::new(content);

        let i = self.table.push(()).unwrap();
        self.element.insert(i.rep(), button.into());
        i
    }

    fn on_press(
        &mut self,
        button: Resource<core::widget::Button>,
        message: host::Message,
    ) -> Resource<core::widget::Button> {
        let mut widget = self
            .element
            .remove(&button.rep())
            .unwrap()
            .downcast::<IcedButton>();
        *widget = widget.on_press(Message::Direct(message));
        self.element.insert(button.rep(), (*widget).into());

        Resource::new_own(button.rep())
    }

    fn on_press_with(
        &mut self,
        button: Resource<core::widget::Button>,
        closure: Resource<core::types::Closure>,
    ) -> Resource<core::widget::Button> {
        let mut widget = self
            .element
            .remove(&button.rep())
            .unwrap()
            .downcast::<IcedButton>();
        *widget = widget.on_press_with(move || Message::Stateless(closure.rep()));
        self.element.insert(button.rep(), (*widget).into());

        Resource::new_own(button.rep())
    }

    fn into_element(
        &mut self,
        button: Resource<core::widget::Button>,
    ) -> Resource<core::widget::Element> {
        Resource::new_own(button.rep())
    }

    fn drop(&mut self, _button: Resource<core::widget::Button>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl core::widget::HostColumn for InternalState {
    fn new(&mut self) -> Resource<core::widget::Column> {
        let i = self.table.push(()).unwrap();
        self.element.insert(i.rep(), IcedColumn::new().into());
        i
    }

    fn push(
        &mut self,
        column: Resource<core::widget::Column>,
        child: Resource<core::widget::Element>,
    ) -> Resource<core::widget::Column> {
        let content = self
            .element
            .remove(&child.rep())
            .expect("button content not found");
        let mut widget = self
            .element
            .remove(&column.rep())
            .unwrap()
            .downcast::<IcedColumn>();
        *widget = widget.push(content);
        self.element.insert(column.rep(), (*widget).into());

        Resource::new_own(column.rep())
    }

    fn into_element(
        &mut self,
        column: Resource<core::widget::Column>,
    ) -> Resource<core::widget::Element> {
        Resource::new_own(column.rep())
    }

    fn drop(&mut self, _column: Resource<core::widget::Column>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl core::widget::HostText for InternalState {
    fn new(&mut self, fragment: String) -> Resource<core::widget::Text> {
        let i = self.table.push(()).unwrap();
        self.element.insert(i.rep(), IcedText::new(fragment).into());
        i
    }

    fn size(
        &mut self,
        text: Resource<core::widget::Text>,
        size: Pixels,
    ) -> Resource<core::widget::Text> {
        let mut widget = self
            .element
            .remove(&text.rep())
            .unwrap()
            .downcast::<IcedText>();
        *widget = widget.size(size);
        self.element.insert(text.rep(), (*widget).into());

        Resource::new_own(text.rep())
    }

    fn color(
        &mut self,
        text: Resource<core::widget::Text>,
        color: Color,
    ) -> Resource<core::widget::Text> {
        let mut widget = self
            .element
            .remove(&text.rep())
            .unwrap()
            .downcast::<IcedText>();
        *widget = widget.color(iced::Color {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        });
        self.element.insert(text.rep(), (*widget).into());

        Resource::new_own(text.rep())
    }

    fn into_element(
        &mut self,
        text: Resource<core::widget::Text>,
    ) -> Resource<core::widget::Element> {
        Resource::new_own(text.rep())
    }

    fn drop(&mut self, _text: Resource<core::widget::Text>) -> wasmtime::Result<()> {
        Ok(())
    }
}
