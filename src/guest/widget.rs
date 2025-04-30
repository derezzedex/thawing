use crate::guest;
use crate::runtime::thawing::core;
use core::types::{Color, Horizontal, Length, Padding, Pixels};

use wasmtime::component::Resource;

pub type Column<'a> =
    iced_widget::Column<'a, guest::Message, iced_widget::Theme, iced_widget::Renderer>;
pub type Button<'a> =
    iced_widget::Button<'a, guest::Message, iced_widget::Theme, iced_widget::Renderer>;
pub type Text<'a> = iced_widget::Text<'a, iced_widget::Theme, iced_widget::Renderer>;
pub type Checkbox<'a> =
    iced_widget::Checkbox<'a, guest::Message, iced_widget::Theme, iced_widget::Renderer>;

impl<'a> core::widget::HostCheckbox for guest::State<'a> {
    fn new(&mut self, label: String, is_checked: bool) -> Resource<core::widget::Checkbox> {
        let checkbox = Checkbox::new(label, is_checked);

        self.push(checkbox)
    }

    fn on_toggle(
        &mut self,
        checkbox: Resource<core::widget::Checkbox>,
        closure: Resource<core::types::Closure>,
    ) -> Resource<core::widget::Checkbox> {
        let mut widget = self.get_widget::<Checkbox, _>(&checkbox);
        widget = widget.on_toggle(move |value| guest::Message::stateful(&closure, value));

        self.insert(checkbox, widget)
    }

    fn into_element(
        &mut self,
        button: Resource<core::widget::Checkbox>,
    ) -> Resource<core::widget::Element> {
        Resource::new_own(button.rep())
    }

    fn drop(&mut self, _button: Resource<core::widget::Checkbox>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl<'a> core::widget::HostButton for guest::State<'a> {
    fn new(&mut self, content: Resource<core::widget::Element>) -> Resource<core::widget::Button> {
        let content = self.get(&content);
        let button = Button::new(content);

        self.push(button)
    }

    fn on_press_with(
        &mut self,
        button: Resource<core::widget::Button>,
        closure: Resource<core::types::Closure>,
    ) -> Resource<core::widget::Button> {
        let mut widget = self.get_widget::<Button, _>(&button);
        widget = widget.on_press_with(move || guest::Message::stateless(&closure));

        self.insert(button, widget)
    }

    fn into_element(
        &mut self,
        button: Resource<core::widget::Button>,
    ) -> Resource<core::widget::Element> {
        Resource::new_own(button.rep())
    }

    fn drop(&mut self, _button: Resource<core::widget::Button>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl<'a> core::widget::HostColumn for guest::State<'a> {
    fn new(&mut self) -> Resource<core::widget::Column> {
        self.push(Column::new())
    }

    fn from_vec(
        &mut self,
        children: Vec<Resource<core::widget::Element>>,
    ) -> Resource<core::widget::Column> {
        let capacity = children.capacity();
        let children =
            children
                .into_iter()
                .fold(Vec::with_capacity(capacity), |mut children, element| {
                    children.push(self.get(&element));
                    children
                });

        self.push(Column::from_vec(children))
    }

    fn spacing(
        &mut self,
        column: Resource<core::widget::Column>,
        amount: Pixels,
    ) -> Resource<core::widget::Column> {
        let mut widget = self.get_widget::<Column, _>(&column);
        widget = widget.spacing(amount);

        self.insert(column, widget)
    }

    fn padding(
        &mut self,
        column: Resource<core::widget::Column>,
        padding: Padding,
    ) -> Resource<core::widget::Column> {
        let mut widget = self.get_widget::<Column, _>(&column);
        widget = widget.padding(padding);

        self.insert(column, widget)
    }

    fn width(
        &mut self,
        column: Resource<core::widget::Column>,
        width: Length,
    ) -> Resource<core::widget::Column> {
        let mut widget = self.get_widget::<Column, _>(&column);
        widget = widget.width(width);

        self.insert(column, widget)
    }

    fn height(
        &mut self,
        column: Resource<core::widget::Column>,
        height: Length,
    ) -> Resource<core::widget::Column> {
        let mut widget = self.get_widget::<Column, _>(&column);
        widget = widget.height(height);

        self.insert(column, widget)
    }

    fn max_width(
        &mut self,
        column: Resource<core::widget::Column>,
        width: Pixels,
    ) -> Resource<core::widget::Column> {
        let mut widget = self.get_widget::<Column, _>(&column);
        widget = widget.max_width(width);

        self.insert(column, widget)
    }

    fn align_x(
        &mut self,
        column: Resource<core::widget::Column>,
        align: Horizontal,
    ) -> Resource<core::widget::Column> {
        let mut widget = self.get_widget::<Column, _>(&column);
        widget = widget.align_x(align);

        self.insert(column, widget)
    }

    fn clip(
        &mut self,
        column: Resource<core::widget::Column>,
        clip: bool,
    ) -> Resource<core::widget::Column> {
        let mut widget = self.get_widget::<Column, _>(&column);
        widget = widget.clip(clip);

        self.insert(column, widget)
    }

    fn push(
        &mut self,
        column: Resource<core::widget::Column>,
        child: Resource<core::widget::Element>,
    ) -> Resource<core::widget::Column> {
        let content = self.get(&child);
        let mut widget = self.get_widget::<Column, _>(&column);
        widget = widget.push(content);

        self.insert(column, widget)
    }

    fn extend(
        &mut self,
        column: Resource<core::widget::Column>,
        children: Vec<Resource<core::widget::Element>>,
    ) -> Resource<core::widget::Column> {
        let capacity = children.capacity();
        let children =
            children
                .into_iter()
                .fold(Vec::with_capacity(capacity), |mut children, element| {
                    children.push(self.get(&element));
                    children
                });

        let mut widget = self.get_widget::<Column, _>(&column);
        widget = widget.extend(children);

        self.insert(column, widget)
    }

    fn into_element(
        &mut self,
        column: Resource<core::widget::Column>,
    ) -> Resource<core::widget::Element> {
        Resource::new_own(column.rep())
    }

    fn drop(&mut self, _column: Resource<core::widget::Column>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl<'a> core::widget::HostText for guest::State<'a> {
    fn new(&mut self, fragment: String) -> Resource<core::widget::Text> {
        self.push(Text::new(fragment))
    }

    fn size(
        &mut self,
        text: Resource<core::widget::Text>,
        size: Pixels,
    ) -> Resource<core::widget::Text> {
        let mut widget = self.get_widget::<Text, _>(&text);
        widget = widget.size(size);

        self.insert(text, widget)
    }

    fn style(
        &mut self,
        text: Resource<core::widget::Text>,
        style_fn: Resource<core::types::Closure>,
    ) -> Resource<core::widget::Text> {
        let mut widget = self.get_widget::<Text, _>(&text);

        let runtime = self.runtime.as_ref().unwrap().clone();
        widget = widget
            .style(move |theme| runtime.call(style_fn.rep(), bincode::serialize(theme).unwrap()));

        self.insert(text, widget)
    }

    fn color(
        &mut self,
        text: Resource<core::widget::Text>,
        color: Color,
    ) -> Resource<core::widget::Text> {
        let mut widget = self.get_widget::<Text, _>(&text);
        widget = widget.color(color);

        self.insert(text, widget)
    }

    fn into_element(
        &mut self,
        text: Resource<core::widget::Text>,
    ) -> Resource<core::widget::Element> {
        Resource::new_own(text.rep())
    }

    fn drop(&mut self, _text: Resource<core::widget::Text>) -> wasmtime::Result<()> {
        Ok(())
    }
}
