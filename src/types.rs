use crate::runtime::thawing::core::types::{Color, Horizontal, Length, Padding, Pixels};

impl From<Pixels> for iced::Pixels {
    fn from(pixels: Pixels) -> Self {
        iced::Pixels(pixels.amount)
    }
}

impl From<Padding> for iced::Padding {
    fn from(padding: Padding) -> Self {
        iced::Padding {
            top: padding.top,
            right: padding.right,
            bottom: padding.top,
            left: padding.left,
        }
    }
}

impl From<Color> for iced::Color {
    fn from(color: Color) -> Self {
        iced::Color {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        }
    }
}

impl From<Length> for iced::Length {
    fn from(length: Length) -> Self {
        match length {
            Length::Fill => iced::Length::Fill,
            Length::FillPortion(portion) => iced::Length::FillPortion(portion),
            Length::Fixed(amount) => iced::Length::Fixed(amount),
            Length::Shrink => iced::Length::Shrink,
        }
    }
}

impl From<Horizontal> for iced::alignment::Horizontal {
    fn from(align: Horizontal) -> Self {
        match align {
            Horizontal::Left => iced::alignment::Horizontal::Left,
            Horizontal::Center => iced::alignment::Horizontal::Center,
            Horizontal::Right => iced::alignment::Horizontal::Right,
        }
    }
}
