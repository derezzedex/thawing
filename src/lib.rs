mod runtime;
mod types;
mod widget;

use std::cell::OnceCell;
use std::io::Write;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

// Marker for `view` macro
pub trait State {}

// Marker for `view` macro
pub trait Message {}

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
    None {
        temp_dir: Option<tempfile::TempDir>,
    },
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
            Kind::ViewMacro(_) => Runtime::None { temp_dir: None },
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
            Runtime::None { .. } => self
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
            Runtime::None { .. } => self.initial.as_ref().unwrap().as_widget().layout(
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
            Runtime::None { .. } => self.initial.as_ref().unwrap().as_widget().operate(
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

        if let Runtime::None { temp_dir } = &mut state.runtime {
            if let Some(temp_dir) = temp_dir.take() {
                let timer = std::time::Instant::now();
                let runtime = runtime::State::from_view(temp_dir);
                let element = runtime.view(&state.bytes);
                state.runtime = Runtime::Built { runtime, element };
                tracing::info!(
                    "Building `runtime::State::from_view` took {:?}",
                    timer.elapsed()
                );
            }
        }

        match &mut state.runtime {
            Runtime::None { .. } => self.initial.as_mut().unwrap().as_widget_mut().update(
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
            Runtime::None { .. } => self
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
            Runtime::None { .. } => self.initial.as_ref().unwrap().as_widget().draw(
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

pub fn watch(path: impl AsRef<Path> + 'static) -> Task<()> {
    Task::stream(watch_file(path.as_ref().to_path_buf()))
}

// TODO(derezzedex): if `watch` is used instead, a `Widget::update` call
// is needed (e.g. moving the mouse, or focusing the window) to display changes;
// this sends a message to the application, forcing a `Widget::update`
pub fn watch_and_reload<Message, Theme, Renderer>(
    id: impl Into<Id> + Clone + Send + 'static,
    on_reload: Message,
) -> Task<Message>
where
    Message: Clone + Send + 'static,
    Theme: Send + 'static,
    Renderer: Send + 'static,
{
    let id = id.into();

    get_path::<Theme, Renderer>(id.clone())
        .then(move |(id, path)| {
            reload::<Theme, Renderer>(id).then(move |_| Task::done(path.clone()))
        })
        .then(watch)
        .then(move |_| reload::<Theme, Renderer>(id.clone()))
        .map(move |_| on_reload.clone())
}

fn get_path<Theme: Send + 'static, Renderer: Send + 'static>(id: Id) -> Task<(Id, PathBuf)> {
    struct GetPath<Theme, Renderer> {
        id: iced_core::widget::Id,
        path: Option<PathBuf>,
        theme: PhantomData<Theme>,
        renderer: PhantomData<Renderer>,
    }

    impl<Theme: Send + 'static, Renderer: Send + 'static> Operation<(Id, PathBuf)>
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
                    if let Some(state) = state.downcast_mut::<crate::Inner<Theme, Renderer>>() {
                        let path = match &state.kind {
                            Kind::ComponentFile(path) => path.join("src"),
                            Kind::ViewMacro(path) => path.clone(),
                        };
                        self.path = Some(path);
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
            operate_on_children: &mut dyn FnMut(&mut dyn Operation<(Id, PathBuf)>),
        ) {
            operate_on_children(self)
        }

        fn finish(&self) -> operation::Outcome<(Id, PathBuf)> {
            self.path
                .as_ref()
                .map(|path| operation::Outcome::Some((self.id.clone().into(), path.clone())))
                .unwrap_or(operation::Outcome::None)
        }
    }

    task::widget(GetPath {
        id: id.into(),
        path: None,
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
                        if let Kind::ViewMacro(caller) = &state.kind {
                            let manifest = match &mut state.runtime {
                                Runtime::None { temp_dir } => {
                                    let timer = std::time::Instant::now();
                                    let tmp = tempfile::tempdir().unwrap();
                                    let manifest = tmp.path().join("component");

                                    std::fs::create_dir(&manifest).unwrap();

                                    let src_path = manifest.join("src");
                                    std::fs::create_dir(&src_path).unwrap();

                                    let target_path = manifest.join("target");
                                    std::fs::create_dir(&target_path).unwrap();

                                    std::fs::File::create(src_path.join("lib.rs")).unwrap();

                                    let mut toml_file =
                                        std::fs::File::create(manifest.join("Cargo.toml")).unwrap();
                                    toml_file.write_all(COMPONENT_TOML.as_bytes()).unwrap();

                                    let mut gitignore_file =
                                        std::fs::File::create(manifest.join(".gitignore")).unwrap();
                                    gitignore_file
                                        .write_all(COMPONENT_GITIGNORE.as_bytes())
                                        .unwrap();

                                    *temp_dir = Some(tmp);
                                    tracing::info!(
                                        "Created temporary `component` directory in {:?}",
                                        timer.elapsed()
                                    );

                                    manifest
                                }
                                Runtime::Built { runtime, .. } => runtime.manifest.clone(),
                            };

                            let content = std::fs::read_to_string(caller).unwrap();
                            let caller = syn::parse_file(&content).unwrap();

                            let View {
                                message,
                                state,
                                state_ty,
                                view,
                                ..
                            } = ViewBuilder::from_file(&caller).build();

                            let output = quote! {
                                use thawing_guest::widget::{button, checkbox, column, text};
                                use thawing_guest::{Application, Center, Element};

                                #message

                                #state

                                impl Application for #state_ty {
                                    fn view(&self) -> impl Into<Element> {
                                        #view
                                    }
                                }

                                thawing_guest::thaw!(#state_ty);
                            };

                            let lib_content = prettyplease::unparse(
                                &syn::parse_file(&output.to_string()).unwrap(),
                            );
                            let mut lib_file = std::fs::File::options()
                                .write(true)
                                .open(manifest.join("src").join("lib.rs"))
                                .unwrap();
                            lib_file.write_all(lib_content.as_bytes()).unwrap();

                            tracing::info!("Building component...");
                            let timer = Instant::now();
                            Command::new("cargo")
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
                                .output()
                                .expect("Failed to build component");
                            tracing::info!("Component built in {:?}", timer.elapsed());
                        }

                        if let Runtime::Built { runtime, .. } = &mut state.runtime {
                            runtime.reload();
                        }

                        state.invalidated = true;
                        tracing::info!("Reloaded in {:?}", timer.elapsed());
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

fn watch_file(src_path: PathBuf) -> impl Stream<Item = ()> {
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
    message_ty: Option<&'ast syn::Ident>,
    message: Option<TypeDef<'ast>>,
}

pub(crate) struct View {
    pub(crate) view: TokenStream,
    pub(crate) state: TokenStream,
    pub(crate) state_ty: syn::Ident,
    pub(crate) message: TokenStream,
}

impl<'ast> ViewBuilder<'ast> {
    pub(crate) fn build(mut self) -> View {
        self.visit_file(&self.file);

        if self.state.is_none() || self.message.is_none() {
            self.visit_file(&self.file);
        }

        View {
            view: self.view.unwrap().tokens.clone(),
            state: self.state.unwrap().to_token_stream(),
            state_ty: self.state_ty.unwrap().clone(),
            message: self.message.unwrap().to_token_stream(),
        }
    }

    pub(crate) fn from_file(file: &'ast syn::File) -> Self {
        Self {
            file,
            view: None,
            state_ty: None,
            state: None,
            message_ty: None,
            message: None,
        }
    }
}

impl<'ast> Visit<'ast> for ViewBuilder<'ast> {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if self.state_ty == Some(&node.ident) {
            self.state = Some(TypeDef::Struct(node));
        } else if self.message_ty == Some(&node.ident) {
            self.message = Some(TypeDef::Struct(node));
        }

        visit::visit_item_struct(self, node);
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        if self.state_ty == Some(&node.ident) {
            self.state = Some(TypeDef::Enum(node));
        } else if self.message_ty == Some(&node.ident) {
            self.message = Some(TypeDef::Enum(node));
        }

        visit::visit_item_enum(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if let Some((_, path, _)) = &node.trait_ {
            if path
                .segments
                .first()
                .map(|p| p.ident.to_string())
                .is_some_and(|ident| &ident == "thawing")
            {
                let marker = path.segments.last().map(|p| p.ident.to_string());
                match marker {
                    Some(ident) if &ident == "State" => {
                        if let syn::Type::Path(self_ty) = &*node.self_ty {
                            self.state_ty = self_ty.path.get_ident();
                        }
                    }
                    Some(ident) if &ident == "Message" => {
                        if let syn::Type::Path(self_ty) = &*node.self_ty {
                            self.message_ty = self_ty.path.get_ident();
                        }
                    }
                    _ => {}
                }
            }
        }

        visit::visit_item_impl(self, node);
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
thawing_guest = { git = "ssh://github.com/derezzedex/thawing", branch = "dev/widget-api" }
serde = { version = "1.0", features = ["derive"] }

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
