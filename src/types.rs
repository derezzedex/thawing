use crate::runtime::thawing::core::types::{Color, Horizontal, Length, Padding, Pixels};

impl From<Pixels> for iced_core::Pixels {
    fn from(pixels: Pixels) -> Self {
        iced_core::Pixels(pixels.amount)
    }
}

impl From<Padding> for iced_core::Padding {
    fn from(padding: Padding) -> Self {
        iced_core::Padding {
            top: padding.top,
            right: padding.right,
            bottom: padding.top,
            left: padding.left,
        }
    }
}

impl From<Color> for iced_core::Color {
    fn from(color: Color) -> Self {
        iced_core::Color {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        }
    }
}

impl From<Length> for iced_core::Length {
    fn from(length: Length) -> Self {
        match length {
            Length::Fill => iced_core::Length::Fill,
            Length::FillPortion(portion) => iced_core::Length::FillPortion(portion),
            Length::Fixed(amount) => iced_core::Length::Fixed(amount),
            Length::Shrink => iced_core::Length::Shrink,
        }
    }
}

impl From<Horizontal> for iced_core::alignment::Horizontal {
    fn from(align: Horizontal) -> Self {
        match align {
            Horizontal::Left => iced_core::alignment::Horizontal::Left,
            Horizontal::Center => iced_core::alignment::Horizontal::Center,
            Horizontal::Right => iced_core::alignment::Horizontal::Right,
        }
    }
}
