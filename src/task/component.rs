use std::path::PathBuf;
use std::process::Stdio;

use iced_core::Rectangle;
use iced_core::widget::Operation;
use iced_core::widget::operation;
use iced_widget::runtime::{Task, task};

use crate::runtime;
use crate::task::executor;
use crate::widget::{Id, Inner, View};

pub fn fetch_caller_path(id: &Id) -> Task<PathBuf> {
    struct GetPath {
        id: iced_core::widget::Id,
        path: Option<PathBuf>,
    }

    impl Operation<PathBuf> for GetPath {
        fn custom(
            &mut self,
            id: Option<&iced_core::widget::Id>,
            _bounds: Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            match id {
                Some(id) if id == &self.id => {
                    if let Some(state) = state.downcast_mut::<Inner>() {
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

pub fn create_runtime(
    id: &Id,
    manifest: Result<PathBuf, crate::Error>,
) -> Task<(Id, Result<PathBuf, crate::Error>)> {
    struct CreateRuntime {
        id: Id,
        manifest: Result<PathBuf, crate::Error>,
    }

    impl Operation<(Id, Result<PathBuf, crate::Error>)> for CreateRuntime {
        fn custom(
            &mut self,
            id: Option<&iced_core::widget::Id>,
            _bounds: Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            match id {
                Some(id) if id == &self.id.0 => {
                    if let Some(state) = state.downcast_mut::<Inner>() {
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
    })
}

pub fn reload(id: impl Into<Id>, error: Option<crate::Error>) -> Task<()> {
    let id = id.into();

    struct Reload {
        id: iced_core::widget::Id,
        error: Option<crate::Error>,
    }

    impl Operation for Reload {
        fn custom(
            &mut self,
            id: Option<&iced_core::widget::Id>,
            _bounds: Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            match id {
                Some(id) if id == &self.id => {
                    if let Some(state) = state.downcast_mut::<Inner>() {
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
    })
}
