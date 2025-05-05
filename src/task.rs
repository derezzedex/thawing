mod component;
mod executor;
mod file;

use iced_widget::runtime::Task;

use crate::widget;

pub fn thaw<Message: serde::de::DeserializeOwned + Send + 'static>(
    id: impl Into<widget::Id>,
) -> Task<()> {
    let id = id.into();

    component::fetch_caller_path::<Message>(&id).then(move |target| {
        let id = id.clone();
        let caller = target.clone();

        file::init_directory()
            .then(move |manifest| file::parse_and_write(&target, manifest))
            .then(component::build)
            .then(move |manifest| component::create_runtime::<Message>(&id, manifest))
            .then(move |(id, manifest)| {
                let caller = caller.clone();

                Task::stream(file::watch(caller.clone())).then(move |_| {
                    let id = id.clone();

                    file::parse_and_write(&caller, manifest.clone())
                        .then(component::build)
                        .then(move |manifest| {
                            component::reload::<Message>(id.clone(), manifest.err())
                        })
                })
            })
    })
}
