use std::marker::PhantomData;
use std::path::PathBuf;
use std::process::Stdio;

use iced_core::Rectangle;
use iced_core::widget::Operation;
use iced_core::widget::operation;
use iced_widget::runtime::{Task, task};

use crate::guest;
use crate::runtime;
use crate::task::executor;
use crate::widget::{Error, Id, Inner, View};

pub fn fetch_caller_path<Message: Send + 'static>(id: &Id) -> Task<PathBuf> {
    struct GetPath<Message> {
        id: iced_core::widget::Id,
        path: Option<PathBuf>,
        message: PhantomData<Message>,
    }

    impl<Message: Send + 'static> Operation<PathBuf> for GetPath<Message> {
        fn custom(
            &mut self,
            id: Option<&iced_core::widget::Id>,
            _bounds: Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            match id {
                Some(id) if id == &self.id => {
                    if let Some(state) = state.downcast_mut::<Inner<Message>>() {
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
        message: PhantomData::<Message>,
    })
}

pub fn build(manifest: Result<PathBuf, crate::Error>) -> Task<Result<PathBuf, crate::Error>> {
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

pub fn create_runtime<Message: serde::de::DeserializeOwned + Send + 'static>(
    id: &Id,
    manifest: Result<PathBuf, crate::Error>,
) -> Task<(Id, Result<PathBuf, crate::Error>)> {
    struct CreateRuntime<Message> {
        id: Id,
        manifest: Result<PathBuf, crate::Error>,
        message: PhantomData<Message>,
    }

    impl<Message: serde::de::DeserializeOwned + Send + 'static>
        Operation<(Id, Result<PathBuf, crate::Error>)> for CreateRuntime<Message>
    {
        fn custom(
            &mut self,
            id: Option<&iced_core::widget::Id>,
            _bounds: Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            match id {
                Some(id) if id == &self.id.0 => {
                    if let Some(state) = state.downcast_mut::<Inner<Message>>() {
                        if let View::None | View::Failed(_) = &mut state.view {
                            match &self.manifest {
                                Err(error) => {
                                    state.view = View::Failed(Error::new(error.clone()));
                                    tracing::error!("Failed to create runtime {error:?}")
                                }
                                Ok(manifest) => {
                                    let timer = std::time::Instant::now();
                                    let runtime = runtime::Runtime::new(manifest);
                                    let element = runtime.view(&state.bytes);
                                    let mapper = {
                                        let runtime = runtime.state();
                                        Box::new(move |message: guest::Message| {
                                            runtime.call(message.closure, message.data)
                                        })
                                    };

                                    state.view = View::Built {
                                        runtime,
                                        element,
                                        mapper,
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
        message: PhantomData::<Message>,
    })
}

pub fn reload<Message: Send + 'static>(id: impl Into<Id>, error: Option<crate::Error>) -> Task<()> {
    let id = id.into();

    struct Reload<Message> {
        id: iced_core::widget::Id,
        error: Option<crate::Error>,
        message: PhantomData<Message>,
    }

    impl<Message: Send + 'static> Operation for Reload<Message> {
        fn custom(
            &mut self,
            id: Option<&iced_core::widget::Id>,
            _bounds: Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            match id {
                Some(id) if id == &self.id => {
                    if let Some(state) = state.downcast_mut::<Inner<Message>>() {
                        let timer = std::time::Instant::now();
                        if let View::Built {
                            runtime,
                            error: current_error,
                            ..
                        } = &mut state.view
                        {
                            *current_error = self.error.take().map(Error::new);
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
        message: PhantomData::<Message>,
    })
}
