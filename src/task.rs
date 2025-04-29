mod executor;
mod file;

use std::marker::PhantomData;
use std::path::PathBuf;
use std::process::Stdio;

use iced_core::Rectangle;
use iced_core::text;
use iced_core::widget::Operation;
use iced_core::widget::operation;
use iced_widget::runtime::{Task, task};

use crate::runtime;
use crate::widget::{Id, Inner, View};

pub fn watcher<Theme, Renderer>(id: impl Into<Id>) -> Task<()>
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

    fetch_caller_path::<Theme, Renderer>(&id).then(move |target| {
        let id = id.clone();

        file::init_directory().then(move |manifest| {
            let id = id.clone();
            let caller = target.clone();

            file::parse_and_write(&caller, manifest)
                .then(build)
                .then(move |manifest| create_runtime::<Theme, Renderer>(&id, manifest))
                .then(move |(id, manifest)| {
                    let caller = caller.clone();

                    Task::stream(file::watch(caller.clone())).then(move |_| {
                        let id = id.clone();

                        file::parse_and_write(&caller, manifest.clone())
                            .then(build)
                            .then(move |manifest| {
                                reload::<Theme, Renderer>(id.clone(), manifest.err())
                            })
                    })
                })
        })
    })
}

fn fetch_caller_path<Theme: Send + 'static, Renderer: Send + 'static>(id: &Id) -> Task<PathBuf> {
    struct GetPath<Theme, Renderer> {
        id: iced_core::widget::Id,
        path: Option<PathBuf>,
        theme: PhantomData<Theme>,
        renderer: PhantomData<Renderer>,
    }

    impl<Theme: Send + 'static, Renderer: Send + 'static> Operation<PathBuf>
        for GetPath<Theme, Renderer>
    {
        fn custom(
            &mut self,
            id: Option<&iced_core::widget::Id>,
            _bounds: Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            match id {
                Some(id) if id == &self.id => {
                    if let Some(state) = state.downcast_mut::<Inner<Theme, Renderer>>() {
                        self.path = Some(state.caller.clone());
                        return;
                    }
                }
                _ => {}
            }
        }

        fn container(
            &mut self,
            _id: Option<&iced_core::widget::Id>,
            _bounds: Rectangle,
            operate_on_children: &mut dyn FnMut(&mut dyn Operation<PathBuf>),
        ) {
            operate_on_children(self)
        }

        fn finish(&self) -> operation::Outcome<PathBuf> {
            self.path
                .clone()
                .map(operation::Outcome::Some)
                .unwrap_or(operation::Outcome::None)
        }
    }

    task::widget(GetPath {
        id: id.0.clone(),
        path: None,
        theme: PhantomData::<Theme>,
        renderer: PhantomData::<Renderer>,
    })
}

fn build(manifest: Result<PathBuf, crate::Error>) -> Task<Result<PathBuf, crate::Error>> {
    let manifest = match manifest {
        Err(error) => return Task::done(Err(error)),
        Ok(manifest) => manifest.to_path_buf(),
    };

    executor::try_spawn_blocking(move |mut sender| {
        let timer = std::time::Instant::now();
        let output = std::process::Command::new("cargo")
            .args([
                "component",
                "build",
                "--target",
                "wasm32-unknown-unknown",
                "--target-dir",
                "target",
            ])
            .current_dir(&manifest)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?
            .wait_with_output()?;

        tracing::info!(
            "`cargo component` finished with {:?} in {:?}",
            output.status,
            timer.elapsed()
        );

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::CargoComponent(stderr.to_string()));
        }

        let _ = sender.try_send(manifest);

        Ok(())
    })
}

fn create_runtime<Theme, Renderer>(
    id: &Id,
    manifest: Result<PathBuf, crate::Error>,
) -> Task<(Id, Result<PathBuf, crate::Error>)>
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
    struct CreateRuntime<Theme, Renderer> {
        id: Id,
        manifest: Result<PathBuf, crate::Error>,
        theme: PhantomData<Theme>,
        renderer: PhantomData<Renderer>,
    }

    impl<Theme, Renderer> Operation<(Id, Result<PathBuf, crate::Error>)>
        for CreateRuntime<Theme, Renderer>
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
        fn custom(
            &mut self,
            id: Option<&iced_core::widget::Id>,
            _bounds: Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            match id {
                Some(id) if id == &self.id.0 => {
                    if let Some(state) = state.downcast_mut::<Inner<Theme, Renderer>>() {
                        if let View::None | View::Failed(_) = &mut state.view {
                            match &self.manifest {
                                Err(error) => {
                                    state.view = View::Failed(error.clone());
                                    tracing::error!("Failed to create runtime {error:?}")
                                }
                                Ok(manifest) => {
                                    let timer = std::time::Instant::now();
                                    let runtime = runtime::Runtime::new(manifest);
                                    let element = runtime.view(&state.bytes);
                                    state.view = View::Built {
                                        runtime,
                                        element,
                                        error: None,
                                    };
                                    state.invalidated = true;
                                    tracing::info!(
                                        "Building `runtime::State` took {:?}",
                                        timer.elapsed()
                                    );
                                }
                            }
                        }
                    }
                    return;
                }
                _ => {}
            }
        }

        fn container(
            &mut self,
            _id: Option<&iced_core::widget::Id>,
            _bounds: Rectangle,
            operate_on_children: &mut dyn FnMut(
                &mut dyn Operation<(Id, Result<PathBuf, crate::Error>)>,
            ),
        ) {
            operate_on_children(self)
        }

        fn finish(&self) -> operation::Outcome<(Id, Result<PathBuf, crate::Error>)> {
            operation::Outcome::Some((self.id.clone(), self.manifest.clone()))
        }
    }

    task::widget(CreateRuntime {
        id: id.clone(),
        manifest,
        theme: PhantomData::<Theme>,
        renderer: PhantomData::<Renderer>,
    })
}

pub fn reload<Theme: Send + 'static, Renderer: Send + 'static>(
    id: impl Into<Id>,
    error: Option<crate::Error>,
) -> Task<()> {
    let id = id.into();

    struct Reload<Theme, Renderer> {
        id: iced_core::widget::Id,
        error: Option<crate::Error>,
        theme: PhantomData<Theme>,
        renderer: PhantomData<Renderer>,
    }

    impl<Theme: Send + 'static, Renderer: Send + 'static> Operation for Reload<Theme, Renderer> {
        fn custom(
            &mut self,
            id: Option<&iced_core::widget::Id>,
            _bounds: Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            match id {
                Some(id) if id == &self.id => {
                    if let Some(state) = state.downcast_mut::<Inner<Theme, Renderer>>() {
                        let timer = std::time::Instant::now();
                        if let View::Built {
                            runtime,
                            error: current_error,
                            ..
                        } = &mut state.view
                        {
                            *current_error = self.error.clone();
                            if self.error.is_some() {
                                return;
                            }

                            runtime.reload();
                            state.invalidated = true;
                            tracing::info!("Reloaded in {:?}", timer.elapsed());
                        }
                    }
                    return;
                }
                _ => {}
            }
        }

        fn container(
            &mut self,
            _id: Option<&iced_core::widget::Id>,
            _bounds: Rectangle,
            operate_on_children: &mut dyn FnMut(&mut dyn Operation<()>),
        ) {
            operate_on_children(self)
        }

        fn finish(&self) -> operation::Outcome<()> {
            operation::Outcome::Some(())
        }
    }

    task::widget(Reload {
        id: id.into(),
        error,
        theme: PhantomData::<Theme>,
        renderer: PhantomData::<Renderer>,
    })
}
