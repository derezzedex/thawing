use std::collections::HashMap;
use std::path::Path;

use iced::futures;
use iced::futures::channel::mpsc::{channel, Receiver};
use iced::futures::{SinkExt, Stream, StreamExt};
use wasmtime::component::{Component, Linker, Resource, ResourceTable};

pub use iced_thawing::host::Message;
use iced_thawing::types::{Color, Pixels};

pub type IcedColumn = iced::widget::Column<'static, Message>;
pub type IcedButton = iced::widget::Button<'static, Message>;
pub type IcedText = iced::widget::Text<'static>;
pub type IcedElement = iced::Element<'static, Message>;

pub type Empty = ();

wasmtime::component::bindgen!({
    world: "thawing",
    with: {
        "component:iced-thawing/widget/column": Empty,
        "component:iced-thawing/widget/text": Empty,
        "component:iced-thawing/widget/button": Empty,
        "component:iced-thawing/types/element": Empty,
    }
});

pub fn watch(path: impl AsRef<Path>) -> impl Stream<Item = crate::Message> {
    use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

    pub fn async_watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)>
    {
        let (mut tx, rx) = channel(1);

        let watcher = RecommendedWatcher::new(
            move |res| {
                futures::executor::block_on(async {
                    tx.send(res).await.unwrap();
                })
            },
            Config::default(),
        )?;
        Ok((watcher, rx))
    }

    iced::stream::channel(10, |mut output| async move {
        let (mut watcher, mut rx) = async_watcher().expect("Failed to create watcher");
        watcher
            .watch(path.as_ref(), RecursiveMode::NonRecursive)
            .unwrap_or_else(|_| panic!("Failed to watch path {:?}", path.as_ref()));

        loop {
            while let Some(res) = rx.next().await {
                match res {
                    Ok(event) => {
                        if event.kind.is_create() {
                            std::thread::sleep(std::time::Duration::from_millis(500));
                            println!("{event:?}");
                            output.send(crate::Message::FileChanged).await.expect(
                                "Couldn't send a WatchedFileChanged Message for some odd reason",
                            );
                        }
                    }
                    Err(e) => println!("watch error: {:?}", e),
                }
            }
        }
    })
}

pub(crate) struct State {
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

        Self {
            engine,
            component,
            linker,
        }
    }

    pub fn view(&self, value: i64) -> iced::Element<'static, Message> {
        let mut store = wasmtime::Store::new(&self.engine, InternalState::default());
        let bindings = Thawing::instantiate(&mut store, &self.component, &self.linker).unwrap();

        let app = bindings
            .component_iced_thawing_guest()
            .app()
            .call_constructor(&mut store, value)
            .unwrap();
        let view = bindings
            .component_iced_thawing_guest()
            .app()
            .call_view(&mut store, app)
            .unwrap();

        let el = store.data_mut().element.remove(&view.rep()).unwrap();
        el
    }
}

type Table<T> = HashMap<u32, T>;

#[derive(Default)]
struct InternalState {
    indices: ResourceTable,
    element: Table<IcedElement>,
}

use component::iced_thawing;

impl iced_thawing::host::Host for InternalState {}

impl iced_thawing::types::Host for InternalState {}
impl iced_thawing::types::HostElement for InternalState {
    fn drop(&mut self, element: Resource<iced_thawing::types::Element>) -> wasmtime::Result<()> {
        self.element.remove(&element.rep());
        Ok(())
    }
}

impl iced_thawing::widget::Host for InternalState {}

impl iced_thawing::widget::HostButton for InternalState {
    fn new(
        &mut self,
        content: Resource<iced_thawing::widget::Element>,
    ) -> Resource<iced_thawing::widget::Button> {
        let content = self
            .element
            .remove(&content.rep())
            .expect("button content not found");
        let button = IcedButton::new(content);

        let i = self.indices.push(()).unwrap();
        self.element.insert(i.rep(), button.into());
        i
    }

    fn on_press(
        &mut self,
        button: Resource<iced_thawing::widget::Button>,
        message: iced_thawing::widget::Message,
    ) -> Resource<iced_thawing::widget::Button> {
        let mut widget = self
            .element
            .remove(&button.rep())
            .unwrap()
            .downcast::<IcedButton>();
        *widget = widget.on_press(message);
        self.element.insert(button.rep(), (*widget).into());

        Resource::new_own(button.rep())
    }

    fn into_element(
        &mut self,
        button: Resource<iced_thawing::widget::Button>,
    ) -> Resource<iced_thawing::widget::Element> {
        Resource::new_own(button.rep())
    }

    fn drop(&mut self, button: Resource<iced_thawing::widget::Button>) -> wasmtime::Result<()> {
        let _ = self.indices.delete(button);
        Ok(())
    }
}

impl iced_thawing::widget::HostColumn for InternalState {
    fn new(&mut self) -> Resource<iced_thawing::widget::Column> {
        let i = self.indices.push(()).unwrap();
        self.element.insert(i.rep(), IcedColumn::new().into());
        i
    }

    fn push(
        &mut self,
        column: Resource<iced_thawing::widget::Column>,
        child: Resource<iced_thawing::widget::Element>,
    ) -> Resource<iced_thawing::widget::Column> {
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
        column: Resource<iced_thawing::widget::Column>,
    ) -> Resource<iced_thawing::widget::Element> {
        Resource::new_own(column.rep())
    }

    fn drop(&mut self, column: Resource<iced_thawing::widget::Column>) -> wasmtime::Result<()> {
        let _ = self.indices.delete(column);
        Ok(())
    }
}

impl iced_thawing::widget::HostText for InternalState {
    fn new(&mut self, fragment: String) -> Resource<iced_thawing::widget::Text> {
        let i = self.indices.push(()).unwrap();
        self.element.insert(i.rep(), IcedText::new(fragment).into());
        i
    }

    fn size(
        &mut self,
        text: Resource<iced_thawing::widget::Text>,
        size: Pixels,
    ) -> Resource<iced_thawing::widget::Text> {
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
        text: Resource<iced_thawing::widget::Text>,
        color: Color,
    ) -> Resource<iced_thawing::widget::Text> {
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
        text: Resource<iced_thawing::widget::Text>,
    ) -> Resource<iced_thawing::widget::Element> {
        Resource::new_own(text.rep())
    }

    fn drop(&mut self, text: Resource<iced_thawing::widget::Text>) -> wasmtime::Result<()> {
        let _ = self.indices.delete(text);
        Ok(())
    }
}
