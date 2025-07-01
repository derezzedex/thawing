# thawing
Experimental hot-reloading for [`iced`] through the [WebAssembly Component Model].

> [!WARNING]
> `thawing` is __extremely__ experimental and not ready for real usage!

## Overview
`thawing` exposes bindings to `iced` through the [Component Model], aiming to enable safe hot-reloading of `iced` applications.

This approach has the benefit of allowing other usages, such as integrating these bindings to other languages that support the component model, thus allowing developers to directly integrate them into their `iced` development workflow.

## Usage

> [!IMPORTANT]
> Note that as currently is, `thawing` is not yet ready for real usage, the following section is a merely a representation of the library's goals, how it plans to be used in the future!

To start using `thawing` you need to have [`cargo-component`] installed, and then introduce it to your dependecies:
```toml
# Cargo.toml
[dependencies]
thawing = "https://github.com/derezzedex/thawing"
```

Next, you should add the `thawing::view` widget somewhere in your application's view code, it's a widget macro that wraps your view code, allowing hot-reload of it's contents.

```rust
const ID: &'static str = "some-unique-string";

impl Example {
    fn view(&self) -> Element<Message> {
        thawing::view![
            text("Hello, world!")
        ]
        .id(ID)
        .into()
    }
}
```

Notice that you'll need to add an `id` to the `thawing::view!` macro, which takes us to the next step, you'll need to use the `thawing::thaw` function to begin the hot-reloading, it creates a task that will find the `view` widget and do the necessary setup to enable hot-reloading!

```rust
impl Example {
    fn new() -> (Self, Task<Message>) {
        (Example, thawing::thaw::<Message>(ID).map(|_| Message::Reloaded))
    }
}
```

After this, you can edit the contents of the `thawing::view!` macro and it'll be automatically reloaded for you, but it doesn't stop there, if you want to use the `State` of your application, you'll need to annotate it with another macro, that's where you'll use `thawing::data`:

```rust
#[thawing::data(message)]
enum Message {
    Increment,
    Decrement,
}

#[thawing::data(state)]
struct Example {
    value: i64,
}
```

This macro marks your `State` and `Message` so that they can be used inside of the `thawing::view!` macro, after that you'll need to pass your `State` to the `view` widget, like this:

```rust
impl Example {
    fn view(&self) -> Element<Message> {
        thawing::view![
            button("Increment").on_press(Message::Increment),
            text(self.value).size(50),
            button("Increment").on_press(Message::Decrement),
        ]
        .state(self)
        .id(ID)
        .into()
    }
}
```

And that's it!
You can also annotate other `struct`'s or `enum`'s with `#[thawing::data]` which your `State` or `Message` may depend on!

## Implementation

At it's core, `thawing` works by copying the contents of `thawing::view!` and other necessary code annotated by `thawing::data` into a temporary workspace, which gets compiled to a `.wasm` file, that file is then run by a runtime returning a new [`Element`], that `Element` represents the hot-reloaded view code.

Let's go through that process in more detail, step-by-step:

### Finding, copying and parsing the code

The `thawing::view` widget calls the [`file!`] macro, this path then gets [`canonicalized`] and stored in the widget state.
After that, when `thawing::thaw` task runs, it finds the widget using it's [`widget::Id`] and runs [`syn::parse_file`] 

It also creates a new temporary workspace through [`tempfile`], all of the code annotated by `thawing::data` and contained by `thawing::view!` gets [`quote`]d there, which looks something like this: 

```rust
quote! {
    #![allow(unused_imports)]
    use thawing_guest::thawing;
    use thawing_guest::widget::{button, checkbox, column, text, Style};
    use thawing_guest::{Application, Center, Element, Color, Theme, color};

    // Code annotated by `#[thawing::data]`
    #(#data)*

    // Code annotated by `#[thawing::data(message)]`
    #message

    // Code annotated by `#[thawing::data(state)]`
    #state

    impl Application for #state_ty {
        fn view(&self) -> impl Into<Element> {
            // Code inside of `thawing::view!`
            #view
        }
    }

    thawing_guest::thaw!(#state_ty);
}
```

[`file!`]: https://doc.rust-lang.org/std/macro.file.html
[`canonicalized`]: https://doc.rust-lang.org/std/path/struct.Path.html#method.canonicalize
[`widget::Id`]: https://docs.iced.rs/iced/advanced/widget/struct.Id.html
[`syn::parse_file`]: https://docs.rs/syn/latest/syn/fn.parse_file.html
[`tempfile`]: https://docs.rs/tempfile/latest/tempfile/fn.tempdir.html
[`quote`]: https://docs.rs/quote/latest/quote/

## Known limitations

There are quite a few limitations with the current approach, some are known to have a better solution.

### Reliance on `serde` and `bincode`

Current code is still a prototype, because of decisions taken to speed up the prototyping of the idea, 
`serde` and `bincode` are used instead of using the [Canonical ABI], this can be solved by expanding the interface (`.wit` files).

### Guest callbacks and generics

Guest callbacks and generics are modelled improperly; WIT, the used interface language doesn't have the callbacks or generics as first-class citizen, meaning a workaround is needed.
One of the drawbacks of this, is that the `'a` lifetime used in [`iced`]'s `View` is not possible, making all the callbacks `'static`.

[`iced`]: https://github.com/iced-rs/iced
[`Element`]: https://docs.iced.rs/iced_core/struct.Element.html
[`wasmtime`]: https://docs.rs/wasmtime/latest/wasmtime/
[WebAssembly Component]: https://component-model.bytecodealliance.org/design/components.html
[Component Model]: https://github.com/WebAssembly/component-model/
[WebAssembly Component Model]: https://component-model.bytecodealliance.org/
[`cargo-component`]: https://github.com/bytecodealliance/cargo-component