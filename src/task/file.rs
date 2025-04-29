use std::path::{Path, PathBuf};
use std::time::Duration;

use iced_futures::futures::channel::mpsc::channel;
use iced_futures::futures::{SinkExt, Stream, StreamExt};
use iced_futures::{futures, stream};
use iced_runtime::Task;
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{DebouncedEventKind, new_debouncer};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::visit::{self, Visit};

use crate::error::MacroError;

pub fn init_directory() -> Task<Result<PathBuf, crate::Error>> {
    use tokio::fs;
    use tokio::io::AsyncWriteExt;

    Task::future(async move {
        let manifest = tempfile::tempdir()?.into_path().join("component");

        let timer = std::time::Instant::now();
        fs::create_dir(&manifest).await?;

        let src_path = manifest.join("src");
        fs::create_dir(&src_path).await?;

        let target_path = manifest.join("target");
        fs::create_dir(&target_path).await?;

        fs::File::create(src_path.join("lib.rs")).await?;

        let mut toml_file = fs::File::create(manifest.join("Cargo.toml")).await?;
        toml_file.write_all(COMPONENT_TOML.as_bytes()).await?;

        let mut gitignore_file = fs::File::create(manifest.join(".gitignore")).await?;
        gitignore_file
            .write_all(COMPONENT_GITIGNORE.as_bytes())
            .await?;

        tracing::info!("Creating `component` tempdir took {:?}", timer.elapsed());

        Ok(manifest)
    })
}

pub fn parse_and_write(
    caller: &PathBuf,
    manifest: Result<PathBuf, crate::Error>,
) -> Task<Result<PathBuf, crate::Error>> {
    use tokio::io::AsyncWriteExt;
    use tokio::sync::oneshot;
    use tokio::{fs, task};

    let manifest = match manifest {
        Err(error) => return Task::done(Err(error)),
        Ok(manifest) => manifest.to_path_buf(),
    };

    let caller = caller.to_path_buf();
    let target = manifest.join("src").join("lib.rs");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build();

    match rt {
        Err(error) => Task::done(Err(error.into())),
        Ok(rt) => {
            let (tx, rx) =
                oneshot::channel::<(PathBuf, PathBuf, oneshot::Sender<Result<(), crate::Error>>)>();

            std::thread::spawn(move || {
                let local = task::LocalSet::new();
                local.spawn_local(async {
                    let (caller, target, tx) =
                        rx.await.expect("Failed to recv over `oneshot` channel");

                    let timer = std::time::Instant::now();
                    let output = task::spawn_local(async move {
                        let content = fs::read_to_string(caller).await?;
                        let caller = syn::parse_file(&content)?;
                        let ParsedFile {
                            data,
                            message,
                            state,
                            state_ty,
                            view,
                        } = FileParser::from_file(&caller).build()?;

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

                        let content = prettyplease::unparse(&syn::parse_file(&output.to_string())?);
                        tracing::info!("Wrote:\n{content}");
                        let mut lib_file = fs::File::create(target).await?;
                        lib_file.write_all(content.as_bytes()).await?;
                        lib_file.sync_data().await?;

                        Ok(())
                    })
                    .await
                    .map_err(crate::Error::from);

                    tx.send(output.and_then(std::convert::identity))
                        .expect("Failed to send over `oneshot` channel");

                    tracing::info!(
                        "Parsing and writing to `component` took {:?}",
                        timer.elapsed()
                    );
                });

                rt.block_on(local);
            });

            Task::future(async move {
                let (send, response) = oneshot::channel::<Result<(), crate::Error>>();
                tx.send((caller, target, send))
                    .map_err(|_| crate::Error::SendFailed)?;
                response.await.map_err(|_| crate::Error::RecvFailed)??;

                Ok(manifest)
            })
        }
    }
}

pub fn watch(path: impl AsRef<Path>) -> impl Stream<Item = ()> {
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

struct FileParser<'ast> {
    file: &'ast syn::File,
    view: Option<&'ast syn::Macro>,
    state_ty: Option<&'ast syn::Ident>,
    state: Option<TypeDef<'ast>>,
    message: Option<TypeDef<'ast>>,
    data: Vec<TypeDef<'ast>>,
}
struct ParsedFile {
    view: TokenStream,
    state: TokenStream,
    state_ty: syn::Ident,
    message: TokenStream,
    data: Vec<TokenStream>,
}

impl<'ast> FileParser<'ast> {
    fn build(mut self) -> Result<ParsedFile, crate::Error> {
        self.visit_file(&self.file);

        let data = self.data.iter().map(ToTokens::to_token_stream).collect();
        let view = self
            .view
            .ok_or(MacroError::ViewMacroMissing)?
            .tokens
            .clone();
        let state = self
            .state
            .ok_or(MacroError::StateAttributeMissing)?
            .to_token_stream();
        let state_ty = self
            .state_ty
            .ok_or(MacroError::StateAttributeMissing)?
            .clone();
        let message = self
            .message
            .ok_or(MacroError::MessageAttributeMissing)?
            .to_token_stream();

        Ok(ParsedFile {
            data,
            view,
            state,
            state_ty,
            message,
        })
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

impl<'ast> Visit<'ast> for FileParser<'ast> {
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
