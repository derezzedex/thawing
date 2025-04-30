mod component;
mod executor;
mod file;

use iced_core::text;
use iced_widget::runtime::Task;

use crate::widget;

pub fn thaw<Theme, Renderer>(id: impl Into<widget::Id>) -> Task<()>
where
    Renderer: 'static + Send + iced_core::Renderer + text::Renderer,
    Theme: 'static
        + Send
        + serde::Serialize
        + iced_widget::checkbox::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    <Theme as iced_widget::text::Catalog>::Class<'static>:
        From<iced_widget::text::StyleFn<'static, Theme>>,
{
    let id = id.into();

    component::fetch_caller_path::<Theme, Renderer>(&id).then(move |target| {
        let id = id.clone();
        let caller = target.clone();

        file::init_directory()
            .then(move |manifest| file::parse_and_write(&target, manifest))
            .then(component::build)
            .then(move |manifest| component::create_runtime::<Theme, Renderer>(&id, manifest))
            .then(move |(id, manifest)| {
                let caller = caller.clone();

                Task::stream(file::watch(caller.clone())).then(move |_| {
                    let id = id.clone();

                    file::parse_and_write(&caller, manifest.clone())
                        .then(component::build)
                        .then(move |manifest| {
                            component::reload::<Theme, Renderer>(id.clone(), manifest.err())
                        })
                })
            })
    })
}
