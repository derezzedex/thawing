use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use iced_widget::runtime::Task;
use iced_widget::runtime::futures::futures::channel::mpsc::channel;
use iced_widget::runtime::futures::futures::{SinkExt, Stream, StreamExt};
use iced_widget::runtime::futures::{futures, stream};
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{DebouncedEventKind, new_debouncer};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::visit::{self, Visit};

use crate::error::MacroError;
use crate::task::executor;

pub fn init_directory() -> Task<Result<PathBuf, crate::Error>> {
    executor::try_spawn_blocking(|mut sender| {
        let manifest = tempfile::tempdir()?.keep().join("component");

        let timer = std::time::Instant::now();
        fs::create_dir(&manifest)?;

        let src_path = manifest.join("src");
        fs::create_dir(&src_path)?;

        let target_path = manifest.join("target");
        fs::create_dir(&target_path)?;

        fs::File::create(src_path.join("lib.rs"))?;

        let mut toml_file = fs::File::create(manifest.join("Cargo.toml"))?;
        toml_file.write_all(COMPONENT_TOML.as_bytes())?;

        tracing::info!("Creating `component` tempdir took {:?}", timer.elapsed());

        let _ = sender.try_send(manifest);

        Ok(())
    })
}

pub fn parse_and_write(
    caller: &PathBuf,
    manifest: Result<PathBuf, crate::Error>,
) -> Task<Result<PathBuf, crate::Error>> {
    let manifest = match manifest {
        Err(error) => return Task::done(Err(error)),
        Ok(manifest) => manifest.to_path_buf(),
    };

    let caller = caller.to_path_buf();
    let target = manifest.join("src").join("lib.rs");

    executor::try_spawn_blocking(move |mut sender| {
        let timer = std::time::Instant::now();
        let content = fs::read_to_string(caller)?;
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
        let mut lib_file = fs::File::create(target)?;
        lib_file.write_all(content.as_bytes())?;
        lib_file.sync_data()?;

        tracing::info!(
            "Parsing and writing to `component` took {:?}",
            timer.elapsed()
        );

        let _ = sender.try_send(manifest);

        Ok(())
    })
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
