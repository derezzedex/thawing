mod runtime;
mod types;
mod widget;

use std::cell::OnceCell;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced_core::widget::{Operation, Tree, operation, tree};
use iced_core::{Clipboard, Event, Layout, Length, Rectangle, Shell, Size, Widget};
use iced_core::{layout, mouse, renderer, text};
use iced_futures::futures::channel::mpsc::channel;
use iced_futures::futures::{SinkExt, Stream, StreamExt};
use iced_futures::{futures, stream};
use iced_runtime::{Task, task};
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{DebouncedEventKind, new_debouncer};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::visit::{self, Visit};

pub use iced_core::Element;
pub use serde;
pub use thawing_macro::data;

#[macro_export]
macro_rules! view {
    ($widget:expr) => {
        $crate::Thawing::from_view($crate::Element::from($widget), file!())
    };
}

pub fn component<'a, Message, Theme, Renderer, State>(
    path: impl AsRef<Path>,
) -> Thawing<'a, Message, Theme, Renderer, State> {
    Thawing::from_component(path)
}

#[derive(Debug, Clone)]
enum Kind {
    ViewMacro(PathBuf),
    ComponentFile(PathBuf),
}

pub struct Thawing<'a, Message, Theme, Renderer, State = ()> {
    id: Option<Id>,
    width: Length,
    height: Length,

    kind: Kind,
    initial: Option<Element<'a, Message, Theme, Renderer>>,
    bytes: Arc<Vec<u8>>,
    tree: Mutex<OnceCell<Tree>>,

    state: PhantomData<&'a State>,
    message: PhantomData<Message>,
}

impl<'a, Message, Theme, Renderer, State> Thawing<'a, Message, Theme, Renderer, State> {
    pub fn from_view(
        element: impl Into<Element<'a, Message, Theme, Renderer>>,
        file: &'static str,
    ) -> Self {
        Self {
            id: None,
            kind: Kind::ViewMacro(Path::new(file).canonicalize().unwrap()),
            initial: Some(element.into()),
            bytes: Arc::new(Vec::new()),
            width: Length::Shrink,
            height: Length::Shrink,
            tree: Mutex::new(OnceCell::new()),
            state: PhantomData,
            message: PhantomData,
        }
    }

    pub fn from_component(path: impl AsRef<Path>) -> Self {
        Self {
            id: None,
            kind: Kind::ComponentFile(path.as_ref().to_path_buf()),
            initial: None,
            bytes: Arc::new(Vec::new()),
            width: Length::Shrink,
            height: Length::Shrink,
            tree: Mutex::new(OnceCell::new()),
            state: PhantomData,
            message: PhantomData,
        }
    }

    pub fn id(mut self, id: impl Into<Id>) -> Self {
        self.id = Some(id.into());
        self
    }
}

impl<'a, Message, Theme, Renderer, State> Thawing<'a, Message, Theme, Renderer, State>
where
    State: serde::Serialize,
{
    pub fn state<'b>(mut self, state: &'b State) -> Self {
        self.bytes = Arc::new(bincode::serialize(state).unwrap());
        self
    }
}

impl<'a, Message, Theme, Renderer, State> From<Thawing<'a, Message, Theme, Renderer, State>>
    for Element<'a, Message, Theme, Renderer>
where
    State: serde::Serialize + 'static,
    Message: 'static + serde::Serialize + serde::de::DeserializeOwned,
    Renderer: 'static + iced_core::Renderer + iced_core::text::Renderer,
    Theme: 'static
        + iced_widget::checkbox::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    <Theme as iced_widget::text::Catalog>::Class<'static>:
        From<iced_widget::text::StyleFn<'static, Theme>>,
{
    fn from(widget: Thawing<'a, Message, Theme, Renderer, State>) -> Self {
        Element::new(widget)
    }
}

enum Runtime<Theme, Renderer> {
    None,
    Built {
        runtime: runtime::State<'static, Theme, Renderer>,
        element: Element<'static, runtime::Message, Theme, Renderer>,
    },
}

pub(crate) struct Inner<Theme, Renderer> {
    runtime: Runtime<Theme, Renderer>,
    invalidated: bool,
    bytes: Arc<Vec<u8>>,
    kind: Kind,
}

impl<Theme, Renderer> Inner<Theme, Renderer>
where
    Renderer: 'static + iced_core::Renderer + text::Renderer,
    Theme: 'static
        + iced_widget::checkbox::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    <Theme as iced_widget::text::Catalog>::Class<'static>:
        From<iced_widget::text::StyleFn<'static, Theme>>,
{
    fn new(kind: &Kind, bytes: Arc<Vec<u8>>) -> Self {
        let runtime = match kind {
            Kind::ComponentFile(path) => {
                let runtime = runtime::State::from_component(&path);
                let element = runtime.view(&bytes);

                Runtime::Built { runtime, element }
            }
            Kind::ViewMacro(_) => Runtime::None,
        };

        Self {
            runtime,
            invalidated: false,
            kind: kind.clone(),
            bytes,
        }
    }

    fn diff(&mut self, other: &Arc<Vec<u8>>) {
        if Arc::ptr_eq(&self.bytes, other) {
            return;
        }

        if let Runtime::Built { runtime, element } = &mut self.runtime {
            *element = runtime.view(other);
        }

        self.bytes = Arc::clone(other);
    }
}

impl<'a, Message, Theme, Renderer, State> Widget<Message, Theme, Renderer>
    for Thawing<'a, Message, Theme, Renderer, State>
where
    State: serde::Serialize + 'static,
    Message: serde::Serialize + serde::de::DeserializeOwned,
    Renderer: 'static + iced_core::Renderer + text::Renderer,
    Theme: 'static
        + iced_widget::checkbox::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    <Theme as iced_widget::text::Catalog>::Class<'static>:
        From<iced_widget::text::StyleFn<'static, Theme>>,
{
    fn tag(&self) -> tree::Tag {
        struct Tag<T>(T);

        tree::Tag::of::<Tag<State>>()
    }

    fn state(&self) -> tree::State {
        let state: Inner<Theme, Renderer> = Inner::new(&self.kind, Arc::clone(&self.bytes));
        if let Runtime::Built { element, .. } = &state.runtime {
            let _ = self.tree.lock().unwrap().set(Tree::new(element));
        }
        tree::State::new(state)
    }

    fn children(&self) -> Vec<Tree> {
        self.initial
            .as_ref()
            .map(|el| el.as_widget().children())
            .unwrap_or_else(|| vec![self.tree.lock().unwrap().take().unwrap()])
    }

    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<Inner<Theme, Renderer>>();
        state.diff(&self.bytes);

        match &state.runtime {
            Runtime::None => self
                .initial
                .as_ref()
                .unwrap()
                .as_widget()
                .diff(&mut tree.children[0]),
            Runtime::Built { element, .. } => element.as_widget().diff(&mut tree.children[0]),
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_ref::<Inner<Theme, Renderer>>();

        match &state.runtime {
            Runtime::None => self.initial.as_ref().unwrap().as_widget().layout(
                &mut tree.children[0],
                renderer,
                limits,
            ),
            Runtime::Built { element, .. } => {
                element
                    .as_widget()
                    .layout(&mut tree.children[0], renderer, limits)
            }
        }
    }

    fn operate(
        &self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let id = self.id.as_ref().map(|id| &id.0);
        let state = tree.state.downcast_mut::<Inner<Theme, Renderer>>();

        operation.custom(id, layout.bounds(), state);
        operation.container(id, layout.bounds(), &mut |operation| match &state.runtime {
            Runtime::None => self.initial.as_ref().unwrap().as_widget().operate(
                &mut tree.children[0],
                layout,
                renderer,
                operation,
            ),
            Runtime::Built { element, .. } => {
                element
                    .as_widget()
                    .operate(&mut tree.children[0], layout, renderer, operation)
            }
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<Inner<Theme, Renderer>>();

        if state.invalidated {
            shell.request_redraw();
            state.invalidated = false;
        }

        match &mut state.runtime {
            Runtime::None => self.initial.as_mut().unwrap().as_widget_mut().update(
                &mut tree.children[0],
                event,
                layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            ),
            Runtime::Built { element, runtime } => {
                let mut messages = vec![];
                let mut guest = Shell::new(&mut messages);

                element.as_widget_mut().update(
                    &mut tree.children[0],
                    event,
                    layout,
                    cursor,
                    renderer,
                    clipboard,
                    &mut guest,
                    viewport,
                );

                shell.merge(guest, move |message| {
                    runtime.call(message.closure, message.data)
                });
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<Inner<Theme, Renderer>>();

        match &state.runtime {
            Runtime::None => self
                .initial
                .as_ref()
                .unwrap()
                .as_widget()
                .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer),
            Runtime::Built { element, .. } => element.as_widget().mouse_interaction(
                &tree.children[0],
                layout,
                cursor,
                viewport,
                renderer,
            ),
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<Inner<Theme, Renderer>>();

        match &state.runtime {
            Runtime::None => self.initial.as_ref().unwrap().as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout,
                cursor,
                viewport,
            ),
            Runtime::Built { element, .. } => element.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout,
                cursor,
                viewport,
            ),
        }
    }

    // TODO(derezzedex): implement Widget::overlay
}

#[derive(Debug, Clone)]
pub struct Id(pub(crate) iced_core::widget::Id);

impl Id {
    pub fn new(id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        Self(iced_core::widget::Id::new(id))
    }

    pub fn unique() -> Self {
        Self(iced_core::widget::Id::unique())
    }
}

impl From<iced_core::widget::Id> for Id {
    fn from(id: iced_core::widget::Id) -> Self {
        Self(id)
    }
}

impl From<Id> for iced_core::widget::Id {
    fn from(id: Id) -> Self {
        id.0
    }
}

impl From<&'static str> for Id {
    fn from(value: &'static str) -> Self {
        Id::new(value)
    }
}

pub fn watcher<Theme, Renderer>(id: impl Into<Id> + Clone + Send + 'static) -> Task<()>
where
    Renderer: 'static + Send + iced_core::Renderer + text::Renderer,
    Theme: 'static
        + Send
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
                        use thawing_guest::thawing;
                        use thawing_guest::widget::{button, checkbox, column, text};
                        use thawing_guest::{Application, Center, Element};

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

pub fn create_runtime<Theme, Renderer>(
    id: impl Into<Id> + Clone + Send + 'static,
    temp_dir: tempfile::TempDir,
) -> Task<()>
where
    Renderer: 'static + Send + iced_core::Renderer + text::Renderer,
    Theme: 'static
        + Send
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

pub(crate) struct ViewBuilder<'ast> {
    file: &'ast syn::File,
    view: Option<&'ast syn::Macro>,
    state_ty: Option<&'ast syn::Ident>,
    state: Option<TypeDef<'ast>>,
    message: Option<TypeDef<'ast>>,
    data: Vec<TypeDef<'ast>>,
}

pub(crate) struct View {
    pub(crate) view: TokenStream,
    pub(crate) state: TokenStream,
    pub(crate) state_ty: syn::Ident,
    pub(crate) message: TokenStream,
    pub(crate) data: Vec<TokenStream>,
}

impl<'ast> ViewBuilder<'ast> {
    pub(crate) fn build(mut self) -> View {
        self.visit_file(&self.file);

        View {
            data: self.data.iter().map(ToTokens::to_token_stream).collect(),
            view: self.view.unwrap().tokens.clone(),
            state: self.state.unwrap().to_token_stream(),
            state_ty: self.state_ty.unwrap().clone(),
            message: self.message.unwrap().to_token_stream(),
        }
    }

    pub(crate) fn from_file(file: &'ast syn::File) -> Self {
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
