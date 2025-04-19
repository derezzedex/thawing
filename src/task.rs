use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use iced_core::Rectangle;
use iced_core::text;
use iced_core::widget::Operation;
use iced_core::widget::operation;
use iced_futures::futures::channel::mpsc::channel;
use iced_futures::futures::{SinkExt, Stream, StreamExt};
use iced_futures::{futures, stream};
use iced_runtime::{Task, task};
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{DebouncedEventKind, new_debouncer};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::visit::{self, Visit};

use crate::{Id, Kind, Runtime, runtime};

pub fn watcher<Theme, Renderer>(id: impl Into<Id> + Clone + Send + 'static) -> Task<()>
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

    fetch_widget_kind::<Theme, Renderer>(id.clone()).then(move |kind| {
        let id = id.clone();

        match kind {
            Kind::ComponentFile(manifest) => reload::<Theme, Renderer>(id.clone()).chain(
                Task::stream(watch_file(manifest.clone())).then(move |_| {
                    build(manifest.clone()).chain(reload::<Theme, Renderer>(id.clone()))
                }),
            ),
            Kind::ViewMacro(caller) => {
                let temp_dir = tempfile::tempdir().unwrap();
                let manifest = temp_dir.path().join("component");

                init_directory(&manifest)
                    .chain(
                        parse_and_write(&caller, &manifest).chain(
                            build(&manifest)
                                .chain(create_runtime::<Theme, Renderer>(id.clone(), temp_dir)),
                        ),
                    )
                    .chain(Task::stream(watch_file(caller.clone())).then(move |_| {
                        let manifest = manifest.clone();
                        let id = id.clone();

                        parse_and_write(&caller, &manifest)
                            .then(move |_| build(manifest.clone()))
                            .then(move |_| reload::<Theme, Renderer>(id.clone()))
                    }))
            }
        }
    })
}

fn fetch_widget_kind<Theme: Send + 'static, Renderer: Send + 'static>(id: Id) -> Task<Kind> {
    struct GetPath<Theme, Renderer> {
        id: iced_core::widget::Id,
        kind: Option<Kind>,
        theme: PhantomData<Theme>,
        renderer: PhantomData<Renderer>,
    }

    impl<Theme: Send + 'static, Renderer: Send + 'static> Operation<Kind> for GetPath<Theme, Renderer> {
        fn custom(
            &mut self,
            id: Option<&iced_core::widget::Id>,
            _bounds: Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            match id {
                Some(id) if id == &self.id => {
                    if let Some(state) = state.downcast_mut::<crate::Inner<Theme, Renderer>>() {
                        self.kind = Some(state.kind.clone());
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
            operate_on_children: &mut dyn FnMut(&mut dyn Operation<Kind>),
        ) {
            operate_on_children(self)
        }

        fn finish(&self) -> operation::Outcome<Kind> {
            self.kind
                .clone()
                .map(operation::Outcome::Some)
                .unwrap_or(operation::Outcome::None)
        }
    }

    task::widget(GetPath {
        id: id.into(),
        kind: None,
        theme: PhantomData::<Theme>,
        renderer: PhantomData::<Renderer>,
    })
}

fn init_directory(path: impl AsRef<Path>) -> Task<()> {
    use tokio::fs;
    use tokio::io::AsyncWriteExt;

    let manifest = path.as_ref().to_path_buf();

    Task::future(async move {
        let timer = std::time::Instant::now();
        fs::create_dir(&manifest).await.unwrap();

        let src_path = manifest.join("src");
        fs::create_dir(&src_path).await.unwrap();

        let target_path = manifest.join("target");
        fs::create_dir(&target_path).await.unwrap();

        fs::File::create(src_path.join("lib.rs")).await.unwrap();

        let mut toml_file = fs::File::create(manifest.join("Cargo.toml")).await.unwrap();
        toml_file
            .write_all(COMPONENT_TOML.as_bytes())
            .await
            .unwrap();

        let mut gitignore_file = fs::File::create(manifest.join(".gitignore")).await.unwrap();
        gitignore_file
            .write_all(COMPONENT_GITIGNORE.as_bytes())
            .await
            .unwrap();

        tracing::info!("Creating `component` tempdir took {:?}", timer.elapsed());
    })
}

fn parse_and_write(caller: impl AsRef<Path>, manifest: impl AsRef<Path>) -> Task<()> {
    use tokio::io::AsyncWriteExt;
    use tokio::sync::oneshot;
    use tokio::{fs, task};

    let caller = caller.as_ref().to_path_buf();
    let target = manifest.as_ref().join("src").join("lib.rs");

    let (tx, rx) = oneshot::channel::<(PathBuf, PathBuf)>();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .unwrap();

    std::thread::spawn(move || {
        let local = task::LocalSet::new();
        local.spawn_local(async {
            if let Ok((caller, target)) = rx.await {
                let timer = std::time::Instant::now();
                task::spawn_local(async move {
                    let content = fs::read_to_string(caller).await.unwrap();
                    let caller = syn::parse_file(&content).unwrap();
                    let View {
                        data,
                        message,
                        state,
                        state_ty,
                        view,
                    } = ViewBuilder::from_file(&caller).build();

                    let output = quote! {
                        #![allow(unused_imports)]
                        use thawing_guest::thawing;
                        use thawing_guest::widget::{button, checkbox, column, text, Style};
                        use thawing_guest::{Application, Center, Element, Color, Theme, color};

                        #(#data)*

                        #message

                        #state

                        impl Application for #state_ty {
                            fn view(&self) -> impl Into<Element> {
                                #view
                            }
                        }

                        thawing_guest::thaw!(#state_ty);
                    };

                    let content =
                        prettyplease::unparse(&syn::parse_file(&output.to_string()).unwrap());
                    tracing::warn!("Wrote:\n{content}");
                    let mut lib_file = fs::File::options().write(true).open(target).await.unwrap();
                    lib_file.write_all(content.as_bytes()).await.unwrap();
                })
                .await
                .unwrap();
                tracing::info!(
                    "Parsing and writing to `component` took {:?}",
                    timer.elapsed()
                );
            }
        });

        rt.block_on(local);
    });

    Task::future(async move {
        tx.send((caller, target)).unwrap();
    })
}

fn build(manifest: impl AsRef<Path>) -> Task<()> {
    use tokio::process::Command;

    let manifest = manifest.as_ref().to_path_buf();

    Task::future(async move {
        let timer = std::time::Instant::now();
        let status = Command::new("cargo")
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
            .spawn()
            .expect("Failed to spawn compile command")
            .wait()
            .await
            .unwrap();

        tracing::info!(
            "`cargo component` finished with {status:?} in {:?}",
            timer.elapsed()
        );
    })
}

fn create_runtime<Theme, Renderer>(
    id: impl Into<Id> + Clone + Send + 'static,
    temp_dir: tempfile::TempDir,
) -> Task<()>
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

    struct Reload<Theme, Renderer> {
        id: iced_core::widget::Id,
        temp_dir: Option<tempfile::TempDir>,
        theme: PhantomData<Theme>,
        renderer: PhantomData<Renderer>,
    }

    impl<Theme, Renderer> Operation for Reload<Theme, Renderer>
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
                Some(id) if id == &self.id => {
                    if let Some(state) = state.downcast_mut::<crate::Inner<Theme, Renderer>>() {
                        if let Runtime::None = &mut state.runtime {
                            let timer = std::time::Instant::now();
                            let runtime = runtime::State::from_view(self.temp_dir.take().unwrap());
                            let element = runtime.view(&state.bytes);
                            state.runtime = Runtime::Built { runtime, element };
                            state.invalidated = true;
                            tracing::info!("Building `runtime::State` took {:?}", timer.elapsed());
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
        temp_dir: Some(temp_dir),
        theme: PhantomData::<Theme>,
        renderer: PhantomData::<Renderer>,
    })
}

pub fn reload<Theme: Send + 'static, Renderer: Send + 'static>(
    id: impl Into<Id> + Clone + Send + 'static,
) -> Task<()> {
    let id = id.into();

    struct Reload<Theme, Renderer> {
        id: iced_core::widget::Id,
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
                    if let Some(state) = state.downcast_mut::<crate::Inner<Theme, Renderer>>() {
                        let timer = std::time::Instant::now();
                        if let Runtime::Built { runtime, .. } = &mut state.runtime {
                            runtime.reload();
                        }

                        state.invalidated = true;
                        tracing::info!("Reloaded in {:?}", timer.elapsed());
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
        theme: PhantomData::<Theme>,
        renderer: PhantomData::<Renderer>,
    })
}

fn watch_file(path: impl AsRef<Path>) -> impl Stream<Item = ()> {
    let src_path = path.as_ref().to_path_buf();

    stream::channel(
        10,
        move |mut output: futures::channel::mpsc::Sender<()>| async move {
            let (mut tx, mut rx) = channel(1);

            let mut debouncer = new_debouncer(Duration::from_millis(500), move |res| {
                futures::executor::block_on(async {
                    tx.send(res).await.expect("Failed to send debounce event");
                })
            })
            .expect("Failed to create file watcher");

            debouncer
                .watcher()
                .watch(&src_path, RecursiveMode::Recursive)
                .expect("Failed to watch path");

            tracing::info!("Watching {src_path:?}");

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
                    output.send(()).await.expect("Failed to send message");
                }
            }
        },
    )
}

enum TypeDef<'ast> {
    Enum(&'ast syn::ItemEnum),
    Struct(&'ast syn::ItemStruct),
}

impl<'ast> quote::ToTokens for TypeDef<'ast> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            TypeDef::Enum(node) => node.to_tokens(tokens),
            TypeDef::Struct(node) => node.to_tokens(tokens),
        }
    }
}

struct ViewBuilder<'ast> {
    file: &'ast syn::File,
    view: Option<&'ast syn::Macro>,
    state_ty: Option<&'ast syn::Ident>,
    state: Option<TypeDef<'ast>>,
    message: Option<TypeDef<'ast>>,
    data: Vec<TypeDef<'ast>>,
}
struct View {
    view: TokenStream,
    state: TokenStream,
    state_ty: syn::Ident,
    message: TokenStream,
    data: Vec<TokenStream>,
}

impl<'ast> ViewBuilder<'ast> {
    fn build(mut self) -> View {
        self.visit_file(&self.file);

        View {
            data: self.data.iter().map(ToTokens::to_token_stream).collect(),
            view: self.view.unwrap().tokens.clone(),
            state: self.state.unwrap().to_token_stream(),
            state_ty: self.state_ty.unwrap().clone(),
            message: self.message.unwrap().to_token_stream(),
        }
    }

    fn from_file(file: &'ast syn::File) -> Self {
        Self {
            file,
            data: vec![],
            view: None,
            state_ty: None,
            state: None,
            message: None,
        }
    }
}

impl<'ast> Visit<'ast> for ViewBuilder<'ast> {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        for attr in node.attrs.iter() {
            if attr
                .path()
                .segments
                .first()
                .is_some_and(|p| p.ident == "thawing")
            {
                let mut state_or_data = false;
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("state") {
                        self.state = Some(TypeDef::Struct(node));
                        self.state_ty = Some(&node.ident);
                        state_or_data = true;
                    }

                    if meta.path.is_ident("message") {
                        self.message = Some(TypeDef::Struct(node));
                        state_or_data = true;
                    }

                    Ok(())
                });

                if !state_or_data {
                    self.data.push(TypeDef::Struct(node));
                }
            }
        }

        visit::visit_item_struct(self, node);
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        for attr in node.attrs.iter() {
            if attr
                .path()
                .segments
                .first()
                .is_some_and(|p| p.ident == "thawing")
            {
                let mut state_or_data = false;
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("state") {
                        self.state = Some(TypeDef::Enum(node));
                        self.state_ty = Some(&node.ident);
                        state_or_data = true;
                    }

                    if meta.path.is_ident("message") {
                        self.message = Some(TypeDef::Enum(node));
                        state_or_data = true;
                    }

                    Ok(())
                });

                if !state_or_data {
                    self.data.push(TypeDef::Enum(node));
                }
            }
        }

        visit::visit_item_enum(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if node
            .path
            .segments
            .first()
            .map(|p| p.ident.to_string())
            .is_some_and(|ident| &ident == "thawing")
            && node
                .path
                .segments
                .last()
                .map(|p| p.ident.to_string())
                .is_some_and(|ident| &ident == "view")
        {
            self.view = Some(node);
        }

        visit::visit_macro(self, node);
    }
}

const COMPONENT_GITIGNORE: &'static str = r#"bindings.rs
"#;

const COMPONENT_TOML: &'static str = r#"[package]
name = "component"
version = "0.1.0"
edition = "2024"

[workspace]

[dependencies]
thawing_guest = { git = "ssh://github.com/derezzedex/thawing" }

[lib]
crate-type = ["cdylib"]

[profile.release]
codegen-units = 1
opt-level = "s"
debug = false
strip = true
lto = true

[package.metadata.component]
package = "thawing:component"
dependencies = {}
"#;
