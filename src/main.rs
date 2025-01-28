use wasmtime::component::{Component, Linker, Resource, ResourceTable};

use iced_thawing::host::Message;
use iced_thawing::types::{Color, Pixels};

pub type IcedText = iced::widget::Text<'static>;
pub type IcedElement = iced::Element<'static, Message>;

wasmtime::component::bindgen!({
    world: "thawing",
    // with: {
    //     "component:iced-thawing/widget/text": IcedText,
    //     "component:iced-thawing/types/element": IcedElement,
    // }
});

fn main() -> wasmtime::Result<()> {
    let engine = wasmtime::Engine::default();
    let component = Component::from_file(
        &engine,
        "./component/target/wasm32-unknown-unknown/release/component.wasm",
    )?;

    let mut linker = Linker::new(&engine);
    Thawing::add_to_linker(&mut linker, |state| state)?;

    let mut store = wasmtime::Store::new(&engine, State::default());
    let _bindings = Thawing::instantiate(&mut store, &component, &linker)?;

    Ok(())
}

#[derive(Default)]
struct State {}

use component::iced_thawing;

impl iced_thawing::host::Host for State {}

impl iced_thawing::types::Host for State {}
impl iced_thawing::types::HostElement for State {
    fn drop(
        &mut self,
        rep: wasmtime::component::Resource<iced_thawing::types::Element>,
    ) -> wasmtime::Result<()> {
        todo!()
    }
}

impl iced_thawing::widget::Host for State {}

impl iced_thawing::widget::HostButton for State {
    fn new(
        &mut self,
        content: wasmtime::component::Resource<iced_thawing::widget::Element>,
    ) -> wasmtime::component::Resource<iced_thawing::widget::Button> {
        todo!()
    }

    fn on_press(
        &mut self,
        self_: wasmtime::component::Resource<iced_thawing::widget::Button>,
        message: iced_thawing::widget::Message,
    ) -> wasmtime::component::Resource<iced_thawing::widget::Button> {
        todo!()
    }

    fn into_element(
        &mut self,
        self_: wasmtime::component::Resource<iced_thawing::widget::Button>,
    ) -> wasmtime::component::Resource<iced_thawing::widget::Element> {
        todo!()
    }

    fn drop(
        &mut self,
        rep: wasmtime::component::Resource<iced_thawing::widget::Button>,
    ) -> wasmtime::Result<()> {
        todo!()
    }
}

impl iced_thawing::widget::HostColumn for State {
    fn new(&mut self) -> wasmtime::component::Resource<iced_thawing::widget::Column> {
        todo!()
    }

    fn push(
        &mut self,
        self_: wasmtime::component::Resource<iced_thawing::widget::Column>,
        child: wasmtime::component::Resource<iced_thawing::widget::Element>,
    ) -> wasmtime::component::Resource<iced_thawing::widget::Column> {
        todo!()
    }

    fn into_element(
        &mut self,
        self_: wasmtime::component::Resource<iced_thawing::widget::Column>,
    ) -> wasmtime::component::Resource<iced_thawing::widget::Element> {
        todo!()
    }

    fn drop(
        &mut self,
        rep: wasmtime::component::Resource<iced_thawing::widget::Column>,
    ) -> wasmtime::Result<()> {
        todo!()
    }
}

impl iced_thawing::widget::HostFragment for State {
    fn drop(
        &mut self,
        rep: wasmtime::component::Resource<iced_thawing::widget::Fragment>,
    ) -> wasmtime::Result<()> {
        todo!()
    }
}

impl iced_thawing::widget::HostText for State {
    fn new(
        &mut self,
        fragment: wasmtime::component::Resource<iced_thawing::widget::Fragment>,
    ) -> wasmtime::component::Resource<iced_thawing::widget::Text> {
        todo!()
    }

    fn size(
        &mut self,
        self_: wasmtime::component::Resource<iced_thawing::widget::Text>,
        size: Pixels,
    ) -> wasmtime::component::Resource<iced_thawing::widget::Text> {
        todo!()
    }

    fn color(
        &mut self,
        self_: wasmtime::component::Resource<iced_thawing::widget::Text>,
        color: Color,
    ) -> wasmtime::component::Resource<iced_thawing::widget::Text> {
        todo!()
    }

    fn into_element(
        &mut self,
        self_: wasmtime::component::Resource<iced_thawing::widget::Text>,
    ) -> wasmtime::component::Resource<iced_thawing::widget::Element> {
        todo!()
    }

    fn drop(
        &mut self,
        rep: wasmtime::component::Resource<iced_thawing::widget::Text>,
    ) -> wasmtime::Result<()> {
        todo!()
    }
}
